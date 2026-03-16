pub mod auth;
pub mod client;
pub mod events;
pub mod send;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};

use opencrab_config::FeishuConfig;

use crate::traits::Channel;
use crate::types::{ChannelHealth, InboundMessage, OutboundMessage};

/// Feishu (Lark) bot channel implementation.
pub struct FeishuChannel {
    config: FeishuConfig,
    client: Arc<client::FeishuClient>,
    connected: Arc<Mutex<bool>>,
    shutdown_tx: Arc<Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
}

impl FeishuChannel {
    /// Create a new Feishu channel from config.
    pub fn new(config: FeishuConfig) -> Result<Self> {
        let auth = Arc::new(auth::FeishuAuth::new(
            config.app_id.clone(),
            config.app_secret.clone(),
            config.base_url(),
        ));
        let client = Arc::new(client::FeishuClient::new(auth, config.base_url()));

        Ok(Self {
            config,
            client,
            connected: Arc::new(Mutex::new(false)),
            shutdown_tx: Arc::new(Mutex::new(None)),
        })
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    fn id(&self) -> &str {
        "feishu"
    }

    fn display_name(&self) -> &str {
        "Feishu"
    }

    async fn start(&self, tx: mpsc::Sender<InboundMessage>) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let config = self.config.clone();
        let client = self.client.clone();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            *connected.lock().await = true;
            if let Err(e) = events::run_event_loop(&config, &client, tx, shutdown_rx).await {
                tracing::error!("Feishu event loop error: {e}");
            }
            *connected.lock().await = false;
        });

        Ok(())
    }

    async fn send(&self, msg: OutboundMessage) -> Result<()> {
        send::send_message(&self.client, &msg).await
    }

    async fn health(&self) -> ChannelHealth {
        ChannelHealth {
            connected: *self.connected.lock().await,
            latency_ms: None,
            error: None,
        }
    }

    async fn stop(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(true);
        }
        Ok(())
    }
}
