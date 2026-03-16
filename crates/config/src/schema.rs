use serde::{Deserialize, Serialize};

/// Top-level application configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub gateway: GatewayConfig,
    pub agent: AgentConfig,
    #[serde(default)]
    pub channels: ChannelsConfig,
    pub logging: Option<LoggingConfig>,
}

/// Gateway server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatewayConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub auth_token: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    18789
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            auth_token: None,
        }
    }
}

/// Agent / LLM configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub system_prompt: Option<String>,
}

/// All channel configurations.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChannelsConfig {
    pub feishu: Option<FeishuConfig>,
}

/// Feishu (Lark) bot channel configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeishuConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub app_id: String,
    pub app_secret: String,
    pub encrypt_key: Option<String>,
    pub verification_token: Option<String>,
    /// "feishu" | "lark" | custom base URL.
    #[serde(default = "default_domain")]
    pub domain: Option<String>,
    /// "websocket" | "webhook".
    #[serde(default = "default_connection_mode")]
    pub connection_mode: Option<String>,
    pub webhook_port: Option<u16>,
    /// Whether group messages require @mention.
    #[serde(default = "default_true_opt")]
    pub require_mention: Option<bool>,
}

fn default_true() -> bool {
    true
}

fn default_true_opt() -> Option<bool> {
    Some(true)
}

fn default_domain() -> Option<String> {
    Some("feishu".to_string())
}

fn default_connection_mode() -> Option<String> {
    Some("websocket".to_string())
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl FeishuConfig {
    /// Resolve the API base URL from the domain setting.
    pub fn base_url(&self) -> String {
        let domain = self.domain.as_deref().unwrap_or("feishu");
        match domain {
            "feishu" => "https://open.feishu.cn/open-apis".to_string(),
            "lark" => "https://open.larksuite.com/open-apis".to_string(),
            custom if custom.starts_with("http") => custom.to_string(),
            other => format!("https://{other}/open-apis"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feishu_base_url_defaults() {
        let cfg = FeishuConfig {
            enabled: true,
            app_id: String::new(),
            app_secret: String::new(),
            encrypt_key: None,
            verification_token: None,
            domain: Some("feishu".to_string()),
            connection_mode: None,
            webhook_port: None,
            require_mention: None,
        };
        assert_eq!(cfg.base_url(), "https://open.feishu.cn/open-apis");
    }

    #[test]
    fn feishu_base_url_lark() {
        let cfg = FeishuConfig {
            enabled: true,
            app_id: String::new(),
            app_secret: String::new(),
            encrypt_key: None,
            verification_token: None,
            domain: Some("lark".to_string()),
            connection_mode: None,
            webhook_port: None,
            require_mention: None,
        };
        assert_eq!(cfg.base_url(), "https://open.larksuite.com/open-apis");
    }
}
