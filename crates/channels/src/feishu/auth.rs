use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::Mutex;

/// Manages Feishu tenant_access_token lifecycle (auto-refresh).
pub struct FeishuAuth {
    app_id: String,
    app_secret: String,
    base_url: String,
    http: reqwest::Client,
    cached: Mutex<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: Instant,
}

#[derive(Deserialize)]
struct TokenResponse {
    code: i32,
    msg: String,
    tenant_access_token: Option<String>,
    expire: Option<u64>,
}

impl FeishuAuth {
    pub fn new(app_id: String, app_secret: String, base_url: String) -> Self {
        Self {
            app_id,
            app_secret,
            base_url,
            http: reqwest::Client::new(),
            cached: Mutex::new(None),
        }
    }

    /// Get a valid tenant_access_token, refreshing if needed.
    pub async fn get_token(&self) -> Result<String> {
        let mut cached = self.cached.lock().await;

        // Return cached token if still valid (with 5-minute buffer).
        if let Some(ref c) = *cached
            && c.expires_at > Instant::now() + Duration::from_secs(300)
        {
            return Ok(c.token.clone());
        }

        // Fetch a new token.
        let url = format!("{}/auth/v3/tenant_access_token/internal", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "app_id": self.app_id,
                "app_secret": self.app_secret,
            }))
            .send()
            .await
            .context("Failed to request tenant_access_token")?;

        let body: TokenResponse = resp
            .json()
            .await
            .context("Failed to parse token response")?;

        if body.code != 0 {
            anyhow::bail!("Feishu auth error (code {}): {}", body.code, body.msg);
        }

        let token = body
            .tenant_access_token
            .context("Missing tenant_access_token in response")?;
        let expire_secs = body.expire.unwrap_or(7200);

        *cached = Some(CachedToken {
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expire_secs),
        });

        tracing::debug!("Feishu token refreshed, expires in {expire_secs}s");
        Ok(token)
    }
}

/// Create a thread-safe auth handle.
pub fn new_shared(app_id: String, app_secret: String, base_url: String) -> Arc<FeishuAuth> {
    Arc::new(FeishuAuth::new(app_id, app_secret, base_url))
}
