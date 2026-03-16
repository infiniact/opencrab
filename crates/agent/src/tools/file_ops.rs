use std::path::PathBuf;

use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Tool for reading file contents.
pub struct ReadFileTool {
    pub cwd: PathBuf,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Path to the file (absolute or relative to working directory).
    pub path: String,
    /// Line offset to start reading from (0-based).
    pub offset: Option<usize>,
    /// Maximum number of lines to read.
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ReadFileOutput {
    pub content: String,
    pub lines: usize,
}

impl std::fmt::Display for ReadFileOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("IO error: {0}")]
    Io(String),
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";

    type Error = FileError;
    type Args = ReadFileArgs;
    type Output = ReadFileOutput;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file. Returns the file content with line numbers."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file (absolute or relative to working directory)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line offset to start reading from (0-based)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = if std::path::Path::new(&args.path).is_absolute() {
            PathBuf::from(&args.path)
        } else {
            self.cwd.join(&args.path)
        };

        let content = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path.display().to_string()),
            std::io::ErrorKind::PermissionDenied => {
                FileError::PermissionDenied(path.display().to_string())
            }
            _ => FileError::Io(e.to_string()),
        })?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let offset = args.offset.unwrap_or(0);
        let limit = args.limit.unwrap_or(2000);

        let selected: Vec<String> = lines
            .into_iter()
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(i, line)| format!("{:>4}\t{}", offset + i + 1, line))
            .collect();

        Ok(ReadFileOutput {
            content: selected.join("\n"),
            lines: total_lines,
        })
    }
}
