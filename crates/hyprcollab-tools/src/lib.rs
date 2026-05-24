//! # hyprcollab-tools
//!
//! Built-in tool implementations for the HyprCollab agent runtime.
//!
//! Provides five core tools:
//! - **Shell** — Execute system commands with output capture
//! - **FileOps** — Read and write files
//! - **WebSearch** — Search the web (SearXNG / Brave API)
//! - **WebFetch** — Fetch and parse web pages
//! - **MemoryTools** — Store and retrieve facts from agent memory

pub mod shell;
pub mod shell_enhanced;
pub mod file_ops;
pub mod web_search;
pub mod web_fetch;
pub mod memory_tools;

pub use shell::ShellTool;
pub use shell_enhanced::EnhancedShellTool;
pub use file_ops::FileOpsTool;
pub use web_search::WebSearchTool;
pub use web_fetch::WebFetchTool;
pub use memory_tools::MemoryTools;

#[cfg(test)]
mod tests;
