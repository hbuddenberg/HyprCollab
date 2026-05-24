use thiserror::Error;

/// Unified error type for the HyprCollab core crate.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("LLM provider error: {0}")]
    Llm(String),

    #[error("Memory / RAG error: {0}")]
    Memory(String),

    #[error("Tool execution error: {0}")]
    Tool(String),

    #[error("ACP connection error: {0}")]
    Acp(String),

    #[error("MCP protocol error: {0}")]
    Mcp(String),

    #[error("Browser integration error: {0}")]
    Browser(String),

    #[error("Approval required: {0}")]
    Approval(String),

    #[error("Persona error: {0}")]
    Persona(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Serialization(e.to_string())
    }
}

/// Convenience Result alias.
pub type Result<T> = std::result::Result<T, CoreError>;
