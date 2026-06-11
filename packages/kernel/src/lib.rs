//! aaBot 微内核。
//!
//! 拥有运行时级的注册表和宿主组合契约。具体工具、扩展加载器、
//! 服务器、CLI 从此 crate 外部注册自身。

pub mod tool_pack;
pub mod tool_registry;

pub use tool_pack::{ToolPack, ToolPackScope};
pub use tool_registry::ToolRegistry;

use std::sync::Arc;

/// 可组合的内核表面，供宿主组装运行时能力。
#[derive(Clone, Default)]
pub struct Kernel {
    tool_packs: Arc<[Arc<dyn ToolPack>]>,
}

impl Kernel {
    pub fn builder() -> KernelBuilder {
        KernelBuilder::default()
    }

    pub fn tool_packs(&self) -> &[Arc<dyn ToolPack>] {
        &self.tool_packs
    }

    pub fn build_tool_registry(&self, scope: &ToolPackScope<'_>) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for pack in self.tool_packs() {
            for tool in pack.tools(scope) {
                registry.register(tool);
            }
        }
        registry
    }
}

/// 可嵌入 aaBot 内核的构建器。
#[derive(Default)]
#[allow(dead_code)]
pub struct KernelBuilder {
    tool_packs: Vec<Arc<dyn ToolPack>>,
}

impl KernelBuilder {
    pub fn with_tool_pack(mut self, pack: Arc<dyn ToolPack>) -> Self {
        self.tool_packs.push(pack);
        self
    }

    pub fn build(self) -> Kernel {
        Kernel {
            tool_packs: Arc::from(self.tool_packs),
        }
    }
}
