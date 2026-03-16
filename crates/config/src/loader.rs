use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::schema::AppConfig;

/// Load configuration from the given path (or default `~/.opencrab/config.toml`).
///
/// Environment variables in the form `${VAR_NAME}` are substituted before parsing.
pub fn load_config(path: Option<&Path>) -> Result<AppConfig> {
    let config_path = match path {
        Some(p) => p.to_path_buf(),
        None => default_config_path()?,
    };

    let raw = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

    let substituted = substitute_env_vars(&raw);

    let config: AppConfig =
        toml::from_str(&substituted).with_context(|| "Failed to parse config TOML")?;

    Ok(config)
}

/// Load configuration from a TOML string (useful for testing).
pub fn load_config_from_str(toml_str: &str) -> Result<AppConfig> {
    let substituted = substitute_env_vars(toml_str);
    let config: AppConfig =
        toml::from_str(&substituted).with_context(|| "Failed to parse config TOML")?;
    Ok(config)
}

/// Default config file path: `~/.opencrab/config.toml`.
pub fn default_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".opencrab").join("config.toml"))
}

/// Replace `${VAR_NAME}` patterns with environment variable values.
/// Unset variables are replaced with empty strings.
fn substitute_env_vars(input: &str) -> String {
    let re = Regex::new(r"\$\{([^}]+)\}").expect("valid regex");
    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match std::env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                tracing::warn!("Environment variable {var_name} is not set, using empty string");
                String::new()
            }
        }
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_substitution() {
        // SAFETY: test-only, single-threaded context.
        unsafe { std::env::set_var("OPENCRAB_TEST_KEY", "secret123") };
        let input = "api_key = \"${OPENCRAB_TEST_KEY}\"";
        let result = substitute_env_vars(input);
        assert_eq!(result, "api_key = \"secret123\"");
        unsafe { std::env::remove_var("OPENCRAB_TEST_KEY") };
    }

    #[test]
    fn unset_env_var_becomes_empty() {
        let input = "api_key = \"${OPENCRAB_NONEXISTENT_VAR}\"";
        let result = substitute_env_vars(input);
        assert_eq!(result, "api_key = \"\"");
    }

    #[test]
    fn load_example_config() {
        // SAFETY: test-only, single-threaded context.
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "sk-test");
            std::env::set_var("FEISHU_APP_SECRET", "fs-test");
            std::env::set_var("FEISHU_VERIFICATION_TOKEN", "vt-test");
        }

        let toml_str = include_str!("../../../config.example.toml");
        let config = load_config_from_str(toml_str).expect("should parse example config");

        assert_eq!(config.gateway.port, 18789);
        assert_eq!(config.agent.provider, "openai");
        assert_eq!(config.agent.api_key, "sk-test");

        let feishu = config.channels.feishu.expect("feishu should be present");
        assert!(feishu.enabled);
        assert_eq!(feishu.app_secret, "fs-test");

        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("FEISHU_APP_SECRET");
            std::env::remove_var("FEISHU_VERIFICATION_TOKEN");
        }
    }
}
