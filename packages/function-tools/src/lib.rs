use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use aa_kernel::tool_provider::*;
use async_trait::async_trait;
use tokio::fs;
use reqwest::Client;

pub struct FsToolProvider;

impl ToolProvider for FsToolProvider {
    fn tools(&self, _scope: &ToolProviderScope<'_>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(FsRead),
            Arc::new(FsWrite),
            Arc::new(FsLs),
            Arc::new(FsFind),
            Arc::new(FsGrep),
            Arc::new(FsInfo),
            Arc::new(ShellExec),
            Arc::new(WebFetch),
        ]
    }
}

struct FsRead;
struct FsWrite;
struct FsLs;
struct FsFind;
struct FsGrep;
struct FsInfo;

#[async_trait]
impl Tool for FsRead {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_read".into(),
            description: "Read file content".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file" }
                },
                "required": ["path"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let path = req_str(&arguments, "path")?;
        let full = resolve_path(&path, ctx);
        let content = fs::read_to_string(&full)
            .await
            .map_err(|e| ToolError::Execution(format!("read {path}: {e}")))?;
        Ok(ToolResult::text(content))
    }
}

#[async_trait]
impl Tool for FsWrite {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_write".into(),
            description: "Write content to file".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file" },
                    "content": { "type": "string", "description": "Content to write" },
                    "append": { "type": "boolean", "description": "Append instead of overwrite" }
                },
                "required": ["path", "content"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let path = req_str(&arguments, "path")?;
        let content = req_str(&arguments, "content")?;
        let append = arguments.get("append").and_then(|v| v.as_bool()).unwrap_or(false);
        let full = resolve_path(&path, ctx);

        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::Execution(format!("create dirs: {e}")))?;
        }

        if append {
            let full = full.clone();
            let content = content.clone();
            let path_clone = path.clone();
            tokio::task::spawn_blocking(move || {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .append(true).create(true)
                    .open(&full)
                    .map_err(|e| format!("open {}: {e}", path_clone))?;
                f.write_all(content.as_bytes())
                    .map_err(|e| format!("write {}: {e}", path_clone))?;
                f.sync_all()
                    .map_err(|e| format!("sync {}: {e}", path_clone))?;
                Ok::<_, String>(())
            })
            .await
            .map_err(|e| ToolError::Execution(format!("join: {e}")))?
            .map_err(|e| ToolError::Execution(e))?;
        } else {
            fs::write(&full, &content)
                .await
                .map_err(|e| ToolError::Execution(format!("write {path}: {e}")))?;
        }

        Ok(ToolResult::text(format!("{path} {}", if append { "appended" } else { "written" })))
    }
}

#[async_trait]
impl Tool for FsLs {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_ls".into(),
            description: "List directory contents".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path" },
                    "recursive": { "type": "boolean", "description": "List recursively" }
                },
                "required": []
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let recursive = arguments.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
        let mut entries = list_entries(resolve_path(path, ctx), recursive).await?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ToolResult::text(serde_json::to_string_pretty(&entries).unwrap_or_default()))
    }
}

