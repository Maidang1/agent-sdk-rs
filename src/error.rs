use crate::provider::ProviderError;

#[derive(Debug)]
pub enum AgentError {
    Provider(ProviderError),
    ToolNotFound(String),
    ToolExecutionFailed(String),
    ParseError(String),
    InvalidParameters(String),
}

impl From<ProviderError> for AgentError {
    fn from(err: ProviderError) -> Self {
        AgentError::Provider(err)
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(err) => write!(f, "Provider error: {}", err),
            Self::ToolNotFound(name) => write!(f, "Tool not found: {}", name),
            Self::ToolExecutionFailed(msg) => write!(f, "Tool execution failed: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
        }
    }
}

impl std::error::Error for AgentError {}

pub type Result<T> = std::result::Result<T, AgentError>;
