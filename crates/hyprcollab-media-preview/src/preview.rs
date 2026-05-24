use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;

use crate::capabilities::{GraphicsProtocol, TerminalCapabilities};
use crate::kitty::encode_kitty;
use crate::sixel::encode_image_sixel;

/// The result of a preview encoding operation.
#[derive(Debug, Clone)]
pub enum PreviewOutput {
    /// A terminal escape sequence (kitty or sixel) ready to be written.
    EscapeSequence(String),
    /// No protocol available; fall back to ASCII art or skip.
    Unavailable,
}

/// Unified image preview interface — detects the best available graphics
/// protocol and resizes images to fit the terminal width.
pub struct ImagePreview {
    caps: TerminalCapabilities,
    cache: HashMap<u64, String>,
}

impl ImagePreview {
    pub fn new(caps: TerminalCapabilities) -> Self {
        Self {
            caps,
            cache: HashMap::new(),
        }
    }

    /// Build an `ImagePreview` by auto-detecting terminal capabilities.
    pub fn auto() -> Self {
        Self::new(TerminalCapabilities::from_env())
    }

    pub fn protocol(&self) -> GraphicsProtocol {
        self.caps.graphics
    }

    /// Encode `png_bytes` using the best available protocol.
    ///
    /// Resizes to `max_cols` terminal columns if `max_cols > 0`.
    pub fn encode_png(&mut self, png_bytes: &[u8], max_cols: u16) -> Result<PreviewOutput> {
        if !self.caps.supports_any_graphics() {
            return Ok(PreviewOutput::Unavailable);
        }

        let img = image::load_from_memory(png_bytes)
            .map_err(|e| anyhow::anyhow!("image load: {e}"))?;

        let img = if max_cols > 0 {
            resize_to_cols(img, max_cols)
        } else {
            img
        };

        let seq = match self.caps.graphics {
            GraphicsProtocol::Kitty => {
                let mut png_out = std::io::Cursor::new(Vec::new());
                img.write_to(&mut png_out, image::ImageFormat::Png)
                    .map_err(|e| anyhow::anyhow!("PNG encode: {e}"))?;
                encode_kitty(png_out.get_ref())?
            }
            GraphicsProtocol::Sixel => encode_image_sixel(&img)?,
            GraphicsProtocol::None => return Ok(PreviewOutput::Unavailable),
        };

        Ok(PreviewOutput::EscapeSequence(seq))
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Scale `img` so that its width fits within `cols` terminal columns.
///
/// Assumes each column is roughly 8px wide.  Height is scaled proportionally.
fn resize_to_cols(img: DynamicImage, cols: u16) -> DynamicImage {
    let max_px = (cols as u32).saturating_mul(8);
    let (w, _h) = (img.width(), img.height());
    if w <= max_px || max_px == 0 {
        return img;
    }
    img.thumbnail(max_px, u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::TerminalCapabilities;

    fn tiny_png_bytes() -> Vec<u8> {
        use image::{ImageBuffer, Rgb, RgbImage};
        let img: RgbImage = ImageBuffer::from_pixel(4, 4, Rgb([200u8, 100, 50]));
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn no_protocol_returns_unavailable() {
        let mut preview =
            ImagePreview::new(TerminalCapabilities::with_protocol(GraphicsProtocol::None));
        let result = preview.encode_png(&tiny_png_bytes(), 0).unwrap();
        assert!(matches!(result, PreviewOutput::Unavailable));
    }

    #[test]
    fn kitty_protocol_returns_escape_sequence() {
        let mut preview =
            ImagePreview::new(TerminalCapabilities::with_protocol(GraphicsProtocol::Kitty));
        let result = preview.encode_png(&tiny_png_bytes(), 0).unwrap();
        match result {
            PreviewOutput::EscapeSequence(s) => {
                assert!(s.contains("\x1b_G"), "should be kitty sequence");
            }
            PreviewOutput::Unavailable => panic!("expected kitty output"),
        }
    }

    #[test]
    fn sixel_protocol_returns_escape_sequence() {
        let mut preview =
            ImagePreview::new(TerminalCapabilities::with_protocol(GraphicsProtocol::Sixel));
        let result = preview.encode_png(&tiny_png_bytes(), 0).unwrap();
        match result {
            PreviewOutput::EscapeSequence(s) => {
                assert!(s.contains("\x1bP"), "should be sixel sequence");
            }
            PreviewOutput::Unavailable => panic!("expected sixel output"),
        }
    }

    #[test]
    fn resize_to_cols_shrinks_wide_image() {
        use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};
        let wide: RgbImage = ImageBuffer::from_pixel(1600, 100, Rgb([0u8, 0, 0]));
        let img = DynamicImage::ImageRgb8(wide);
        let resized = resize_to_cols(img, 80); // 80 cols → max 640px
        assert!(
            resized.width() <= 640,
            "should be resized, got {}",
            resized.width()
        );
    }

    #[test]
    fn resize_to_cols_zero_does_not_change_image() {
        use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};
        let img_raw: RgbImage = ImageBuffer::from_pixel(100, 100, Rgb([0u8, 0, 0]));
        let img = DynamicImage::ImageRgb8(img_raw);
        let result = resize_to_cols(img, 0);
        assert_eq!(result.width(), 100);
    }

    #[test]
    fn protocol_getter() {
        let preview =
            ImagePreview::new(TerminalCapabilities::with_protocol(GraphicsProtocol::Kitty));
        assert_eq!(preview.protocol(), GraphicsProtocol::Kitty);
    }
}
