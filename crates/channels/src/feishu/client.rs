use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::auth::FeishuAuth;

/// Feishu REST API client.
pub struct FeishuClient {
    auth: Arc<FeishuAuth>,
    base_url: String,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct ApiResponse {
    code: i32,
    msg: String,
    data: Option<serde_json::Value>,
}

impl FeishuClient {
    pub fn new(auth: Arc<FeishuAuth>, base_url: String) -> Self {
        Self {
            auth,
            base_url,
            http: reqwest::Client::new(),
        }
    }

    /// Send a text message to a chat.
    pub async fn send_text(&self, chat_id: &str, text: &str) -> Result<String> {
        let content = serde_json::json!({ "text": text });
        self.send_message(chat_id, "text", &content).await
    }

    /// Reply to a specific message with text.
    pub async fn reply_text(&self, message_id: &str, text: &str) -> Result<String> {
        let token = self.auth.get_token().await?;
        let url = format!("{}/im/v1/messages/{}/reply", self.base_url, message_id);
        let content = serde_json::json!({ "text": text });

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "msg_type": "text",
                "content": content.to_string(),
            }))
            .send()
            .await
            .context("Failed to reply message")?;

        let body: ApiResponse = resp.json().await?;
        if body.code != 0 {
            anyhow::bail!("Feishu reply error (code {}): {}", body.code, body.msg);
        }

        Ok(body
            .data
            .and_then(|d| {
                d.get("message_id")
                    .and_then(|v| v.as_str().map(String::from))
            })
            .unwrap_or_default())
    }

    /// Send a rich card message.
    pub async fn send_card(&self, chat_id: &str, card_json: &serde_json::Value) -> Result<String> {
        self.send_message(chat_id, "interactive", card_json).await
    }

    /// Low-level message send.
    async fn send_message(
        &self,
        chat_id: &str,
        msg_type: &str,
        content: &serde_json::Value,
    ) -> Result<String> {
        let token = self.auth.get_token().await?;
        let url = format!("{}/im/v1/messages?receive_id_type=chat_id", self.base_url);

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "receive_id": chat_id,
                "msg_type": msg_type,
                "content": content.to_string(),
            }))
            .send()
            .await
            .context("Failed to send message")?;

        let body: ApiResponse = resp.json().await?;
        if body.code != 0 {
            anyhow::bail!("Feishu send error (code {}): {}", body.code, body.msg);
        }

        Ok(body
            .data
            .and_then(|d| {
                d.get("message_id")
                    .and_then(|v| v.as_str().map(String::from))
            })
            .unwrap_or_default())
    }

    /// Get a user's display name by open_id.
    pub async fn get_user_name(&self, open_id: &str) -> Result<String> {
        let token = self.auth.get_token().await?;
        let url = format!(
            "{}/contact/v3/users/{}?user_id_type=open_id",
            self.base_url, open_id
        );

        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .context("Failed to get user info")?;

        let body: ApiResponse = resp.json().await?;
        if body.code != 0 {
            return Ok(open_id.to_string());
        }

        Ok(body
            .data
            .and_then(|d| {
                d.get("user")
                    .and_then(|u| u.get("name").and_then(|n| n.as_str().map(String::from)))
            })
            .unwrap_or_else(|| open_id.to_string()))
    }
}
