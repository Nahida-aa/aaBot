本地对比项目索引见 `.agent/REFS.md`，克隆于 `/home/aa/repos/learn_ls/`

所有代码统一放在 `packages/` 下，不分 Rust/TS：
- `packages/kernel/`             微内核
- `packages/core/`               核心类型（Extension trait 等）
- `packages/extensions/`         扩展系统 + WASM 加载器
- `packages/extension-sdk/`      扩展公开 API
- `packages/cli/`                CLI 入口
- `packages/server/`             HTTP/API 服务
- `packages/web/`                Solid 
- `packages/desktop/`            Tauri 桌面
- `packages/tui/`                TUI（Solid, 最后开发）
- `packages/function-tools/`     LLM function calling 工具集
- `packages/web/`                Solid.js 前端（Vite, TypeScript）
- `packages/llm/`                LLM Provider 实现（OpenAI 兼容）
- `packages/extension-*/`        内置扩展

📁 **目录名 vs Cargo.toml name 规则**：目录名用简洁形式（如 `kernel/`），`package.name` 用 `aa-` 前缀（如 `aa-kernel`）。不要混用。

纠错记录见 `.agent/CORRECTIONS.md`。
