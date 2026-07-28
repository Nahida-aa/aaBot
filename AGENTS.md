<!-- intent-skills:start -->

## Skill Loading

Before editing files for a substantial task:

- Run `bunx @tanstack/intent@latest list` from the workspace root to see available local skills.
- If a listed skill matches the task, run `bunx @tanstack/intent@latest load <package>#<skill>` before changing files.
- Use the loaded `SKILL.md` guidance while making the change.
- Monorepos: when working across packages, run the skill check from the workspace root and prefer the local skill for the package being changed.
- Multiple matches: prefer the most specific local skill for the package or concern you are changing; load additional skills only when the task spans multiple packages or concerns.

<!-- intent-skills:end -->

所有代码统一放在 `packages/` 下，不分 Rust/TS：

- `packages/kernel/` 微内核
- `packages/core/` 核心类型（Extension trait 等）
- `packages/extensions/` 扩展系统 + WASM 加载器
- `packages/extension-sdk/` 扩展公开 API
- `packages/cli/` 主入口（`aa`=TUI, `aa run`=行模式, `aa tool`, `aa extension`）
- `packages/tui/` OpenTUI + Solid.js TUI（Bun，参考 opencode 结构）
- `packages/server/` HTTP/API 服务（axum, `/chat` SSE, `/health`, `/tools`）
- `packages/app/` Solid.js 前端 + Tauri 桌面（Web API 通信，无 IPC）
- `packages/function-tools/` LLM function calling 工具集
- `packages/llm/` LLM Provider 实现（OpenAI 兼容）
- `packages/ollama/` Ollama 原生 Provider（/api/chat）
- `packages/config/` LLM 配置系统（aa.json + AA_* env）
- `packages/session/` 共享对话循环（run_turn），事件驱动
- `packages/extension-*/` 内置扩展

**Rust edition**: 2024（`set_var`/`remove_var` 需 `unsafe` 块）

📁 **目录名 vs Cargo.toml name 规则**：目录名用简洁形式（如 `kernel/`），`package.name` 用 `aa-` 前缀（如 `aa-kernel`）。不要混用。

## 架构决策

- **Server 层不单独拆 C/S**，内嵌在 `packages/server/` 作为同进程调用边界（参考 opencode CLI→`Server.Default().app.fetch()`）
- CLI/TUI 直接调 `session::run_turn()`，不走网络
- `packages/server/` 可选暴露 HTTP/SSE（`aa serve`）给远程客户端（`--attach`）

## TUI 架构（OpenTUI + Solid.js）

- TUI 是独立 Bun 进程（`packages/tui/`），通过 localhost HTTP/SSE 连 Rust server
- `aa` 无参数 → 启动 Rust server + 启动 Bun TUI
- `aa serve` → 只启动 server（headless）
- `aa attach <url>` → 连接到远程 server
- **服务器生命周期**：TUI 启动时自动检查 `/health`，未运行则 spawn server，TUI 退出时 kill server
- **UI 组织参考 opencode**：`routes/`, `component/`, `context/`, `util/`
- **SSE 协议**：AG-UI JSON over SSE（TEXT_MESSAGE_START/CONTENT/END, TOOL_CALL_START/ARGS/END, RUN_STARTED/FINISHED/ERROR）
- **`@opentui/*` 必须 0.3.4**（0.4.1 Solid context 不传播），JSX 通过 Babel 插件转换，需要 `bunfig.toml` + `--conditions=browser`（见 CORRECTIONS.md）

## App 架构（Web + Tauri 共用）

- `packages/app/` 是统一的前端包，同时支持 Web 和 Tauri 桌面
- 开发：`bun dev`（纯 web）或 `bun run tauri dev`（桌面）
- 前端通过 HTTP/SSE 与 Rust server 通信，不走 Tauri IPC
- `src-tauri/` 只做 sidecar 启动和窗口管理，不包含业务逻辑
- Web API（文件对话框、剪贴板）在 Tauri webview 中正常工作

## 已验证

- **Ollama 端到端**：`aa run --provider ollama --model gemma4:31b-cloud` 成功返回 "Hello"（462 prompt + 2 completion tokens）
- **TUI 编译通过**（无终端时 panic 属预期）
- **`Config::resolve()` 正确性**：cli 传 `Option`，不覆盖 provider 默认值（ollama→`http://localhost:11434`）
- **`RunArgs` 改为 `Option<String>`**：避免 clap default 覆盖 provider 特定默认值

纠错记录见 `.agent/CORRECTIONS.md`。
