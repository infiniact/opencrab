use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use axum::Router;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use tokio::sync::Mutex;

use opencrab_agent::engine::StatefulAgent;
use opencrab_channels::ChannelManager;
use opencrab_config::AppConfig;
use opencrab_protocol::rpc::{RpcError, RpcRequest, RpcResponse};

/// Shared application state.
pub struct AppState {
    pub agent: Arc<Mutex<StatefulAgent>>,
    pub channel_manager: Arc<ChannelManager>,
    pub start_time: Instant,
    pub config: AppConfig,
}

/// Start the HTTP + WebSocket server.
pub async fn start_server(state: Arc<AppState>) -> Result<()> {
    let addr = format!(
        "{}:{}",
        state.config.gateway.host, state.config.gateway.port
    );

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ws", get(ws_handler))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Gateway listening on {addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

/// GET /health — returns basic health info.
async fn health_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().as_secs();
    Json(serde_json::json!({
        "status": "ok",
        "uptime_secs": uptime,
    }))
}

/// GET /ws — upgrade to WebSocket for JSON-RPC communication.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_ws_connection(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    tracing::info!("WebSocket client connected");

    while let Some(msg) = ws_rx.next().await {
        let msg = match msg {
            Ok(WsMessage::Text(text)) => text,
            Ok(WsMessage::Close(_)) => break,
            Err(e) => {
                tracing::warn!("WebSocket error: {e}");
                break;
            }
            _ => continue,
        };

        // Parse JSON-RPC request.
        let request: RpcRequest = match serde_json::from_str(&msg) {
            Ok(req) => req,
            Err(e) => {
                let resp = RpcResponse::error(
                    None,
                    RpcError {
                        code: opencrab_protocol::errors::PARSE_ERROR,
                        message: format!("Invalid JSON: {e}"),
                        data: None,
                    },
                );
                let _ = ws_tx
                    .send(WsMessage::text(
                        serde_json::to_string(&resp).unwrap_or_default(),
                    ))
                    .await;
                continue;
            }
        };

        // Dispatch RPC method.
        let response = handle_rpc(&request, &state).await;
        let _ = ws_tx
            .send(WsMessage::text(
                serde_json::to_string(&response).unwrap_or_default(),
            ))
            .await;
    }

    tracing::info!("WebSocket client disconnected");
}

/// Dispatch a JSON-RPC request to the appropriate handler.
async fn handle_rpc(req: &RpcRequest, state: &Arc<AppState>) -> RpcResponse {
    match req.method.as_str() {
        "health" => {
            let uptime = state.start_time.elapsed().as_secs();
            RpcResponse::success(
                req.id.clone(),
                serde_json::json!({
                    "status": "ok",
                    "uptime_secs": uptime,
                }),
            )
        }

        "chat.send" => {
            let text = req
                .params
                .as_ref()
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if text.is_empty() {
                return RpcResponse::error(
                    req.id.clone(),
                    RpcError {
                        code: opencrab_protocol::errors::INVALID_PARAMS,
                        message: "Missing 'text' parameter".to_string(),
                        data: None,
                    },
                );
            }

            let mut agent = state.agent.lock().await;
            match agent.prompt(text).await {
                Ok(reply) => {
                    RpcResponse::success(req.id.clone(), serde_json::json!({ "reply": reply }))
                }
                Err(e) => RpcResponse::error(
                    req.id.clone(),
                    RpcError {
                        code: opencrab_protocol::errors::AGENT_ERROR,
                        message: format!("Agent error: {e}"),
                        data: None,
                    },
                ),
            }
        }

        "channels.status" => {
            let health = state.channel_manager.health_all().await;
            let channels: Vec<serde_json::Value> = health
                .into_iter()
                .map(|(id, h)| {
                    serde_json::json!({
                        "id": id,
                        "connected": h.connected,
                        "latency_ms": h.latency_ms,
                        "error": h.error,
                    })
                })
                .collect();

            RpcResponse::success(req.id.clone(), serde_json::json!(channels))
        }

        "config.get" => {
            let key = req
                .params
                .as_ref()
                .and_then(|p| p.get("key"))
                .and_then(|v| v.as_str());

            let config_value = serde_json::to_value(&state.config).unwrap_or_default();

            let result = match key {
                Some(k) => {
                    // Support dotted keys like "agent.model".
                    let mut current = &config_value;
                    for part in k.split('.') {
                        current = current.get(part).unwrap_or(&serde_json::Value::Null);
                    }
                    current.clone()
                }
                None => config_value,
            };

            RpcResponse::success(req.id.clone(), result)
        }

        other => RpcResponse::error(
            req.id.clone(),
            RpcError {
                code: opencrab_protocol::errors::METHOD_NOT_FOUND,
                message: format!("Method not found: {other}"),
                data: None,
            },
        ),
    }
}
