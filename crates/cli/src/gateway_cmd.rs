use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use opencrab_agent::engine::StatefulAgent;
use opencrab_channels::ChannelManager;
use opencrab_channels::feishu::FeishuChannel;
use opencrab_channels::types::{ChatType, OutboundMessage};
use opencrab_config::load_config;

/// Run the full gateway server (channels + HTTP/WS + message loop).
pub async fn run_gateway(config_path: &str, port_override: Option<u16>) -> Result<()> {
    // Expand ~ in path.
    let expanded = if config_path.starts_with('~') {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        home.join(config_path.trim_start_matches("~/"))
    } else {
        std::path::PathBuf::from(config_path)
    };

    let mut config = load_config(Some(&expanded))?;

    // Apply port override.
    if let Some(port) = port_override {
        config.gateway.port = port;
    }

    // Initialize logging.
    let log_level = config
        .logging
        .as_ref()
        .map(|l| l.level.as_str())
        .unwrap_or("info");

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .init();

    tracing::info!("OpenCrab Gateway starting...");

    // Create agent.
    let agent = StatefulAgent::new(&config.agent).context("Failed to create agent")?;
    let agent = Arc::new(Mutex::new(agent));

    // Create channel manager.
    let mut channel_manager = ChannelManager::new(256);

    // Register Feishu channel if enabled.
    if let Some(ref feishu_config) = config.channels.feishu
        && feishu_config.enabled
    {
        let channel =
            FeishuChannel::new(feishu_config.clone()).context("Failed to create Feishu channel")?;
        channel_manager.register(Box::new(channel));
    }

    // Start channels.
    channel_manager.start_all().await?;

    // Take the inbound receiver before wrapping in Arc.
    let inbound_rx = channel_manager.take_inbound_rx();

    let channel_manager = Arc::new(channel_manager);

    // Start message processing loop.
    if let Some(mut rx) = inbound_rx {
        let agent_clone = agent.clone();
        let cm_clone = channel_manager.clone();
        let require_mention = config
            .channels
            .feishu
            .as_ref()
            .and_then(|f| f.require_mention)
            .unwrap_or(true);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let agent = agent_clone.clone();
                let cm = cm_clone.clone();

                tokio::spawn(async move {
                    // Group chat require_mention filter (double-check at gateway level).
                    if msg.chat_type == ChatType::Group && !msg.mentions_bot && require_mention {
                        return;
                    }

                    tracing::info!(
                        "Received message from {} in {}: {}",
                        msg.sender_id,
                        msg.chat_id,
                        &msg.text[..msg.text.len().min(100)]
                    );

                    let mut agent = agent.lock().await;
                    match agent.prompt(&msg.text).await {
                        Ok(reply) => {
                            let outbound = OutboundMessage {
                                channel: msg.channel,
                                chat_id: msg.chat_id,
                                text: reply,
                                reply_to: Some(msg.id),
                            };
                            if let Err(e) = cm.send(outbound).await {
                                tracing::error!("Failed to send reply: {e}");
                            }
                        }
                        Err(e) => {
                            tracing::error!("Agent error: {e}");
                        }
                    }
                });
            }
        });
    }

    // Build app state.
    let state = Arc::new(opencrab_gateway::server::AppState {
        agent: agent.clone(),
        channel_manager: channel_manager.clone(),
        start_time: Instant::now(),
        config: config.clone(),
    });

    // Print status.
    let health = channel_manager.health_all().await;
    for (id, h) in &health {
        let status = if h.connected {
            "connected"
        } else {
            "disconnected"
        };
        tracing::info!("Channel {id}: {status}");
    }

    tracing::info!(
        "Listening on {}:{}",
        config.gateway.host,
        config.gateway.port
    );

    // Handle Ctrl+C for graceful shutdown.
    let cm_shutdown = channel_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Shutting down...");
        cm_shutdown.stop_all().await.ok();
        std::process::exit(0);
    });

    // Start HTTP/WS server (blocks).
    opencrab_gateway::server::start_server(state).await?;

    Ok(())
}
