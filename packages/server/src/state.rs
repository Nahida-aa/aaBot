use std::sync::Arc;

use tokio::sync::RwLock;

pub(crate) type Registry = Arc<RwLock<aa_kernel::ToolRegistry>>;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) registry: Registry,
    pub(crate) resolved: aa_config::ResolvedConfig,
    pub(crate) mcp_count: usize,
}
