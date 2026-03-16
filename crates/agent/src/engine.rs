use std::path::PathBuf;

use anyhow::{Context, Result};
use async_trait::async_trait;
use rig::completion::{Chat, Message};
use rig::providers::{anthropic, openai};

use opencrab_config::AgentConfig;

use crate::tools::{BashTool, ReadFileTool};

/// Dyn-compatible wrapper for rig's `Chat` trait.
#[async_trait]
trait ChatAgent: Send + Sync {
    async fn chat(&self, input: &str, history: Vec<Message>) -> Result<String>;
}

/// Blanket implementation for any rig Agent that implements Chat.
#[async_trait]
impl<M> ChatAgent for rig::agent::Agent<M>
where
    M: rig::completion::CompletionModel + Send + Sync + 'static,
{
    async fn chat(&self, input: &str, history: Vec<Message>) -> Result<String> {
        Ok(Chat::chat(self, input, history).await?)
    }
}

/// A stateful agent that accumulates conversation history on top of Rig's Agent.
pub struct StatefulAgent {
    /// The underlying rig agent (type-erased via trait object).
    inner: Box<dyn ChatAgent>,
    /// Accumulated chat history.
    messages: Vec<Message>,
}

impl StatefulAgent {
    /// Create a new stateful agent from configuration.
    pub fn new(config: &AgentConfig) -> Result<Self> {
        let inner = build_agent(config)?;

        Ok(Self {
            inner,
            messages: Vec::new(),
        })
    }

    /// Process a user message and return the full reply.
    pub async fn prompt(&mut self, input: &str) -> Result<String> {
        let history = self.messages.clone();

        let response = self
            .inner
            .chat(input, history)
            .await
            .context("LLM completion failed")?;

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

/// Helper: create a standard provider client with api_key + optional base_url,
/// build an agent, and return it boxed.
macro_rules! standard_provider {
    ($config:expr, $system_prompt:expr, $cwd:expr, $client_type:ty, $label:expr) => {{
        let client: $client_type = if let Some(ref url) = $config.base_url {
            <$client_type>::builder()
                .api_key(&$config.api_key)
                .base_url(url)
                .build()
        } else {
            <$client_type>::builder()
                .api_key(&$config.api_key)
                .build()
        }
        .context(concat!("Failed to create ", $label, " client"))?;
        let agent = apply_agent_options(
            rig::prelude::CompletionClient::agent(&client, &$config.model)
                .preamble($system_prompt),
            $config,
            $cwd,
        );
        Ok(Box::new(agent))
    }};
}

/// Build a `Box<dyn ChatAgent>` from config, dispatching on provider name.
fn build_agent(config: &AgentConfig) -> Result<Box<dyn ChatAgent>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let system_prompt = config
        .system_prompt
        .clone()
        .unwrap_or_else(|| "You are a helpful assistant.".to_string());

    match config.provider.as_str() {
        // --- OpenAI (Responses API, default) ---
        "openai" => {
            standard_provider!(
                config,
                &system_prompt,
                cwd,
                openai::Client<reqwest::Client>,
                "OpenAI"
            )
        }

        // --- OpenAI Completions API (for compatible endpoints) ---
        "openai-compatible" | "openai-completions" => {
            let base_url = config
                .base_url
                .as_deref()
                .context("base_url is required for openai-compatible provider")?;
            let api_key = if config.api_key.is_empty() {
                "not-needed"
            } else {
                &config.api_key
            };
            let client = openai::Client::<reqwest::Client>::builder()
                .api_key(api_key)
                .base_url(base_url)
                .build()
                .context("Failed to create OpenAI-compatible client")?
                .completions_api();
            let agent = apply_agent_options(
                rig::prelude::CompletionClient::agent(&client, &config.model)
                    .preamble(&system_prompt),
                config,
                cwd,
            );
            Ok(Box::new(agent))
        }

        // --- Anthropic ---
        "anthropic" => {
            standard_provider!(
                config,
                &system_prompt,
                cwd,
                anthropic::Client<reqwest::Client>,
                "Anthropic"
            )
        }

        // --- DeepSeek ---
        "deepseek" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::deepseek::Client<reqwest::Client>,
            "DeepSeek"
        ),

        // --- Ollama (local, no auth) ---
        "ollama" => {
            let client: rig::providers::ollama::Client<reqwest::Client> =
                if let Some(ref url) = config.base_url {
                    rig::providers::ollama::Client::<reqwest::Client>::builder()
                        .api_key(rig::client::Nothing)
                        .base_url(url)
                        .build()
                } else {
                    rig::providers::ollama::Client::<reqwest::Client>::builder()
                        .api_key(rig::client::Nothing)
                        .build()
                }
                .context("Failed to create Ollama client")?;
            let agent = apply_agent_options(
                rig::prelude::CompletionClient::agent(&client, &config.model)
                    .preamble(&system_prompt),
                config,
                cwd,
            );
            Ok(Box::new(agent))
        }

        // --- Groq ---
        "groq" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::groq::Client<reqwest::Client>,
            "Groq"
        ),

