//! # hyprcollab-core
//!
//! Foundation crate with shared types, traits, and errors for the HyprCollab project.

pub mod errors;
pub mod traits;
pub mod types;

pub use errors::{CoreError, Result};
pub use traits::*;
pub use types::*;
