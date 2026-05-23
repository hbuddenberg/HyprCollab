use std::pin::Pin;
use tokio::sync::mpsc;

pub mod openai;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub enum AgentEvent {
    Token(String),
    Complete { tokens_used: u32 },
    Error(String),
}

/// `'static` boxed future — implementors must clone any self-borrows before boxing.
pub type StreamFut = Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'static>>;

pub trait AgentConnector: Send + Sync {
    fn stream(
        &self,
        messages: Vec<ChatMessage>,
        model: String,
        tx: mpsc::Sender<AgentEvent>,
    ) -> StreamFut;
}
