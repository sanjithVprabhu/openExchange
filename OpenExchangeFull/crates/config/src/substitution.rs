use anyhow::Result;
use regex::Regex;
use std::env;
use tracing::{debug, warn};

/// Substitute environment variables in the format ${VAR_NAME} or $VAR_NAME
pub fn substitute_env_vars(content: &str) -> Result<String> {
    let re = Regex::new(r"\$\{(\w+)\}|\$(\w+)").unwrap();
    let mut result = content.to_string();
    let mut missing_vars = Vec::new();

    for caps in re.captures_iter(content) {
        let var_name = caps.get(1).or(caps.get(2)).unwrap().as_str();
        let placeholder = caps.get(0).unwrap().as_str();

        match env::var(var_name) {
            Ok(value) => {
                debug!("Substituting environment variable: {} = \"{}\"", var_name, value);
                result = result.replace(placeholder, &value);
            }
            Err(_) => {
                warn!("Environment variable '{}' not set", var_name);
                missing_vars.push(var_name.to_string());
                // Keep the placeholder if env var is not set
                // The validator will catch this later
            }
        }
    }

    if !missing_vars.is_empty() {
        debug!(
            "Environment variables not set (may use defaults or fail validation): {:?}",
            missing_vars
        );
    }

    Ok(result)
}

/// Get environment variable with a default value
pub fn get_env_or_default(var_name: &str, default: &str) -> String {
    match env::var(var_name) {
        Ok(value) => {
            debug!("Using environment variable: {} = \"{}\"", var_name, value);
            value
        }
        Err(_) => {
            warn!(
                "Environment variable '{}' not set, using default: \"{}\"",
                var_name, default
            );
            default.to_string()
        }
    }
}

/// Check if a string contains unresolved environment variable placeholders
pub fn has_unresolved_env_vars(content: &str) -> bool {
    let re = Regex::new(r"\$\{(\w+)\}|\$(\w+)").unwrap();
    re.is_match(content)
}