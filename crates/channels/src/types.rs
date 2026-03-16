use serde::{Deserialize, Serialize};

/// Type of chat (direct message or group).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    Direct,
    Group,
}

/// An inbound message received from a channel.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Message ID (used for deduplication).
    pub id: String,
    /// Channel identifier (e.g. "feishu").
    pub channel: String,
    /// Chat / conversation ID.
    pub chat_id: String,
    /// Direct message or group chat.
    pub chat_type: ChatType,
    /// Sender's platform-specific ID.
    pub sender_id: String,
    /// Sender's display name, if available.
    pub sender_name: Option<String>,
    /// Text content of the message.
    pub text: String,
    /// ID of the message being replied to, if any.
    pub reply_to: Option<String>,
    /// Whether the bot was mentioned in this message.
    pub mentions_bot: bool,
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
}

/// An outbound message to send through a channel.
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    /// Target channel identifier.
    pub channel: String,
    /// Target chat / conversation ID.
    pub chat_id: String,
    /// Text content to send.
    pub text: String,
    /// Message ID to reply to, if any.
    pub reply_to: Option<String>,
}

/// Channel health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelHealth {
    pub connected: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}
