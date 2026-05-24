//! # hyprcollab-provider-anthropic
//!
//! Anthropic Claude LLM provider for HyprCollab.

pub mod client;
pub mod embeddings;
pub mod streaming;
pub mod types;

pub use client::AnthropicClient;
pub use types::*;
