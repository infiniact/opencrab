use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use serde::Deserialize;
use tokio::sync::{Mutex, mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use opencrab_config::FeishuConfig;

use super::client::FeishuClient;
use crate::types::{ChatType, InboundMessage};

// ── Protobuf frame types (matching Feishu's pbbp2.proto) ──────────────────

/// Protobuf Header: `message Header { required string key = 1; required string value = 2; }`
#[derive(Clone, PartialEq, ProstMessage)]
struct PbHeader {
    #[prost(string, required, tag = "1")]
    key: String,
    #[prost(string, required, tag = "2")]
    value: String,
}

/// Protobuf Frame: the top-level WebSocket binary message.
#[derive(Clone, PartialEq, ProstMessage)]
struct PbFrame {
    #[prost(uint64, required, tag = "1")]
    seq_id: u64,
    #[prost(uint64, required, tag = "2")]
    log_id: u64,
    #[prost(int32, required, tag = "3")]
    service: i32,
    #[prost(int32, required, tag = "4")]
    method: i32,
    #[prost(message, repeated, tag = "5")]
    headers: Vec<PbHeader>,
    #[prost(string, optional, tag = "6")]
    payload_encoding: Option<String>,
    #[prost(string, optional, tag = "7")]
    payload_type: Option<String>,
    #[prost(bytes = "vec", optional, tag = "8")]
    payload: Option<Vec<u8>>,
    #[prost(string, optional, tag = "9")]
    log_id_new: Option<String>,
}

impl PbFrame {
    fn get_header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|h| h.key == key)
            .map(|h| h.value.as_str())
    }
}

const FRAME_TYPE_CONTROL: i32 = 0;
const FRAME_TYPE_DATA: i32 = 1;

// ── Main event loop ────────────────────────────────────────────────────────

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

        let (mut write, mut read) = ws_stream.split();

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
                        Some(Ok(Message::Binary(data))) => {
                            match handle_binary_frame(
                                &data, &tx, &seen_ids, require_mention,
                            ).await {
                                Ok(Some(response_frame)) => {
                                    let mut buf = Vec::new();
                                    if response_frame.encode(&mut buf).is_ok() {
                                        if let Err(e) = write.send(Message::Binary(buf.into())).await {
                                            tracing::warn!("Failed to send response frame: {e}");
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    tracing::warn!("Failed to handle Feishu binary frame: {e}");
                                }
                            }
                        }
                        Some(Ok(Message::Text(text))) => {
                            // Fallback: some messages may come as text.
                            tracing::debug!("Received text message (unexpected): {}", &text[..text.len().min(200)]);
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

/// Handle a binary protobuf frame from Feishu WebSocket.
/// Returns an optional response frame to send back.
async fn handle_binary_frame(
    data: &[u8],
    tx: &mpsc::Sender<InboundMessage>,
    seen_ids: &Arc<Mutex<HashSet<String>>>,
    require_mention: bool,
) -> Result<Option<PbFrame>> {
    let frame = PbFrame::decode(data).context("Failed to decode protobuf frame")?;

    match frame.method {
        FRAME_TYPE_CONTROL => {
            handle_control_frame(&frame);
            Ok(None)
        }
        FRAME_TYPE_DATA => handle_data_frame(&frame, tx, seen_ids, require_mention).await,
        other => {
            tracing::debug!("Ignoring unknown frame type: {other}");
            Ok(None)
        }
    }
}

/// Handle control frames (pong, config updates).
fn handle_control_frame(frame: &PbFrame) {
    let msg_type = frame.get_header("type").unwrap_or("");
    match msg_type {
        "pong" => {
            tracing::debug!("Received pong from Feishu");
        }
        other => {
            tracing::debug!("Received control frame type: {other}");
        }
    }
}

/// Handle data frames (events). Returns a response frame to ACK.
async fn handle_data_frame(
    frame: &PbFrame,
    tx: &mpsc::Sender<InboundMessage>,
    seen_ids: &Arc<Mutex<HashSet<String>>>,
    require_mention: bool,
) -> Result<Option<PbFrame>> {
    let msg_type = frame.get_header("type").unwrap_or("");
    let message_id = frame.get_header("message_id").unwrap_or("");

    let payload = match &frame.payload {
        Some(p) if !p.is_empty() => p,
        _ => {
            tracing::debug!("Data frame with empty payload, type={msg_type}");
            return Ok(None);
        }
    };

    if msg_type == "event" {
        let payload_str = std::str::from_utf8(payload)
            .context("Payload is not valid UTF-8")?;
        tracing::debug!("Received event payload: {}", &payload_str[..payload_str.len().min(500)]);

        if let Err(e) = handle_event_payload(payload_str, tx, seen_ids, require_mention).await {
            tracing::warn!("Failed to handle event (msg_id={message_id}): {e}");
        }
    } else {
        tracing::debug!("Ignoring data frame type: {msg_type}");
    }

    // Build ACK response frame.
    let resp_payload = serde_json::to_vec(&serde_json::json!({"code": 200}))
        .unwrap_or_default();
    let mut resp_frame = frame.clone();
    resp_frame.payload = Some(resp_payload);
    Ok(Some(resp_frame))
}

/// Parse the JSON event payload and dispatch to inbound channel.
async fn handle_event_payload(
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

// ── WebSocket endpoint negotiation ─────────────────────────────────────────

/// Get the WebSocket endpoint URL from Feishu API.
///
/// Follows the official Feishu Go SDK protocol: send AppID/AppSecret directly
/// to `{domain}/callback/ws/endpoint` (no tenant_access_token needed).
async fn get_ws_endpoint(config: &FeishuConfig) -> Result<String> {
    // The WS endpoint uses the domain root, NOT the /open-apis prefix.
    let domain_raw = config.domain.as_deref().unwrap_or("feishu");
    let domain = match domain_raw {
        "feishu" => "https://open.feishu.cn".to_string(),
        "lark" => "https://open.larksuite.com".to_string(),
        custom if custom.starts_with("http") => custom.trim_end_matches('/').to_string(),
        other => format!("https://{other}"),
    };

    let ws_endpoint_url = format!("{domain}/callback/ws/endpoint");
    tracing::debug!("Requesting Feishu WS endpoint from: {ws_endpoint_url}");

    let http = reqwest::Client::new();
    let ws_resp = http
        .post(&ws_endpoint_url)
        .header("locale", "zh")
        .json(&serde_json::json!({
            "AppID": config.app_id,
            "AppSecret": config.app_secret,
        }))
        .send()
        .await
        .context("Failed to request WS endpoint")?;

    let ws_status = ws_resp.status();
    let ws_text = ws_resp
        .text()
        .await
        .context("Failed to read WS endpoint response body")?;
    tracing::debug!("Feishu WS endpoint response ({ws_status}): {ws_text}");

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

    let ws_body: WsResp = serde_json::from_str(&ws_text)
        .context(format!("Failed to parse WS endpoint response: {ws_text}"))?;
    if ws_body.code != 0 {
        anyhow::bail!(
            "Feishu WS endpoint error (code {}): {}",
            ws_body.code,
            ws_body.msg
        );
    }

    ws_body
        .data
        .and_then(|d| d.url)
        .context("No WebSocket URL in response")
}
