// SPDX-License-Identifier: GPL-3.0

//! Minimal PNG writing and header reading.
//!
//! This exists for one job: an AppImage whose only icon is an XPM. Showing
//! that icon in the window is [`super::xpm`]'s business, but *integrating* it
//! means writing a file into `~/.local/share/icons/hicolor` for every other
//! program to read — and while the icon-theme specification still admits XPM,
//! COSMIC itself cannot decode one, so an `.xpm` written there is a launcher
//! entry with a hole where its icon should be. Converting to PNG at
//! integration time fixes that for every consumer at once.
//!
//! Writing a PNG is small enough to do here: the format is a signature and a
//! sequence of length-type-data-CRC chunks, and the only real work — the
//! compression — is done by `flate2`, which is already a dependency. Only
//! 8-bit RGBA output is supported, because that is the only thing asked of it.

use std::io::Write;

/// The eight bytes every PNG starts with. Also used by callers to recognise a
/// PNG they already have, so it is public.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Byte offset of the width field: the signature, then the IHDR chunk's length
/// and type.
const WIDTH_OFFSET: usize = 16;

/// Encode 8-bit RGBA pixels as a PNG.
///
/// `None` for dimensions that do not match the pixel buffer — the caller
/// assembled them from a decode and a mismatch means that decode is not to be
/// trusted, not that a best effort should be made.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if width == 0 || height == 0 || rgba.len() != expected {
        return None;
    }

    // Scanlines, each prefixed with filter type 0 (none). Cleverer per-row
    // filters shrink the file a little; for a one-time write of an icon the
    // simplicity is worth more than the kilobytes.
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity(expected + height as usize);
    for row in rgba.chunks(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }

    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).ok()?;
    let compressed = encoder.finish().ok()?;

    // IHDR: dimensions, then bit depth 8, colour type 6 (truecolour with
    // alpha), compression 0, filter method 0, no interlace.
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = Vec::with_capacity(compressed.len() + 64);
    png.extend_from_slice(&SIGNATURE);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &compressed);
    chunk(&mut png, b"IEND", &[]);
    Some(png)
}

/// The pixel width recorded in a PNG's header, or `None` if `bytes` is not a
/// PNG. Used to file an icon in the size directory matching its actual pixels.
pub fn width(bytes: &[u8]) -> Option<u32> {
    if !bytes.starts_with(&SIGNATURE) || bytes.len() < WIDTH_OFFSET + 4 {
        return None;
    }
    let width = u32::from_be_bytes([
        bytes[WIDTH_OFFSET],
        bytes[WIDTH_OFFSET + 1],
        bytes[WIDTH_OFFSET + 2],
        bytes[WIDTH_OFFSET + 3],
    ]);
    (width > 0 && width <= 1024).then_some(width)
}

/// Append one chunk: big-endian length, type, data, then the CRC of the type
/// and data together — the length is not covered, per the specification.
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);

    let mut crc = Crc32::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

/// CRC-32 as PNG requires it (the IEEE polynomial, reflected).
///
/// Bitwise rather than table-driven: the largest thing ever summed here is one
/// icon's worth of compressed data, for which a 1 KiB lookup table is not worth
/// its own weight.
struct Crc32(u32);

impl Crc32 {
    fn new() -> Self {
        Self(0xffff_ffff)
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u32::from(byte);
            for _ in 0..8 {
                // Branch-free: the mask is all-ones when the low bit is set.
                let mask = (self.0 & 1).wrapping_neg();
                self.0 = (self.0 >> 1) ^ (0xedb8_8320 & mask);
            }
        }
    }

    fn finish(self) -> u32 {
        !self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        // The check value every CRC-32 implementation is verified against.
        let mut crc = Crc32::new();
        crc.update(b"123456789");
        assert_eq!(crc.finish(), 0xcbf4_3926);
    }

    #[test]
    fn an_encoded_png_round_trips_through_an_independent_inflater() {
        let rgba: Vec<u8> = vec![
            0xff, 0x00, 0x00, 0xff, // red
            0x00, 0x00, 0x00, 0x00, // transparent
        ];
        let png = encode_rgba(2, 1, &rgba).unwrap();

        assert!(png.starts_with(&SIGNATURE));
        assert_eq!(width(&png), Some(2));
        assert!(png.ends_with(&{
            // IEND with its fixed, well-known CRC.
            let mut end = Vec::new();
            chunk(&mut end, b"IEND", &[]);
            end
        }));

        // Pull the IDAT payload back out and inflate it with flate2's decoder,
        // which shares no code with the encoding path above.
        let idat_start = png.windows(4).position(|w| w == b"IDAT").unwrap() + 4;
        let length = u32::from_be_bytes([
            png[idat_start - 8],
            png[idat_start - 7],
            png[idat_start - 6],
            png[idat_start - 5],
        ]) as usize;
        let mut inflated = Vec::new();
        flate2::read::ZlibDecoder::new(&png[idat_start..idat_start + length])
            .read_to_end(&mut inflated)
            .unwrap();

        // One scanline: filter byte 0, then the pixels verbatim.
        let mut expected = vec![0u8];
        expected.extend_from_slice(&rgba);
        assert_eq!(inflated, expected);
    }

    #[test]
    fn dimensions_that_do_not_match_the_buffer_are_refused() {
        assert!(encode_rgba(2, 2, &[0u8; 4]).is_none());
        assert!(encode_rgba(0, 1, &[]).is_none());
        assert!(encode_rgba(1, 0, &[]).is_none());
    }

    #[test]
    fn width_reads_only_real_png_headers() {
        let png = encode_rgba(64, 32, &vec![0u8; 64 * 32 * 4]).unwrap();
        assert_eq!(width(&png), Some(64));

        assert_eq!(width(b"not a png at all"), None);
        assert_eq!(width(&[]), None);
        assert_eq!(width(&SIGNATURE), None);
    }
}
