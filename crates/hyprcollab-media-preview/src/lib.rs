//! hyprcollab-media-preview — terminal image preview via kitty and sixel protocols.
//!
//! # Usage
//!
//! ```no_run
//! use hyprcollab_media_preview::ImagePreview;
//!
//! let mut preview = ImagePreview::auto();
//! // preview.encode_png(&png_bytes, 80) → escape sequence or Unavailable
//! ```

pub mod capabilities;
pub mod kitty;
pub mod preview;
pub mod sixel;

pub use capabilities::{GraphicsProtocol, TerminalCapabilities};
pub use kitty::encode_kitty;
pub use preview::{ImagePreview, PreviewOutput};
pub use sixel::encode_sixel;
