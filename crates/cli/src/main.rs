use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use opencrab_agent::engine::StatefulAgent;
use opencrab_config::load_config;

mod gateway_cmd;

#[derive(Parser)]
#[command(
    name = "opencrab",
    about = "OpenCrab — Multi-channel AI gateway",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the gateway server.
    Gateway {
        #[command(subcommand)]
        cmd: GatewayCmd,
    },
    /// Chat with the agent directly (no gateway needed).
    Chat {
        /// Message to send.
        message: String,
        /// Path to config file.
        #[arg(long)]
        config: Option<String>,
    },
    /// Manage configuration.
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Check channel status.
    Channels {
        #[command(subcommand)]
        cmd: ChannelsCmd,
    },
}

#[derive(Subcommand)]
enum GatewayCmd {
    /// Start the gateway.
    Run {
        /// Path to config file.
        #[arg(long, default_value = "~/.opencrab/config.toml")]
        config: String,
        /// Override listening port.
        #[arg(long)]
        port: Option<u16>,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Get a config value.
    Get {
        /// Dotted config key (e.g. "agent.model"). Omit for full config.
        key: Option<String>,
    },
    /// Set a config value.
    Set {
        /// Dotted config key.
        key: String,
        /// Value to set.
        value: String,
    },
}

#[derive(Subcommand)]
enum ChannelsCmd {
    /// Show channel connection status.
    Status {
        /// Gateway port to connect to (overrides config).
        #[arg(long)]
        port: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Gateway { cmd } => match cmd {
            GatewayCmd::Run { config, port } => {
                gateway_cmd::run_gateway(&config, port).await?;
            }
        },

        Commands::Chat { message, config } => {
            run_chat(&message, config.as_deref()).await?;
        }

        Commands::Config { cmd } => match cmd {
            ConfigCmd::Get { key } => {
                run_config_get(key.as_deref()).await?;
            }
            ConfigCmd::Set { key, value } => {
                eprintln!("Config set not yet implemented (key={key}, value={value})");
            }
        },

        Commands::Channels { cmd } => match cmd {
            ChannelsCmd::Status { port } => {
                run_channels_status(port).await?;
            }
        },
    }

    Ok(())
}

/// Direct chat with agent (no gateway).
async fn run_chat(message: &str, config_path: Option<&str>) -> Result<()> {
    let config = load_config(config_path.map(std::path::Path::new))?;

    // Minimal logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("warn"))
        .init();

    let mut agent = StatefulAgent::new(&config.agent)?;

    let reply = agent.prompt(message).await?;
    println!("{reply}");

    Ok(())
}

/// Get config values.
async fn run_config_get(key: Option<&str>) -> Result<()> {
    let config = load_config(None)?;
    let value = serde_json::to_value(&config)?;

    let result = match key {
        Some(k) => {
            let mut current = &value;
            for part in k.split('.') {
                current = current.get(part).unwrap_or(&serde_json::Value::Null);
            }
            current.clone()
        }
        None => value,
    };

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// Show channel status by connecting to running gateway.
async fn run_channels_status(port_override: Option<u16>) -> Result<()> {
    use futures::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let config = load_config(None)?;
    let port = port_override.unwrap_or(config.gateway.port);
    let ws_url = format!("ws://{}:{}/ws", config.gateway.host, port);

    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .context("Failed to connect to gateway. Is it running?")?;

    // Send channels.status RPC.
    let req = opencrab_protocol::rpc::RpcRequest::new(
        "channels.status",
        None,
        opencrab_protocol::rpc::RpcId::Num(1),
    );
    let msg = Message::text(serde_json::to_string(&req)?);

    use futures::SinkExt;
    ws.send(msg).await?;

    // Read response.
    if let Some(Ok(Message::Text(text))) = ws.next().await {
        let resp: opencrab_protocol::rpc::RpcResponse = serde_json::from_str(&text)?;
        if let Some(result) = resp.result {
            if let Some(channels) = result.as_array() {
                println!("{:<12} {:<12} {:<10}", "CHANNEL", "STATUS", "LATENCY");
                println!("{}", "-".repeat(36));
                for ch in channels {
                    let id = ch.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let connected = ch
                        .get("connected")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let status = if connected {
                        "connected"
                    } else {
                        "disconnected"
                    };
                    let latency = ch
                        .get("latency_ms")
                        .and_then(|v| v.as_u64())
                        .map(|ms| format!("{ms}ms"))
                        .unwrap_or_else(|| "-".to_string());
                    println!("{:<12} {:<12} {:<10}", id, status, latency);
                }
            }
        } else if let Some(error) = resp.error {
            eprintln!("Error: {}", error.message);
        }
    }

    Ok(())
}
