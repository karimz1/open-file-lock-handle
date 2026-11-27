//! Minimal PNG encoder for small RGBA icons.
//!
//! Icons are at most a few kilobytes, so the image data is stored in
//! uncompressed deflate blocks. That keeps the encoder dependency-free and
//! easy to verify; webviews decode it like any other PNG.

/// Encode `rgba` (row-major, 4 bytes per pixel) as a PNG file.
///
/// Returns `None` when the buffer does not match the dimensions.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let row = (width as usize).checked_mul(4)?;
    if width == 0 || height == 0 || rgba.len() != row.checked_mul(height as usize)? {
        return None;
    }
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row) {
        raw.push(0); // filter: none
        raw.extend_from_slice(line);
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, no interlace
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    Some(out)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut blocks = data.chunks(65_535).peekable();
    if blocks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(block) = blocks.next() {
        out.push(u8::from(blocks.peek().is_none()));
        let len = block.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

pub(crate) fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_reference_values() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn encodes_a_valid_png_layout() {
        let pixels = [255, 0, 0, 255, 0, 255, 0, 128];
        let png = encode_rgba(2, 1, &pixels).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 2);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1);
        assert_eq!(&png[png.len() - 12..], b"\0\0\0\0IEND\xaeB`\x82");
        // The stored block carries the filter byte plus both pixels verbatim.
        let idat = png.windows(4).position(|w| w == b"IDAT").unwrap();
        let body = &png[idat + 4 + 2 + 5..];
        assert_eq!(&body[..9], &[0, 255, 0, 0, 255, 0, 255, 0, 128]);
    }

    #[test]
    fn rejects_mismatched_buffers() {
        assert!(encode_rgba(2, 2, &[0; 8]).is_none());
        assert!(encode_rgba(0, 1, &[]).is_none());
    }

    #[test]
    fn splits_large_images_into_stored_blocks() {
        let png = encode_rgba(256, 256, &vec![7; 256 * 256 * 4]).unwrap();
        assert!(png.len() > 256 * 256 * 4);
    }
}
