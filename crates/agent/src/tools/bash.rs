use std::path::PathBuf;

use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Tool for executing shell commands.
pub struct BashTool {
    pub cwd: PathBuf,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BashArgs {
    /// The shell command to execute.
    pub command: String,
    /// Timeout in seconds (default: 30).
    pub timeout: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl std::fmt::Display for BashOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.stdout.is_empty() {
            write!(f, "{}", self.stdout)?;
        }
        if !self.stderr.is_empty() {
            if !self.stdout.is_empty() {
                writeln!(f)?;
            }
            write!(f, "stderr: {}", self.stderr)?;
        }
        if self.exit_code != 0 {
            write!(f, "\n(exit code: {})", self.exit_code)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum BashError {
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Command timed out after {0} seconds")]
    Timeout(u64),
}

impl Tool for BashTool {
    const NAME: &'static str = "bash";

    type Error = BashError;
    type Args = BashArgs;
    type Output = BashOutput;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a shell command and return stdout, stderr, and exit code."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30)"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let timeout_secs = args.timeout.unwrap_or(30);
        let timeout = std::time::Duration::from_secs(timeout_secs);

        let result = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&args.command)
                .current_dir(&self.cwd)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => Ok(BashOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
            }),
            Ok(Err(e)) => Err(BashError::ExecutionFailed(e.to_string())),
            Err(_) => Err(BashError::Timeout(timeout_secs)),
        }
    }
}
