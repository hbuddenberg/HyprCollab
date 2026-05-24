//! # hyprcollab-commands
//!
//! Slash command system for HyprCollab.
//!
//! Provides:
//! - **Parser** — parses `/command arg1 arg2 "quoted arg"` into structured types
//! - **Registry** — registers and looks up `SlashCommand` implementations
//! - **Built-in commands** — `/agent`, `/skill`, `/temperature`, `/approval`, `/run`,
//!   `/browse`, `/design`, `/review`, `/model`, `/help`, `/config`

pub mod parser;
pub mod registry;
pub mod builtins;

pub use parser::ParsedCommand;
pub use registry::CommandRegistry;

#[cfg(test)]
mod tests;
