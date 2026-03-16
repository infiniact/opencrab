use serde::Serialize;

/// Events emitted during agent processing.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// A text delta from the LLM.
    #[serde(rename = "text_delta")]
    TextDelta { delta: String },

    /// A tool call is starting.
    #[serde(rename = "tool_call_start")]
    ToolCallStart {
        name: String,
        args: serde_json::Value,
    },

    /// A tool call completed.
    #[serde(rename = "tool_call_end")]
    ToolCallEnd { name: String, result: String },

    /// The agent has finished processing.
    #[serde(rename = "done")]
    Done { full_text: String },

    /// An error occurred.
    #[serde(rename = "error")]
    Error { message: String },
}
