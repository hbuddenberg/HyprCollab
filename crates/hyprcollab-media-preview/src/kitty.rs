use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Maximum base64 payload per chunk (kitty protocol recommends ≤ 4096).
const CHUNK_SIZE: usize = 4096;

/// Encode PNG bytes into a sequence of kitty graphics protocol escape strings.
///
/// Returns one or more `ESC_G...ESC\` strings that can be written directly to
/// the terminal to display the image.
pub fn encode_kitty(png_bytes: &[u8]) -> Result<String> {
    if png_bytes.is_empty() {
        return Err(anyhow::anyhow!("empty PNG data"));
    }

    let encoded = BASE64.encode(png_bytes);
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(CHUNK_SIZE)
        .map(|c| std::str::from_utf8(c).expect("base64 is ASCII"))
        .collect();

    let mut out = String::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        let more = if is_last { 0 } else { 1 };

        if i == 0 {
            // First chunk — include format metadata.
            // a=T  action=transmit
            // f=100 format=PNG
            // q=1  suppress OK response
            // m=0/1 more chunks
            out.push_str(&format!(
                "\x1b_Ga=T,f=100,q=1,m={more};{chunk}\x1b\\"
            ));
        } else {
            // Continuation chunk.
            out.push_str(&format!("\x1b_Gm={more};{chunk}\x1b\\"));
        }
    }

    Ok(out)
}

/// Decode the payload embedded in a kitty sequence for testing purposes.
///
/// Extracts the base64 payload from the first APC sequence in `seq`.
pub fn extract_payload(seq: &str) -> Option<Vec<u8>> {
    // Find ESC_G...;payload...ESC\
    let start = seq.find("\x1b_G")?;
    let after_header = &seq[start + 3..];
    let semi = after_header.find(';')?;
    let payload_start = semi + 1;
    let payload_end = after_header.find("\x1b\\")?;
    if payload_end <= payload_start {
        return None;
    }
    let b64 = &after_header[payload_start..payload_end];
    BASE64.decode(b64).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        // Minimal valid 1×1 red PNG (89 bytes)
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
            0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
            0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
            0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
            0x44, 0xae, 0x42, 0x60, 0x82,
        ]
    }

    #[test]
    fn encode_starts_with_apc() {
        let seq = encode_kitty(&tiny_png()).unwrap();
        assert!(seq.starts_with("\x1b_G"), "should start with ESC_G");
    }

    #[test]
    fn encode_ends_with_st() {
        let seq = encode_kitty(&tiny_png()).unwrap();
        assert!(seq.ends_with("\x1b\\"), "should end with ESC\\");
    }

    #[test]
    fn encode_contains_action_transmit() {
        let seq = encode_kitty(&tiny_png()).unwrap();
        assert!(seq.contains("a=T"), "should have a=T (transmit)");
    }

    #[test]
    fn encode_contains_png_format() {
        let seq = encode_kitty(&tiny_png()).unwrap();
        assert!(seq.contains("f=100"), "should have f=100 (PNG format)");
    }

    #[test]
    fn encode_payload_is_valid_base64() {
        let png = tiny_png();
        let seq = encode_kitty(&png).unwrap();
        let decoded = extract_payload(&seq).expect("should decode payload");
        assert_eq!(decoded, png, "decoded payload should match original PNG");
    }

    #[test]
    fn empty_input_returns_error() {
        assert!(encode_kitty(&[]).is_err());
    }

    #[test]
    fn large_image_uses_multiple_chunks() {
        // 8 KB of data → at least 2 chunks at 4096 base64 chars each
        let big_data = vec![0xFFu8; 8192];
        let seq = encode_kitty(&big_data).unwrap();
        // Count APC starts
        let chunk_count = seq.matches("\x1b_G").count();
        assert!(chunk_count >= 2, "large data should be split: got {chunk_count} chunks");
    }
}
