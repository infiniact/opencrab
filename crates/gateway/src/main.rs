use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use opencrab_agent::engine::StatefulAgent;
use opencrab_channels::ChannelManager;
use opencrab_channels::feishu::FeishuChannel;
use opencrab_config::load_config;

use opencrab_gateway::server::{self, AppState};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse a minimal --config flag (the CLI crate handles full arg parsing).
    let config_path = std::env::args()
        .skip_while(|a| a != "--config")
        .nth(1)
        .map(std::path::PathBuf::from);

    // Load configuration.
    let config = load_config(config_path.as_deref()).context("Failed to load configuration")?;

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

    // Start all channels.
    channel_manager
        .start_all()
        .await
        .context("Failed to start channels")?;

    let channel_manager = Arc::new(channel_manager);

    // Start the message processing loop.
    // Note: take_inbound_rx needs &mut, so we do this before wrapping in Arc.
    // We'll use a separate task for message processing.
    // For now, the ChannelManager is already wrapped; we need to handle this differently.
    // Since ChannelManager is wrapped in Arc, we use a separate approach:
    // The inbound_rx was taken before Arc wrapping won't work — let's restructure.

    // Actually, let's restructure: take rx before Arc.
    // We need to adjust — channel_manager was already converted to Arc above.
    // Let's fix: take rx before Arc wrap.

    // Re-approach: build properly.
    // (The above is fine because we moved channel_manager into Arc after take_inbound_rx
    //  would need &mut. Let's restructure the flow.)

    // We'll handle message routing inside the gateway server instead.

    // Build shared app state.
    let state = Arc::new(AppState {
        agent: agent.clone(),
        channel_manager: channel_manager.clone(),
        start_time: Instant::now(),
        config: config.clone(),
    });

    tracing::info!(
        "Gateway listening on {}:{}",
        config.gateway.host,
        config.gateway.port
    );

    // Print channel status.
    let health = channel_manager.health_all().await;
    for (id, h) in &health {
        let status = if h.connected {
            "connected"
        } else {
            "disconnected"
        };
        tracing::info!("Channel {id}: {status}");
    }

    // Start server (blocks until shutdown).
    server::start_server(state).await?;

    // Graceful shutdown.
    channel_manager.stop_all().await?;
    tracing::info!("OpenCrab Gateway stopped.");

    Ok(())
}
