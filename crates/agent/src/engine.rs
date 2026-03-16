use std::path::PathBuf;

use anyhow::{Context, Result};
use rig::completion::{Chat, Message};
use rig::prelude::CompletionClient;
use rig::providers::{anthropic, openai};

use opencrab_config::AgentConfig;

use crate::tools::{BashTool, ReadFileTool};

/// A stateful agent that accumulates conversation history on top of Rig's Agent.
pub struct StatefulAgent {
    /// The underlying rig agent (type-erased via enum dispatch).
    inner: AgentInner,
    /// Accumulated chat history.
    messages: Vec<Message>,
}

/// Type-erased wrapper for provider-specific Agent types.
enum AgentInner {
    OpenAI(rig::agent::Agent<openai::responses_api::ResponsesCompletionModel<reqwest::Client>>),
    Anthropic(rig::agent::Agent<anthropic::completion::CompletionModel<reqwest::Client>>),
}

impl StatefulAgent {
    /// Create a new stateful agent from configuration.
    pub fn new(config: &AgentConfig) -> Result<Self> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let system_prompt = config
            .system_prompt
            .clone()
            .unwrap_or_else(|| "You are a helpful assistant.".to_string());

        let bash_tool = BashTool { cwd: cwd.clone() };
        let read_file_tool = ReadFileTool { cwd: cwd.clone() };

        let inner = match config.provider.as_str() {
            "openai" => {
                let client = openai::Client::new(&config.api_key)
                    .context("Failed to create OpenAI client")?;
                let mut builder = client.agent(&config.model).preamble(&system_prompt);
                if let Some(temp) = config.temperature {
                    builder = builder.temperature(temp);
                }
                if let Some(max_tokens) = config.max_tokens {
                    builder = builder.max_tokens(max_tokens as u64);
                }
                let agent = builder.tool(bash_tool).tool(read_file_tool).build();
                AgentInner::OpenAI(agent)
            }
            "anthropic" => {
                let client = anthropic::Client::builder()
                    .api_key(&config.api_key)
                    .build()
                    .context("Failed to create Anthropic client")?;
                let mut builder = client.agent(&config.model).preamble(&system_prompt);
                if let Some(temp) = config.temperature {
                    builder = builder.temperature(temp);
                }
                if let Some(max_tokens) = config.max_tokens {
                    builder = builder.max_tokens(max_tokens as u64);
                }
                let agent = builder.tool(bash_tool).tool(read_file_tool).build();
                AgentInner::Anthropic(agent)
            }
            other => anyhow::bail!("Unknown provider: {other}. Supported: openai, anthropic"),
        };

        Ok(Self {
            inner,
            messages: Vec::new(),
        })
    }

    /// Process a user message and return the full reply.
    pub async fn prompt(&mut self, input: &str) -> Result<String> {
        let history = self.messages.clone();

        let response: String = match &self.inner {
            AgentInner::OpenAI(agent) => Chat::chat(agent, input, history)
                .await
                .context("OpenAI completion failed")?,
            AgentInner::Anthropic(agent) => Chat::chat(agent, input, history)
                .await
                .context("Anthropic completion failed")?,
        };

        // Record both messages in history.
        self.messages.push(Message::user(input));
        self.messages.push(Message::assistant(&response));

        Ok(response)
    }

    /// Clear conversation history.
    pub fn reset(&mut self) {
        self.messages.clear();
    }

    /// Get the current message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}
