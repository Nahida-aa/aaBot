本地对比项目索引见 `.agent/REFS.md`，克隆于 `/home/aa/repos/learn_ls/`

所有代码统一放在 `packages/` 下，不分 Rust/TS：
- `packages/kernel/`             微内核
- `packages/core/`               核心类型（Extension trait 等）
- `packages/extensions/`         扩展系统 + WASM 加载器
- `packages/extension-sdk/`      扩展公开 API
- `packages/cli/`                CLI 入口
- `packages/server/`             HTTP/API 服务
- `packages/web/`                Solid 前端
- `packages/desktop/`            Tauri 桌面
- `packages/tui/`                TUI（最后开发）
- `packages/extension-*/`        内置扩展

📁 **目录名 vs Cargo.toml name 规则**：目录名用简洁形式（如 `kernel/`），`package.name` 用 `aa-` 前缀（如 `aa-kernel`）。不要混用。

---

## 关键记录 & 纠错

### 架构决策
- 插件三层架构：Rust 内置 (零开销) → WASM Component Model (wasmtime, 沙箱) → s5r 子进程 (任何语言, 备选)
- LLM provider 是内置平台能力，非插件
- SQLite 推迟到插件层；kernel 保持存储无关

### 纠错记录
- **`wasmtime::Error` 不实现 `std::error::Error`**：不能直接 `?` 转 `anyhow::Error`，需 `.map_err(|e| anyhow::anyhow!(e))` 或 `.to_string()`
- **WIT `result` 语法**：`result<string, string>` 写作 `(result string (error string))`（不是 `(result string string)`）
- **`bindgen!()` 无参用法**：默认解析 `wit/` 目录。生成的模块名 = WIT 包名 `aa:extension` → `extension_world`，`ExtensionWorld` 结构体
- **导出的 interface accessor**：`instance.aa_extension_plugin()`（不是 `instance.plugin()`）

### WASM ABI 模式 (WAT Component Model)
- **`() -> string`**：core function 返回 `i32`，值是内存中 (ptr, len) pair 的地址（8 字节：ptr + len 各 4 字节）
- **`(string, string, string, string) -> result<string, string>`**：core function 取 8 个 `i32` 参数 (4 个 ptr+len pair)，返回 `i32`，指向 retptr flat representation：[discriminant: i32][ptr: i32][len: i32]（共 12 字节）。`canon lift` 的 `(realloc ...)` 用于适配器分配返回数据，核心函数只需返回指针
- `canon lift` 不会注入额外的 retptr 参数到 core function signature — 参数数量完全等于 flat 化的 lowered 参数

### Python/Rust 细节
- WAT `data` 段的 hex byte 数 != 实际字符串长度：必须手动计算解码后的字节数记入 (ptr, len) pair

### 内置扩展模式
- **内置扩展 = `ToolPack` 不 = `Extension`**：内置扩展（extension-fs, extension-mcp）实现 `ToolPack` trait，通过 `Kernel::builder().with_tool_pack()` 注册。不加 `Extension` 生命周期（start/stop/config），因为不需要。
- **未来需要生命周期的扩展**（skills engine, 持久连接服务）可以实现 `Extension` trait 通过 `ExtensionRegistry` 加载。
- MCP 适配器在构造函数里同步连接服务器（`block_on`），`tools()` 只是返回预缓存的 tool list。这样 `ToolPack::tools(&self)` 可以不 async。

### extension-fs 要点
- 6 个工具：`fs_read`, `fs_write`, `fs_ls`, `fs_find`, `fs_grep`, `fs_info`
- `fs_write` 的 append 模式使用 `spawn_blocking` + `std::fs::OpenOptions` + `sync_all()` 避免 tokio fs 的异步写缓冲问题
- `tokio::runtime::Runtime::new()` 需要 `rt` + `rt-multi-thread` feature；在测试中每个 test 创建自己的 runtime
- 递归 async fn 必须 `Box::pin` 包裹递归调用

### extension-mcp 要点
- JSON-RPC 2.0 over stdio 子进程
- 协议握手：`initialize` → `notifications/initialized` → `tools/list`
- 工具名格式 `{server_name}__{tool_name}`（如 `weather__get_forecast`）
- `McpClient` 使用 `kill_on_drop(true)` + `Option<Child>` + `impl Drop` 确保子进程清理
- 构造函数同步连接（`block_on` + one-shot runtime），`tools()` 返回预缓存结果
- 响应循环简单：发送 request → 逐行读取 stdout → 按 ID 匹配响应
