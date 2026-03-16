use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::{Mutex, mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use opencrab_config::FeishuConfig;

use super::client::FeishuClient;
use crate::types::{ChatType, InboundMessage};

/// Run the Feishu WebSocket event loop.
pub async fn run_event_loop(
    config: &FeishuConfig,
    _client: &FeishuClient,
    tx: mpsc::Sender<InboundMessage>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let seen_ids: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let require_mention = config.require_mention.unwrap_or(true);

    // Get WebSocket endpoint URL from Feishu.
    let ws_url = get_ws_endpoint(config).await?;
    tracing::info!("Connecting to Feishu WebSocket: {ws_url}");

    loop {
        // Connect to WebSocket.
        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .context("Failed to connect to Feishu WebSocket")?;

        tracing::info!("Feishu WebSocket connected");

        let (_, mut read) = ws_stream.split();

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::info!("Feishu event loop shutting down");
                        return Ok(());
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = handle_ws_message(
                                &text, &tx, &seen_ids, require_mention,
                            ).await {
                                tracing::warn!("Failed to handle Feishu message: {e}");
                            }
                        }
                        Some(Ok(Message::Ping(_))) => {
                            // tungstenite auto-responds with pong
                        }
                        Some(Ok(Message::Close(_))) => {
                            tracing::warn!("Feishu WebSocket closed by server, reconnecting...");
                            break;
                        }
                        Some(Err(e)) => {
                            tracing::error!("Feishu WebSocket error: {e}, reconnecting...");
                            break;
                        }
                        None => {
                            tracing::warn!("Feishu WebSocket stream ended, reconnecting...");
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Backoff before reconnecting.
        tracing::info!("Reconnecting to Feishu WebSocket in 3s...");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

/// Get the WebSocket endpoint URL from Feishu API.
async fn get_ws_endpoint(config: &FeishuConfig) -> Result<String> {
    let base_url = config.base_url();
    let http = reqwest::Client::new();

    // Get tenant_access_token.
    let token_url = format!("{base_url}/auth/v3/tenant_access_token/internal");
    let token_resp = http
        .post(&token_url)
        .json(&serde_json::json!({
            "app_id": config.app_id,
            "app_secret": config.app_secret,
        }))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct TokenResp {
        tenant_access_token: Option<String>,
        code: i32,
        msg: String,
    }
    let token_body: TokenResp = token_resp.json().await?;
    if token_body.code != 0 {
        anyhow::bail!("Feishu auth failed: {}", token_body.msg);
    }
    let token = token_body
        .tenant_access_token
        .context("No token in response")?;

    // Request WebSocket endpoint.
    let ws_endpoint_url = format!("{base_url}/callback/ws/endpoint");
    let ws_resp = http
        .post(&ws_endpoint_url)
        .bearer_auth(&token)
        .json(&serde_json::json!({}))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct WsResp {
        code: i32,
        msg: String,
        data: Option<WsData>,
    }
    #[derive(Deserialize)]
    struct WsData {
        #[serde(rename = "URL")]
        url: Option<String>,
    }

    let ws_body: WsResp = ws_resp.json().await?;
    if ws_body.code != 0 {
        anyhow::bail!("Feishu WS endpoint error: {}", ws_body.msg);
    }

    ws_body
        .data
        .and_then(|d| d.url)
        .context("No WebSocket URL in response")
}

/// Parse and dispatch a single WebSocket message.
async fn handle_ws_message(
    raw: &str,
    tx: &mpsc::Sender<InboundMessage>,
    seen_ids: &Arc<Mutex<HashSet<String>>>,
    require_mention: bool,
) -> Result<()> {
    let value: serde_json::Value = serde_json::from_str(raw)?;

    // Check if this is an event we care about.
    let header = value.get("header").context("Missing header")?;
    let event_type = header
        .get("event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if event_type != "im.message.receive_v1" {
        tracing::debug!("Ignoring Feishu event: {event_type}");
        return Ok(());
    }

    let event = value.get("event").context("Missing event body")?;
    let message = event.get("message").context("Missing message")?;
    let sender = event.get("sender").context("Missing sender")?;

    // Extract message ID for deduplication.
    let message_id = message
        .get("message_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Dedup check.
    {
        let mut seen = seen_ids.lock().await;
        if seen.contains(&message_id) {
            return Ok(());
        }
        seen.insert(message_id.clone());
        // Keep set bounded.
        if seen.len() > 10_000 {
            seen.clear();
        }
    }

    // Extract fields.
    let chat_id = message
        .get("chat_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let chat_type_str = message
        .get("chat_type")
        .and_then(|v| v.as_str())
        .unwrap_or("p2p");

    let chat_type = match chat_type_str {
        "group" => ChatType::Group,
        _ => ChatType::Direct,
    };

    let sender_id = sender
        .get("sender_id")
        .and_then(|s| s.get("open_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let message_type = message
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text");

    // Only handle text and post messages in M1.
    if message_type != "text" && message_type != "post" {
        tracing::debug!("Ignoring non-text message type: {message_type}");
        return Ok(());
    }

    // Parse content JSON.
    let content_str = message
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");

    let content: serde_json::Value = serde_json::from_str(content_str).unwrap_or_default();
    let mut text = content
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Check mentions.
    let mentions = message
        .get("mentions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mentions_bot = !mentions.is_empty();

    // Strip mention placeholders from text (e.g. "@_user_1 ").
    for mention in &mentions {
        if let Some(key) = mention.get("key").and_then(|v| v.as_str()) {
            text = text.replace(key, "").trim().to_string();
        }
    }

    // Group chat require_mention filter.
    if chat_type == ChatType::Group && require_mention && !mentions_bot {
        return Ok(());
    }

    if text.is_empty() {
        return Ok(());
    }

    let timestamp = header
        .get("create_time")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let inbound = InboundMessage {
        id: message_id,
        channel: "feishu".to_string(),
        chat_id,
        chat_type,
        sender_id,
        sender_name: None,
        text,
        reply_to: None,
        mentions_bot,
        timestamp,
    };

    tx.send(inbound).await.context("Inbound channel closed")?;
    Ok(())
}
