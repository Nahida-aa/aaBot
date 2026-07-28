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

## OpenTUI 版本 & JSX 机制

### `@opentui/*` 必须用 0.3.4（0.4.1 context 不传播）
- 原因：`@opentui/solid@0.4.1` 的 `mountSolidRoot` 使用 `get children() { return createComponent2(node, {}); }` 创建组件树，Solid context（`createContext`/`useContext`）不从用户定义的 provider（`ToastProvider`、`RouteProvider` 等）传播到子组件
- 0.3.4 的 `mountSolidRoot` 内部实现不同，context 能正常传播
- 同时 `solid-js` 保持 1.9.x（opencode 用 1.9.10，我们 1.9.12）

### JSX 转换机制（0.3.4 无 `jsx-runtime.js`）
- 0.3.4 没有 `jsx-runtime.js` 文件（只有 `jsx-runtime.d.ts` 提供类型）
- JSX 转换通过 **Bun 插件 + Babel** 完成，不是通过 `jsxImportSource` 的运行时导入
- **必须**有 `bunfig.toml`：`preload = ["@opentui/solid/preload"]`
- `preload.ts` 调用 `ensureSolidTransformPlugin()`，注册 `bun-plugin-solid`，拦截 `.tsx`/`.jsx` 文件，用 Babel (`babel-preset-solid` + `@babel/preset-typescript`) 将 JSX 转换为对 `@opentui/solid` 的 `h()`/`createComponent()` 调用
- `tsconfig.json` 的 `"jsxImportSource": "@opentui/solid"` 仅用于 TypeScript 类型检查和编辑器 IntelliSense

### 0.4.1 vs 0.3.4 文件差异
| 特性 | 0.4.1 | 0.3.4 |
|------|-------|-------|
| `jsx-runtime.js` | 有（从 solid-js 导出 `jsx`/`jsxDEV`） | 无（依赖 Babel 插件） |
| Context 传播 | ❌ 自定义 provider 不传播 | ✅ 正常 |
| `--conditions=browser` | 需要（防 SSR build） | 需要（需 Babel 插件） |
| `bunfig.toml` | 不需要（有 `jsx-runtime.js`） | **需要**（`preload = ["@opentui/solid/preload"]`） |
| 依赖 | 仅 `solid-js` | `@babel/core` + `babel-preset-solid` + `@babel/preset-typescript`（已在 package.json dependencies） |

## 待修复的问题

### Toast 不可见
- `render_toast`: Clear + Block::bordered(green) 叠加在渲染区中间，用户说看不到
- `eprintln!` stderr 调试已加（会直接打印到终端，不受 Ratatui 控制）
- 可能原因：Clear 后未 flush；渲染坐标超界；被后续 render 覆盖
- 需在真实终端确认 `render_toast` 坐标和渲染时机

### ~~复制后选区高亮不消失~~ ✅ 已修复
- 方案：Ratatui 自有选区系统（非终端原生 PRIMARY selection）
- **启用鼠标捕获**（`EnableMouseCapture`/`DisableMouseCapture` on enter/exit）
- **鼠标行为**：Down → 按消息选中 → Drag → 扩展范围 → Up → 自动复制 + 清选区 + toast
- **键盘行为**：`Ctrl+C`（有选区→复制选区，无选区→复制全部），`Ctrl+Y`（复制最后回复），`Escape`（清选区）
- **渲染**：选中消息 `Color::DarkGray` 背景
- **坐标映射**：`render_messages()` 构建 `msg_line_map: Vec<usize>`（每行→消息索引），鼠标事件通过 `(row - msg_area_y + scroll)` 查表
- **代价**：终端原生鼠标选中失效（不能拖选到别的应用）
- **文件**：`app.rs`（handle_mouse, copy_selected_messages, selected_range），`render.rs`（msg_line_map, 高亮渲染），`mod.rs`（鼠标事件路由）
