pub mod commands;
pub mod engine;
pub mod errors;
pub mod page;
pub mod session;

#[cfg(test)]
mod tests;

pub use commands::{
    all_commands, BrowserCommand, WebExtract, WebInteract, WebNavigate, WebScreenshot, WebSearch,
};
pub use engine::BrowserEngine;
pub use errors::{BrowserError, Result};
pub use page::{LinkInfo, PageSnapshot, SearchResult};
pub use session::{BrowserConfig, BrowserSession, BrowserType, Cookie, Viewport};
