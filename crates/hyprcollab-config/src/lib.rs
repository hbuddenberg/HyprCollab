//! # hyprcollab-config
//!
//! Hierarchical 4-layer configuration resolution for HyprCollab.
//!
//! Layers (lowest → highest priority):
//! 1. **Global** — `~/.config/hyprcollab/config.yaml`
//! 2. **Folder** — `.hyprcollab/config.yaml` (walked up from cwd)
//! 3. **Agent** — `~/.config/hyprcollab/agents/<name>.md` (YAML frontmatter)
//! 4. **Chat** — per-chat overrides applied at runtime

pub mod agent;
pub mod chat;
pub mod folder;
pub mod global;
pub mod resolver;

pub use agent::AgentConfig;
pub use chat::ChatConfig;
pub use folder::FolderConfig;
pub use global::GlobalConfig;
pub use resolver::{ConfigResolver, ResolvedConfig};
