use serde::{Deserialize, Serialize};

/// Events pushed from the gateway to connected clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GatewayEvent {
    /// A complete chat message.
    #[serde(rename = "chat.message")]
    ChatMessage {
        text: String,
        channel: String,
        sender: String,
    },

    /// A streaming text delta.
    #[serde(rename = "chat.stream_delta")]
    StreamDelta { delta: String, done: bool },

    /// Channel connection status change.
    #[serde(rename = "channel.status")]
    ChannelStatus { channel: String, status: String },

    /// Gateway health heartbeat.
    #[serde(rename = "health")]
    Health { uptime_secs: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_stream_delta() {
        let event = GatewayEvent::StreamDelta {
            delta: "Hello".to_string(),
            done: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"chat.stream_delta\""));
        assert!(json.contains("\"delta\":\"Hello\""));
    }

    #[test]
    fn deserialize_chat_message() {
        let json = r#"{"type":"chat.message","text":"hi","channel":"feishu","sender":"user1"}"#;
        let event: GatewayEvent = serde_json::from_str(json).unwrap();
        match event {
            GatewayEvent::ChatMessage { text, channel, .. } => {
                assert_eq!(text, "hi");
                assert_eq!(channel, "feishu");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_health() {
        let event = GatewayEvent::Health { uptime_secs: 42 };
        let json = serde_json::to_string(&event).unwrap();
        let back: GatewayEvent = serde_json::from_str(&json).unwrap();
        match back {
            GatewayEvent::Health { uptime_secs } => assert_eq!(uptime_secs, 42),
            _ => panic!("wrong variant"),
        }
    }
}
