use std::sync::Arc;

use aa_function_tools::FsToolProvider;
use aa_kernel::tool_provider::*;

fn setup(dir: &std::path::Path) -> ToolExecutionContext {
    std::fs::write(dir.join("hello.txt"), b"Hello world!\n").unwrap();
    std::fs::write(dir.join("numbers.txt"), b"one\ntwo\nthree\nfour\n").unwrap();
    std::fs::create_dir(dir.join("subdir")).unwrap();
    std::fs::write(dir.join("subdir/nested.txt"), b"nested file\n").unwrap();
    ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.to_string_lossy().to_string(),
    }
}

fn find_tool(provider: &FsToolProvider, name: &str) -> Arc<dyn Tool> {
    let scope = ToolProviderScope::new(".");
    provider
        .tools(&scope)
        .into_iter()
        .find(|t| t.definition().name == name)
        .unwrap_or_else(|| panic!("tool {name} not found"))
}

fn assert_tool_ok(result: Result<ToolResult, ToolError>) -> ToolResult {
    let r = result.unwrap();
    assert!(!r.is_error, "expected success, got error: {}", r.content);
    r
}

#[test]
fn test_fs_read() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_read");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result =
        assert_tool_ok(rt.block_on(tool.execute(serde_json::json!({"path": "hello.txt"}), &ctx)));
    assert_eq!(result.content, "Hello world!\n");
}

#[test]
fn test_fs_write() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_write");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = assert_tool_ok(rt.block_on(tool.execute(
        serde_json::json!({"path": "new.txt", "content": "test content"}),
        &ctx,
    )));
    assert!(result.content.contains("written"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new.txt")).unwrap(),
        "test content"
    );
}

#[test]
fn test_fs_write_append() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_write");
    let rt = tokio::runtime::Runtime::new().unwrap();
    assert_tool_ok(rt.block_on(tool.execute(
        serde_json::json!({"path": "log.txt", "content": "line 1\n"}),
        &ctx,
    )));
    assert_tool_ok(rt.block_on(tool.execute(
        serde_json::json!({"path": "log.txt", "content": "line 2\n", "append": true}),
        &ctx,
    )));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("log.txt")).unwrap(),
        "line 1\nline 2\n"
    );
}

#[test]
fn test_fs_ls() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_ls");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = assert_tool_ok(rt.block_on(tool.execute(serde_json::json!({"path": "."}), &ctx)));
    let entries: Vec<serde_json::Value> = serde_json::from_str(&result.content).unwrap();
    let names: Vec<&str> = entries.iter().filter_map(|e| e["name"].as_str()).collect();
    assert!(names.contains(&"hello.txt"));
    assert!(names.contains(&"numbers.txt"));
    assert!(names.contains(&"subdir"));
}

#[test]
fn test_fs_ls_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_ls");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = assert_tool_ok(
        rt.block_on(tool.execute(serde_json::json!({"path": ".", "recursive": true}), &ctx)),
    );
    let entries: Vec<serde_json::Value> = serde_json::from_str(&result.content).unwrap();
    let names: Vec<&str> = entries.iter().filter_map(|e| e["name"].as_str()).collect();
    assert!(names.contains(&"nested.txt"));
}

#[test]
fn test_fs_find() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_find");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result =
        assert_tool_ok(rt.block_on(tool.execute(serde_json::json!({"pattern": "**/*.txt"}), &ctx)));
    assert!(result.content.contains("hello.txt") || result.content.contains("numbers.txt"));
}

#[test]
fn test_fs_grep() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_grep");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result =
        assert_tool_ok(rt.block_on(tool.execute(serde_json::json!({"pattern": "three"}), &ctx)));
    let matches: Vec<serde_json::Value> = serde_json::from_str(&result.content).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["content"].as_str(), Some("three"));
}

#[test]
fn test_fs_info() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_info");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result =
        assert_tool_ok(rt.block_on(tool.execute(serde_json::json!({"path": "hello.txt"}), &ctx)));
    let info: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(info["kind"], "file");
    assert!(info["size"].as_u64().unwrap() > 0);
}

