use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RpcId>,
}

/// JSON-RPC 2.0 request/response identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    Num(i64),
    Str(String),
}

/// JSON-RPC 2.0 response frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcRequest {
    /// Create a new JSON-RPC 2.0 request.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>, id: RpcId) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
            id: Some(id),
        }
    }

    /// Create a notification (no id, no response expected).
    pub fn notification(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
            id: None,
        }
    }
}

impl RpcResponse {
    /// Create a success response.
    pub fn success(id: Option<RpcId>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<RpcId>, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_request() {
        let req = RpcRequest::new(
            "chat.send",
            Some(serde_json::json!({"text": "hello"})),
            RpcId::Num(1),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"chat.send\""));
    }

    #[test]
    fn deserialize_request() {
        let json = r#"{"jsonrpc":"2.0","method":"health","id":1}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "health");
        assert!(matches!(req.id, Some(RpcId::Num(1))));
    }

    #[test]
    fn serialize_success_response() {
        let resp = RpcResponse::success(Some(RpcId::Num(1)), serde_json::json!({"status": "ok"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn serialize_error_response() {
        let resp = RpcResponse::error(
            Some(RpcId::Num(1)),
            RpcError {
                code: -32600,
                message: "Invalid request".to_string(),
                data: None,
            },
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(!json.contains("\"result\""));
    }
}
