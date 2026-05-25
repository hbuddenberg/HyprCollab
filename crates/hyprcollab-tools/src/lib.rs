//! # hyprcollab-tools
//!
//! Built-in tool implementations for the HyprCollab agent runtime.

pub mod shell;
pub mod shell_enhanced;
pub mod file_ops;
pub mod web_search;
pub mod web_fetch;
pub mod memory_tools;
pub mod browser_tools;
pub mod rag_tools;
pub mod image_tools;

pub use shell::ShellTool;
pub use shell_enhanced::EnhancedShellTool;
pub use file_ops::FileOpsTool;
pub use web_search::WebSearchTool;
pub use web_fetch::WebFetchTool;
pub use memory_tools::MemoryTools;
pub use browser_tools::{
    BrowserExtractTool, BrowserNavigateTool, BrowserScrapeTool, BrowserScreenshotTool,
    BrowserSearchTool,
};
pub use rag_tools::{RagIngestTool, RagQueryTool, RagSearchTool};
pub use image_tools::{ImageGenerateTool, ImageListProvidersTool};

#[cfg(test)]
mod tests;