#[test]
fn test_fs_info_dir() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = setup(dir.path());
    let tool = find_tool(&FsToolProvider, "fs_info");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result =
        assert_tool_ok(rt.block_on(tool.execute(serde_json::json!({"path": "subdir"}), &ctx)));
    let info: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(info["kind"], "dir");
}

#[test]
fn test_fs_read_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "fs_read");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(tool.execute(serde_json::json!({"path": "nope.txt"}), &ctx));
    assert!(result.is_err());
}

#[test]
fn test_shell_exec_echo() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "shell_exec");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = assert_tool_ok(
        rt.block_on(tool.execute(serde_json::json!({"command": "echo hello world"}), &ctx)),
    );
    assert_eq!(result.content.trim(), "hello world");
}

#[test]
fn test_shell_exec_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "shell_exec");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(tool.execute(serde_json::json!({"command": "exit 42"}), &ctx))
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("exit code 42"));
}

#[test]
fn test_shell_exec_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "shell_exec");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(tool.execute(
        serde_json::json!({"command": "sleep 10", "timeout_secs": 1}),
        &ctx,
    ));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timed out"));
}

#[test]
fn test_web_fetch_definition() {
    let provider = FsToolProvider;
    let scope = ToolProviderScope::new(".");
    let tools = provider.tools(&scope);
    let tool = tools
        .iter()
        .find(|t| t.definition().name == "web_fetch")
        .unwrap();
    let def = tool.definition();
    assert_eq!(def.name, "web_fetch");
    assert!(def.parameters["properties"]["url"].is_object());
    assert!(
        def.parameters["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("url"))
    );
}

#[test]
fn test_fs_read_range() {
    let dir = tempfile::tempdir().unwrap();
    let content = (1..=20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.path().join("test.txt"), &content).unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "fs_read_range");
    let rt = tokio::runtime::Runtime::new().unwrap();

    let result = assert_tool_ok(rt.block_on(tool.execute(
        serde_json::json!({"path": "test.txt", "start": 5, "end": 8}),
        &ctx,
    )));
    assert!(result.content.contains("line 5"));
    assert!(result.content.contains("line 8"));
    assert!(result.content.contains("4/20"));
}

#[test]
fn test_fs_edit_simple() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "hello world\nfoo bar\n").unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "fs_edit");
    let rt = tokio::runtime::Runtime::new().unwrap();

    assert_tool_ok(rt.block_on(tool.execute(
        serde_json::json!({"path": "test.txt", "old": "world", "new": "there"}),
        &ctx,
    )));
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "hello there\nfoo bar\n");
}

#[test]
fn test_fs_edit_not_found() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "hello world\n").unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "fs_edit");
    let rt = tokio::runtime::Runtime::new().unwrap();

    let result = rt
        .block_on(tool.execute(
            serde_json::json!({"path": "test.txt", "old": "nope", "new": "x"}),
            &ctx,
        ))
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[test]
fn test_fs_edit_multiple_occurrences() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "foo\nbar\nfoo\n").unwrap();
    let ctx = ToolExecutionContext {
        session_id: "test".into(),
        working_dir: dir.path().to_string_lossy().to_string(),
    };
    let tool = find_tool(&FsToolProvider, "fs_edit");
    let rt = tokio::runtime::Runtime::new().unwrap();

    let result = rt
        .block_on(tool.execute(
            serde_json::json!({"path": "test.txt", "old": "foo", "new": "baz"}),
            &ctx,
        ))
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("2 occurrences"));
}

#[test]
fn test_fs_tool_provider_registration() {
    let provider = FsToolProvider;
    let scope = ToolProviderScope::new(".");
    let tools = provider.tools(&scope);
    let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
    assert_eq!(
        names,
        vec![
            "fs_read",
            "fs_write",
            "fs_ls",
            "fs_find",
            "fs_grep",
            "fs_info",
            "shell_exec",
            "web_fetch",
            "fs_read_range",
            "fs_edit"
        ]
    );
}
