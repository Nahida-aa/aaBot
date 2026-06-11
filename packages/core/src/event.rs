use serde::{Deserialize, Serialize};

/// 扩展可订阅的宿主事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionEvent {
    /// 宿主启动完成。
    HostReady,
    /// 宿主即将关闭。
    HostShutdown,
    /// 新 session 创建。
    SessionCreated,
    /// Session 销毁。
    SessionDestroyed,
    /// 消息即将发送给 LLM。
    BeforeModelCall,
    /// LLM 响应已返回。
    AfterModelCall,
    /// 工具即将执行。
    BeforeToolExecute,
    /// 工具执行完成。
    AfterToolExecute,
    /// LLM 生成的文本已回复给用户。
    MessageSent,
    /// 另一条扩展发射的自定义事件。
    Custom(&'static str),
}
