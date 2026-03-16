use anyhow::Result;
use tokio::sync::mpsc;

use crate::traits::Channel;
use crate::types::{InboundMessage, OutboundMessage};

/// Manages the lifecycle of all registered channels.
pub struct ChannelManager {
    channels: Vec<Box<dyn Channel>>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Option<mpsc::Receiver<InboundMessage>>,
}

impl ChannelManager {
    /// Create a new channel manager with a bounded inbound queue.
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self {
            channels: Vec::new(),
            inbound_tx: tx,
            inbound_rx: Some(rx),
        }
    }

    /// Register a channel.
    pub fn register(&mut self, channel: Box<dyn Channel>) {
        tracing::info!(
            "Registered channel: {} ({})",
            channel.display_name(),
            channel.id()
        );
        self.channels.push(channel);
    }

    /// Start all registered channels.
    pub async fn start_all(&self) -> Result<()> {
        for channel in &self.channels {
            tracing::info!("Starting channel: {}", channel.id());
            channel.start(self.inbound_tx.clone()).await?;
        }
        Ok(())
    }

    /// Stop all registered channels.
    pub async fn stop_all(&self) -> Result<()> {
        for channel in &self.channels {
            tracing::info!("Stopping channel: {}", channel.id());
            if let Err(e) = channel.stop().await {
                tracing::error!("Error stopping channel {}: {e}", channel.id());
            }
        }
        Ok(())
    }

    /// Send a message through the appropriate channel.
    pub async fn send(&self, msg: OutboundMessage) -> Result<()> {
        let channel = self
            .channels
            .iter()
            .find(|c| c.id() == msg.channel)
            .ok_or_else(|| anyhow::anyhow!("Channel not found: {}", msg.channel))?;
        channel.send(msg).await
    }

    /// Take the inbound message receiver (can only be called once).
    pub fn take_inbound_rx(&mut self) -> Option<mpsc::Receiver<InboundMessage>> {
        self.inbound_rx.take()
    }

    /// Get health status of all channels.
    pub async fn health_all(&self) -> Vec<(String, crate::types::ChannelHealth)> {
        let mut results = Vec::new();
        for channel in &self.channels {
            let health = channel.health().await;
            results.push((channel.id().to_string(), health));
        }
        results
    }
}
