use anyhow::Result;
use image::{DynamicImage, Rgb, RgbImage};

/// Maximum palette size for sixel encoding.
const MAX_COLORS: usize = 256;
/// Sixel "band" height in pixels.
const BAND_HEIGHT: usize = 6;

/// Encode a PNG byte slice into a DEC Sixel escape sequence.
///
/// The sequence can be written directly to a sixel-capable terminal.
/// Color quantization is performed to fit within the 256-color palette limit.
pub fn encode_sixel(png_bytes: &[u8]) -> Result<String> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| anyhow::anyhow!("image load error: {e}"))?;
    encode_image_sixel(&img)
}

/// Encode an already-decoded `DynamicImage` to sixel.
pub fn encode_image_sixel(img: &DynamicImage) -> Result<String> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    if w == 0 || h == 0 {
        return Err(anyhow::anyhow!("zero-dimension image"));
    }

    // Build a palette via median-cut–style quantization (simplified: sample
    // unique colours then truncate to MAX_COLORS).
    let palette = build_palette(&rgb);

    let mut out = String::new();
    // DCS intro: ESC P 0 ; 0 ; 0 q
    out.push_str("\x1bP0;0;0q");
    // Raster attributes: "pan;pad;ph;pv  (1:1 ratio, actual size)
    out.push_str(&format!("\"1;1;{w};{h}"));

    // Emit palette entries: #n;2;R;G;B  (R/G/B in 0–100 range)
    for (idx, &[r, g, b]) in palette.iter().enumerate() {
        let r100 = r as u32 * 100 / 255;
        let g100 = g as u32 * 100 / 255;
        let b100 = b as u32 * 100 / 255;
        out.push_str(&format!("#{idx};2;{r100};{g100};{b100}"));
    }

    // Encode pixel rows in bands of 6
    let bands = (h as usize + BAND_HEIGHT - 1) / BAND_HEIGHT;

    for band in 0..bands {
        let y_start = band * BAND_HEIGHT;
        let y_end = ((band + 1) * BAND_HEIGHT).min(h as usize);

        // For each colour that appears in this band, build a sixel scanline.
        let mut color_rows: std::collections::HashMap<usize, Vec<u8>> =
            std::collections::HashMap::new();

        for x in 0..w as usize {
            for (bit, y) in (y_start..y_end).enumerate() {
                let px = rgb.get_pixel(x as u32, y as u32);
                let ci = nearest_color(px, &palette);
                let entry = color_rows.entry(ci).or_insert_with(|| vec![0u8; w as usize]);
                entry[x] |= 1 << bit;
            }
        }

        // Emit each colour's scanline
        for (ci, bits) in &color_rows {
            out.push_str(&format!("#{ci}"));
            for &b in bits {
                out.push((b + 63) as char); // sixel character
            }
            out.push('$'); // carriage return within band
        }
        out.push('-'); // next band
    }

    out.push_str("\x1b\\"); // ST
    Ok(out)
}

fn build_palette(img: &RgbImage) -> Vec<[u8; 3]> {
    use std::collections::HashSet;
    let mut seen: HashSet<[u8; 3]> = HashSet::new();
    for px in img.pixels() {
        seen.insert([px[0], px[1], px[2]]);
        if seen.len() >= MAX_COLORS {
            break;
        }
    }
    let mut palette: Vec<[u8; 3]> = seen.into_iter().collect();
    palette.sort(); // deterministic order
    palette
}

fn nearest_color(px: &Rgb<u8>, palette: &[[u8; 3]]) -> usize {
    let mut best = 0;
    let mut best_dist = u32::MAX;
    for (i, &[r, g, b]) in palette.iter().enumerate() {
        let dr = px[0] as i32 - r as i32;
        let dg = px[1] as i32 - g as i32;
        let db = px[2] as i32 - b as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn tiny_rgb_image(r: u8, g: u8, b: u8) -> DynamicImage {
        let img: RgbImage = ImageBuffer::from_pixel(4, 4, Rgb([r, g, b]));
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn sixel_starts_with_dcs() {
        let img = tiny_rgb_image(255, 0, 0);
        let seq = encode_image_sixel(&img).unwrap();
        assert!(seq.starts_with("\x1bP"), "should start with ESC P (DCS): {seq:?}");
    }

    #[test]
    fn sixel_ends_with_st() {
        let img = tiny_rgb_image(0, 255, 0);
        let seq = encode_image_sixel(&img).unwrap();
        assert!(seq.ends_with("\x1b\\"), "should end with ST");
    }

    #[test]
    fn sixel_contains_q_intro() {
        let img = tiny_rgb_image(0, 0, 255);
        let seq = encode_image_sixel(&img).unwrap();
        assert!(seq.contains('q'), "sixel raster data starts with 'q'");
    }

    #[test]
    fn sixel_contains_palette_entry() {
        let img = tiny_rgb_image(128, 64, 32);
        let seq = encode_image_sixel(&img).unwrap();
        assert!(seq.contains(";2;"), "palette entries use ;2; (RGB) format");
    }

    #[test]
    fn sixel_raster_attributes_contain_dimensions() {
        let img = tiny_rgb_image(255, 255, 255);
        let seq = encode_image_sixel(&img).unwrap();
        assert!(seq.contains("\"1;1;4;4"), "raster attrs should encode 4x4");
    }

    #[test]
    fn nearest_color_finds_exact_match() {
        let palette: Vec<[u8; 3]> = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]];
        assert_eq!(nearest_color(&Rgb([255, 0, 0]), &palette), 0);
        assert_eq!(nearest_color(&Rgb([0, 255, 0]), &palette), 1);
        assert_eq!(nearest_color(&Rgb([0, 0, 255]), &palette), 2);
    }

    #[test]
    fn palette_max_256_colors() {
        // Create an image with many distinct colours
        let mut img: RgbImage = ImageBuffer::new(64, 64);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = Rgb([(x * 4) as u8, (y * 4) as u8, 128]);
        }
        let palette = build_palette(&img);
        assert!(palette.len() <= MAX_COLORS, "palette should be ≤ {MAX_COLORS}");
    }
}
