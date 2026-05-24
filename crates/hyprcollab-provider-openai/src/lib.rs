//! # hyprcollab-provider-openai
//!
//! OpenAI (and OpenAI-compatible) LLM provider for HyprCollab.

pub mod client;
pub mod embeddings;
pub mod streaming;
pub mod types;

pub use client::OpenAiClient;
pub use types::*;
