//! Stable, deliberately small error vocabulary for both session and MCP callers.
use rmcp::schemars::{self, JsonSchema};
use serde::Serialize;

pub type Result<T> = std::result::Result<T, SessionError>;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SessionError {
    pub code: String,
    pub message: String,
}

impl SessionError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    pub(crate) fn invalid(error: impl std::fmt::Display) -> Self {
        Self::new("INVALID_INPUT", error.to_string())
    }

    pub(crate) fn limit(message: impl Into<String>) -> Self {
        Self::new("RESOURCE_LIMIT", message)
    }
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SessionError {}