        // --- Gemini ---
        "gemini" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::gemini::Client<reqwest::Client>,
            "Gemini"
        ),

        // --- Perplexity ---
        "perplexity" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::perplexity::Client<reqwest::Client>,
            "Perplexity"
        ),

        // --- Moonshot ---
        "moonshot" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::moonshot::Client<reqwest::Client>,
            "Moonshot"
        ),

        // --- xAI ---
        "xai" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::xai::Client<reqwest::Client>,
            "xAI"
        ),

        // --- OpenRouter ---
        "openrouter" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::openrouter::Client<reqwest::Client>,
            "OpenRouter"
        ),

        // --- Mistral ---
        "mistral" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::mistral::Client<reqwest::Client>,
            "Mistral"
        ),

        // --- Together ---
        "together" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::together::Client<reqwest::Client>,
            "Together"
        ),

        // --- Cohere ---
        "cohere" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::cohere::Client<reqwest::Client>,
            "Cohere"
        ),

        // --- HuggingFace ---
        "huggingface" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::huggingface::Client<reqwest::Client>,
            "HuggingFace"
        ),

        // --- Hyperbolic ---
        "hyperbolic" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::hyperbolic::Client<reqwest::Client>,
            "Hyperbolic"
        ),

        // --- Mira ---
        "mira" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::mira::Client<reqwest::Client>,
            "Mira"
        ),

        // --- Galadriel ---
        "galadriel" => standard_provider!(
            config,
            &system_prompt,
            cwd,
            rig::providers::galadriel::Client<reqwest::Client>,
            "Galadriel"
        ),

        // --- Azure OpenAI ---
        "azure" => {
            let base_url = config
                .base_url
                .as_deref()
                .context("base_url (Azure endpoint) is required for azure provider")?;
            let client: rig::providers::azure::Client<reqwest::Client> =
                if let Some(ref ver) = config.api_version {
                    rig::providers::azure::Client::<reqwest::Client>::builder()
                        .api_key(&config.api_key)
                        .azure_endpoint(base_url.to_string())
                        .api_version(ver)
                        .build()
                } else {
                    rig::providers::azure::Client::<reqwest::Client>::builder()
                        .api_key(&config.api_key)
                        .azure_endpoint(base_url.to_string())
                        .build()
                }
                .context("Failed to create Azure client")?;
            let agent = apply_agent_options(
                rig::prelude::CompletionClient::agent(&client, &config.model)
                    .preamble(&system_prompt),
                config,
                cwd,
            );
            Ok(Box::new(agent))
        }

        // --- Zhipu AI (智谱) ---
        "zhipu" => build_openai_compatible(
            config,
            &system_prompt,
            cwd,
            "https://open.bigmodel.cn/api/paas/v4",
            "Zhipu",
        ),

        "zhipu-code" => build_openai_compatible(
            config,
            &system_prompt,
            cwd,
            "https://open.bigmodel.cn/api/coding/paas/v4",
            "Zhipu Code",
        ),

        "zhipu-overseas" => build_openai_compatible(
            config,
            &system_prompt,
            cwd,
            "https://api.z.ai/api/paas/v4",
            "Zhipu Overseas",
        ),

        "zhipu-overseas-code" => build_openai_compatible(
            config,
            &system_prompt,
            cwd,
            "https://api.z.ai/api/coding/paas/v4",
            "Zhipu Overseas Code",
        ),

        other => {
            let supported = [
                "openai",
                "openai-compatible",
                "anthropic",
                "deepseek",
                "ollama",
                "groq",
                "gemini",
                "perplexity",
                "moonshot",
                "xai",
                "openrouter",
                "mistral",
                "together",
                "cohere",
                "huggingface",
                "hyperbolic",
                "mira",
                "galadriel",
                "azure",
                "zhipu",
                "zhipu-code",
                "zhipu-overseas",
                "zhipu-overseas-code",
            ];
            anyhow::bail!(
                "Unknown provider: {other}. Supported: {}",
                supported.join(", ")
            )
        }
    }
}

/// Build an agent using OpenAI Completions API with a preset default base_url.
/// Used for OpenAI-compatible providers (Zhipu, etc.).
fn build_openai_compatible(
    config: &AgentConfig,
    system_prompt: &str,
    cwd: PathBuf,
    default_base_url: &str,
    label: &str,
) -> Result<Box<dyn ChatAgent>> {
    let base_url = config.base_url.as_deref().unwrap_or(default_base_url);
    let api_key = if config.api_key.is_empty() {
        "not-needed"
    } else {
        &config.api_key
    };
    let client = openai::Client::<reqwest::Client>::builder()
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .context(format!("Failed to create {label} client"))?
        .completions_api();
    let agent = apply_agent_options(
        rig::prelude::CompletionClient::agent(&client, &config.model).preamble(system_prompt),
        config,
        cwd,
    );
    Ok(Box::new(agent))
}

/// Apply temperature, max_tokens, and tools to an agent builder, then build it.
fn apply_agent_options<M>(
    mut builder: rig::agent::AgentBuilder<M>,
    config: &AgentConfig,
    cwd: PathBuf,
) -> rig::agent::Agent<M>
where
    M: rig::completion::CompletionModel,
{
    if let Some(temp) = config.temperature {
        builder = builder.temperature(temp);
    }
    if let Some(max_tokens) = config.max_tokens {
        builder = builder.max_tokens(max_tokens as u64);
    }
    let bash_tool = BashTool { cwd: cwd.clone() };
    let read_file_tool = ReadFileTool { cwd };
    builder.tool(bash_tool).tool(read_file_tool).build()
}
