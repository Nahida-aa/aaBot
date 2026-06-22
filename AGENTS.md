本地对比项目索引见 `.agent/REFS.md`，克隆于 `/home/aa/repos/learn_ls/`

类型检查: tsgo

所有代码统一放在 `packages/` 下，不分 Rust/TS：
- `packages/kernel/`             微内核
- `packages/core/`               核心类型（Extension trait 等）
- `packages/extensions/`         扩展系统 + WASM 加载器
- `packages/extension-sdk/`      扩展公开 API
- `packages/cli/`                主入口（`aa`=TUI, `aa run`=行模式, `aa tool`, `aa extension`）
- `packages/tui/`                OpenTUI + Solid.js TUI（Bun，参考 opencode 结构）
- `packages/server/`             HTTP/API 服务（axum, `/chat` SSE, `/health`, `/tools`）
- `packages/web/`                Solid.js Web 前端（Vite, TailwindCSS v4）
- `packages/desktop/`            Tauri 桌面
- `packages/function-tools/`     LLM function calling 工具集
- `packages/llm/`                LLM Provider 实现（OpenAI 兼容）
- `packages/ollama/`             Ollama 原生 Provider（/api/chat）
- `packages/config/`             LLM 配置系统（aa.json + AA_* env）
- `packages/session/`            共享对话循环（run_turn），事件驱动
- `packages/extension-*/`        内置扩展

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

## 已验证
- **Ollama 端到端**：`aa run --provider ollama --model gemma4:31b-cloud` 成功返回 "Hello"（462 prompt + 2 completion tokens）
- **TUI 编译通过**（无终端时 panic 属预期）
- **`Config::resolve()` 正确性**：cli 传 `Option`，不覆盖 provider 默认值（ollama→`http://localhost:11434`）
- **`RunArgs` 改为 `Option<String>`**：避免 clap default 覆盖 provider 特定默认值


纠错记录见 `.agent/CORRECTIONS.md`。
