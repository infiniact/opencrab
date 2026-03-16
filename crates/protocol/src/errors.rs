use thiserror::Error;

/// Standard JSON-RPC 2.0 error codes.
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

/// Application-specific error codes (reserved range: -32000 to -32099).
pub const AUTH_REQUIRED: i32 = -32000;
pub const CHANNEL_NOT_FOUND: i32 = -32001;
pub const AGENT_ERROR: i32 = -32002;
pub const CONFIG_ERROR: i32 = -32003;

/// Gateway error type.
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Authentication required")]
    AuthRequired,

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl GatewayError {
    /// Convert to a JSON-RPC error code.
    pub fn code(&self) -> i32 {
        match self {
            Self::AuthRequired => AUTH_REQUIRED,
            Self::MethodNotFound(_) => METHOD_NOT_FOUND,
            Self::InvalidParams(_) => INVALID_PARAMS,
            Self::ChannelNotFound(_) => CHANNEL_NOT_FOUND,
            Self::AgentError(_) => AGENT_ERROR,
            Self::ConfigError(_) => CONFIG_ERROR,
            Self::Internal(_) => INTERNAL_ERROR,
        }
    }

    /// Convert to an `RpcError`.
    pub fn to_rpc_error(&self) -> crate::rpc::RpcError {
        crate::rpc::RpcError {
            code: self.code(),
            message: self.to_string(),
            data: None,
        }
    }
}
