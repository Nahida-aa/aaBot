# 纠错记录 & 关键技术细节

## 架构理解纠正

### ToolPack vs Extension
- 我一开始认为「内置扩展 = ToolPack 不 = Extension」是错的
- 正确划分：`ToolPack` = LLM function calling 工具集（fs 工具、shell 工具，无生命周期），`Extension` = 插件系统（MCP、Skills、Memory，有 lifecycle/hooks/config）
- astrcodey 也这样分：`tools/` 存 builtin tools（read/write/glob/grep/shell），`extension-*` 存插件
- `packages/function-tools/` 现在是 ToolPack，不是 Extension

### extension-fs → function-tools 改名
- 原名 `extension-fs` 暗示它是插件，其实是函数工具集
- 改名 `function-tools`，配套 `aa-function-tools`，内部 `FsToolPack` struct

### PGlite vs SQLite
- 我之前写成了 SQLite，正确是 PGlite（WASM 编译的轻量 PostgreSQL）
- PGlite 在插件层，kernel 保持存储无关

### LLM Provider
- `LlmProvider` 不是插件，是内置平台能力
- 原因：调用频率极高、无生命周期、内核直接依赖

## Rust / wasmtime 技术细节

### `wasmtime::Error` 不实现 `std::error::Error`
- 不能直接 `?` 转 `anyhow::Error`，需 `.map_err(|e| anyhow::anyhow!(e))` 或 `.to_string()`

### WIT `result` 语法
- `result<string, string>` 写作 `(result string (error string))`，不是 `(result string string)`

### `bindgen!()` 无参用法
- 默认解析 `wit/` 目录
- 生成的模块名 = WIT 包名 `aa:extension` → `extension_world`，`ExtensionWorld` 结构体
- 导出的 interface accessor：`instance.aa_extension_plugin()`（不是 `instance.plugin()`）

### WASM ABI 模式 (WAT Component Model)
- **`() -> string`**：core function 返回 `i32`，值是内存中 (ptr, len) pair 的地址（8 字节）
- **`(string x 4) -> result<string, string>`**：core function 取 8 个 `i32` 参数，返回 `i32`，指向 retptr flat representation：[discriminant: i32][ptr: i32][len: i32]（共 12 字节）
- `canon lift` 不会注入额外 retptr 参数到 core function signature

### WAT 数据段
- hex byte 数 != 实际字符串长度；必须手动计算解码后的字节数记入 (ptr, len) pair

## function-tools 实现细节

### fs_write append 模式
- 使用 `spawn_blocking` + `std::fs::OpenOptions` + `sync_all()` —— tokio 的异步 fs write 有缓冲不确定性问题

### tokio runtime 在测试中
- `Runtime::new()` 需要 `rt` + `rt-multi-thread` feature
- 递归 async fn 必须 `Box::pin` 包裹

## LLM Provider 实现细节

- `packages/llm/` 不是 `extension-llm`，LLM provider 是内置平台能力
- 收到工具调用后先转 `ModelRequest` 发给 LLM，LLM 返回 tool_calls 后逐条执行，结果塞回 messages 循环
- OpenAI 兼容 API 要求 assistant 消息中 content 为 null（不是空字符串）当 tool_calls 存在
- SSE 解析：按 `\n` 分割，找 `data: ` 前缀，`[DONE]` 终止

## extension-mcp 实现细节

- JSON-RPC 2.0 over stdio 子进程
- 握手：`initialize` → `notifications/initialized` → `tools/list`
- 工具名 `{server_name}__{tool_name}`（如 `weather__get_forecast`）
- `McpClient` 用 `kill_on_drop(true)` + `Option<Child>` + `impl Drop` 清理子进程
- 构造函数同步连接（`block_on` + one-shot runtime），避免 `ToolPack::tools()` 异步问题
- 响应匹配：发送 request → 逐行读 stdout → 按 ID 匹配
