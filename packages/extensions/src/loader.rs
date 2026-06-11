//! 扩展加载器。

use aa_core::extension::Extension;

/// 扩展清单——声明扩展元数据及可执行文件路径。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub kind: ExtensionKind,
    pub entry: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// 扩展类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    /// Rust 编译时代入的扩展。
    BuiltIn,
    /// WASM 插件（wasmtime 加载，Component Model）。
    Wasm,
    /// s5r 子进程插件（stdio IPC）。
    Subprocess,
}

/// 扩展加载器。
pub trait ExtensionLoader: Send + Sync {
    fn load_extension(&self, manifest: &Manifest) -> Box<dyn Extension>;
}