#[async_trait]
impl Tool for FsFind {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_find".into(),
            description: "Find files matching a glob pattern".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern (e.g. **/*.rs)" },
                    "path": { "type": "string", "description": "Root directory" },
                    "max_depth": { "type": "integer", "description": "Maximum search depth" }
                },
                "required": ["pattern"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let pattern = req_str(&arguments, "pattern")?;
        let root = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max_depth = arguments.get("max_depth").and_then(|v| v.as_u64());
        let full = resolve_path(root, ctx);
        let g = glob::Pattern::new(&pattern)
            .map_err(|e| ToolError::Execution(format!("bad glob '{pattern}': {e}")))?;

        let mut matches = Vec::new();
        find_recursive(&full, &full, &g, max_depth, 0, &mut matches).await?;
        matches.sort();
        Ok(ToolResult::text(serde_json::to_string_pretty(&matches).unwrap_or_default()))
    }
}

#[async_trait]
impl Tool for FsGrep {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_grep".into(),
            description: "Search file contents with regex".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern" },
                    "path": { "type": "string", "description": "Root directory" },
                    "include": { "type": "string", "description": "Optional glob filter" }
                },
                "required": ["pattern"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let pattern = req_str(&arguments, "pattern")?;
        let root = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let include = arguments.get("include").and_then(|v| v.as_str());
        let full = resolve_path(root, ctx);
        let re = regex::Regex::new(&pattern)
            .map_err(|e| ToolError::Execution(format!("bad regex '{pattern}': {e}")))?;
        let include_glob = include.and_then(|p| glob::Pattern::new(p).ok());

        let mut matches = Vec::new();
        grep_recursive(&full, &re, &include_glob, &mut matches).await?;
        Ok(ToolResult::text(serde_json::to_string_pretty(&matches).unwrap_or_default()))
    }
}

#[async_trait]
impl Tool for FsInfo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_info".into(),
            description: "Get file/directory metadata".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file or directory" }
                },
                "required": ["path"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let path = req_str(&arguments, "path")?;
        let full = resolve_path(&path, ctx);
        let meta = fs::metadata(&full)
            .await
            .map_err(|e| ToolError::Execution(format!("stat {path}: {e}")))?;

        let kind = if meta.is_dir() { "dir" } else if meta.is_symlink() { "symlink" } else { "file" };

        Ok(ToolResult::text(serde_json::to_string_pretty(&serde_json::json!({
            "path": path,
            "kind": kind,
            "size": meta.len(),
            "modified": meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
            "created": meta.created().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
            "readonly": meta.permissions().readonly(),
        })).unwrap_or_default()))
    }
}

struct ShellExec;

#[async_trait]
impl Tool for ShellExec {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell_exec".into(),
            description: "Execute a shell command and return stdout + stderr output".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default 30)" }
                },
                "required": ["command"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let command = req_str(&arguments, "command")?;
        let timeout_secs = arguments.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);

        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(&ctx.working_dir)
                .output(),
        )
        .await
        .map_err(|_| ToolError::Execution(format!("command timed out after {timeout_secs}s: {command}")))?
        .map_err(|e| ToolError::Execution(format!("spawn: {e}")))?;

        let mut text = String::new();
        if !output.stdout.is_empty() {
            text.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            if !text.is_empty() { text.push('\n'); }
            text.push_str(&String::from_utf8_lossy(&output.stderr));
        }

        if output.status.success() {
            Ok(ToolResult::text(text))
        } else {
            let code = output.status.code().map_or("?".into(), |c| c.to_string());
            Ok(ToolResult::error(format!("exit code {code}:\n{text}")))
        }
    }
}

struct WebFetch;

#[async_trait]
impl Tool for WebFetch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".into(),
            description: "Fetch a URL and return its content as text (markdown preferred, HTML otherwise)".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default 15)" }
                },
                "required": ["url"]
            }),
            origin: ToolOrigin::BuiltIn,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(&self, arguments: serde_json::Value, _ctx: &ToolExecutionContext) -> Result<ToolResult, ToolError> {
        let url = req_str(&arguments, "url")?;
        let timeout_secs = arguments.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(15);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .user_agent("aaBot/0.1")
            .build()
            .map_err(|e| ToolError::Execution(format!("build client: {e}")))?;

        let response = client.get(&url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ToolError::Execution(format!("timeout after {timeout_secs}s: {url}"))
                } else if e.is_connect() {
                    ToolError::Execution(format!("connection failed: {url} — {e}"))
                } else {
                    ToolError::Execution(format!("fetch {url}: {e}"))
                }
            })?;

        let status = response.status();
        let text = response.text()
            .await
            .map_err(|e| ToolError::Execution(format!("read body: {e}")))?;

        if status.is_success() {
            let truncated = if text.len() > 100_000 {
                let t: String = text.chars().take(100_000).collect();
                format!("{t}\n\n...(truncated, total {} chars)", text.len())
            } else {
                text
            };
            Ok(ToolResult::text(truncated))
        } else {
            Ok(ToolResult::error(format!("HTTP {status}:\n{text}")))
        }
    }
}

fn resolve_path(path: &str, ctx: &ToolExecutionContext) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    if p.is_absolute() { p.to_path_buf() } else { std::path::Path::new(&ctx.working_dir).join(p) }
}

fn req_str(args: &serde_json::Value, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| ToolError::Execution(format!("Missing required '{key}'")))
}

#[derive(serde::Serialize)]
struct DirEntry {
    name: String,
    path: String,
    kind: String,
    size: u64,
    modified: String,
}

async fn list_entries(dir: std::path::PathBuf, recursive: bool) -> Result<Vec<DirEntry>, ToolError> {
    let mut entries = Vec::new();
    let mut rd = fs::read_dir(&dir)
        .await
        .map_err(|e| ToolError::Execution(format!("ls {:?}: {}", dir, e)))?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let meta = entry.metadata().await.map_err(|e| ToolError::Execution(e.to_string()))?;
        let kind = if meta.is_dir() { "dir" } else if meta.is_symlink() { "symlink" } else { "file" };
        let modified = meta.modified().ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs()).unwrap_or(0);

        entries.push(DirEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            kind: kind.to_owned(),
            size: meta.len(),
            modified: modified.to_string(),
        });

        if recursive && meta.is_dir() {
            entries.extend(Box::pin(list_entries(entry.path(), true)).await?);
        }
    }
    Ok(entries)
}

async fn find_recursive(
    root: &std::path::Path,
    dir: &std::path::Path,
    g: &glob::Pattern,
    max_depth: Option<u64>,
    depth: u64,
    results: &mut Vec<String>,
) -> Result<(), ToolError> {
    if let Some(max) = max_depth { if depth > max { return Ok(()); } }
    let mut rd = fs::read_dir(dir).await
        .map_err(|e| ToolError::Execution(format!("find {:?}: {}", dir, e)))?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
        if g.matches(&rel) || g.matches(&path.to_string_lossy()) { results.push(rel); }
        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            Box::pin(find_recursive(root, &path, g, max_depth, depth + 1, results)).await?;
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct GrepMatch {
    file: String,
    line: usize,
    content: String,
}

async fn grep_recursive(
    dir: &std::path::Path,
    re: &regex::Regex,
    include: &Option<glob::Pattern>,
    results: &mut Vec<GrepMatch>,
) -> Result<(), ToolError> {
    let mut rd = fs::read_dir(dir).await
        .map_err(|e| ToolError::Execution(format!("grep {:?}: {}", dir, e)))?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            Box::pin(grep_recursive(&path, re, include, results)).await?;
        } else if entry.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
            let rel = path.to_string_lossy().to_string();
            if let Some(ref g) = include {
                if !g.matches(&rel) && !g.matches(&path.file_name().unwrap_or_default().to_string_lossy()) { continue; }
            }
            if let Ok(content) = fs::read_to_string(&path).await {
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) { results.push(GrepMatch { file: rel.clone(), line: i + 1, content: line.to_owned() }); }
                }
            }
        }
    }
    Ok(())
}
