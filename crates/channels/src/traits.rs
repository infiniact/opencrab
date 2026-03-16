use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{ChannelHealth, InboundMessage, OutboundMessage};

/// A messaging channel that can receive and send messages.
#[async_trait]
pub trait Channel: Send + Sync + 'static {
    /// Unique channel identifier (e.g. "feishu").
    fn id(&self) -> &str;

    /// Human-readable display name.
    fn display_name(&self) -> &str;

    /// Start the channel, sending inbound messages through `tx`.
    async fn start(&self, tx: mpsc::Sender<InboundMessage>) -> Result<()>;

    /// Send an outbound message through this channel.
    async fn send(&self, msg: OutboundMessage) -> Result<()>;

    /// Check channel health / connectivity.
    async fn health(&self) -> ChannelHealth;

    /// Gracefully stop the channel.
    async fn stop(&self) -> Result<()>;
}
