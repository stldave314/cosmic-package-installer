// SPDX-License-Identifier: GPL-3.0

//! Decoding X PixMap (`.xpm`) icons.
//!
//! XPM is not really an image format — it is C source code. A `.xpm` file is a
//! declaration of an array of string literals, designed in the late 1980s to be
//! `#include`d straight into an X11 program. That is why no general-purpose
//! image library decodes it, `image` included, and why a package shipping only
//! an `.xpm` would otherwise show the generic placeholder.
//!
//! ```c
//! /* XPM */
//! static char *icon[] = {
//! "16 16 2 1",          /* width height colours chars-per-pixel */
//! "  c None",           /* the colour table, one line per colour */
//! ". c #FF0000",
//! "  ..........  ",     /* then one line per row of pixels */
//! ...
//! ```
//!
//! Being text, it is decoded here rather than by pulling in a C library for it.
//! That is the safer of the two options and not the riskier one: the parsing is
//! all safe Rust over a byte slice, so the memory-safety class of bug that has
//! historically troubled C implementations of this format cannot arise.
//!
//! ## What is bounded, and why
//!
//! Everything this reads is attacker-controlled in the sense that it comes out
//! of the file being inspected, and the header states its own dimensions and
//! colour count. So:
//!
//! * the input, the dimensions, the colour count and the key length are all
//!   checked against [`crate::constants`] before anything is allocated;
//! * the output allocation is computed with checked arithmetic, so a `width ×
//!   height × 4` that would overflow is a refusal rather than a wrap;
//! * nothing indexes or slices directly, because a panic here would not merely
//!   crash — inspection runs on a worker whose result is dropped on panic,
//!   leaving the window reading "Reading package…" with nothing coming;
//! * colour lookup is by hash, because a linear scan of a 1770-entry palette
//!   across 65 536 pixels is 116 million comparisons.
//!
//! There is no external-reference surface to worry about: XPM has no include,
//! link or URL mechanism, and the optional `XPMEXT` trailer is inert.

use std::collections::HashMap;

use crate::constants::{
    XPM_MAX_CHARS_PER_PIXEL, XPM_MAX_COLORS, XPM_MAX_DIMENSION, XPM_MAX_INPUT_BYTES,
};
use crate::debug::ICON;
use crate::debug_log;

/// The signature an XPM3 file must begin with. Version 1 and 2 files, which
/// have a different syntax entirely, are refused rather than guessed at.
const SIGNATURE: &[u8] = b"/* XPM */";

/// Fully transparent, used for `None` and for any pixel whose key the colour
/// table does not define.
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// The keys an XPM colour table entry can be given for, in the order this
/// decoder prefers them: colour, then greyscale, then monochrome.
const COLOUR_KEYS: &[&str] = &["c", "g", "g4", "m", "s"];

/// A decoded image, as 8-bit RGBA ready for the toolkit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width × height × 4` bytes.
    pub rgba: Vec<u8>,
}

/// Decode an XPM, or `None` if it cannot be read as one.
///
/// Failure is never an error worth surfacing: the caller is looking for an icon
/// and has other candidates to try, so an undecodable one simply is not it.
pub fn decode(bytes: &[u8]) -> Option<Image> {
    if bytes.len() > XPM_MAX_INPUT_BYTES {
        debug_log!(ICON, "XPM of {} bytes is too large to decode", bytes.len());
        return None;
    }
    if !starts_with_signature(bytes) {
        return None;
    }

    // At most one values line, a full colour table and a full image's worth of
    // rows. Bounded up front so a file consisting of nothing but string
    // literals cannot make this collect indefinitely.
    let limit = 1 + XPM_MAX_COLORS + XPM_MAX_DIMENSION as usize;
    let literals = string_literals(bytes, limit);

    let mut literals = literals.into_iter();
    let header = Header::parse(literals.next()?)?;
    debug_log!(
        ICON,
        "XPM {}x{}, {} colours, {} chars per pixel",
        header.width,
        header.height,
        header.colours,
        header.chars_per_pixel
    );

    // Checked because both factors came out of the file. The bounds above make
    // an overflow unreachable in practice; this is what makes that true rather
    // than merely likely.
    let pixels = (header.width as usize).checked_mul(header.height as usize)?;
    let capacity = pixels.checked_mul(4)?;

    let mut palette: HashMap<&[u8], [u8; 4]> = HashMap::with_capacity(header.colours);
    let mut resolved = 0usize;
    for _ in 0..header.colours {
        let entry = literals.next()?;
        let (key, colour) = parse_colour_entry(entry, header.chars_per_pixel)?;
        if colour.is_some() {
            resolved += 1;
        }
        palette.insert(key, colour.unwrap_or(TRANSPARENT));
    }

    // A palette in which nothing at all could be understood would decode to a
    // fully transparent rectangle, which is worse than no icon: the caller
    // would stop looking, having found one.
    if resolved == 0 {
        debug_log!(ICON, "XPM palette has no colour this decoder understands");
        return None;
    }

    let mut rgba = Vec::with_capacity(capacity);
    for _ in 0..header.height {
        // A file that ends early gets transparent rows rather than a refusal.
        let row = literals.next().unwrap_or(&[]);
        for x in 0..header.width as usize {
            let start = x * header.chars_per_pixel;
            let key = row
                .get(start..start + header.chars_per_pixel)
                .unwrap_or(&[]);
            let colour = palette.get(key).copied().unwrap_or(TRANSPARENT);
            rgba.extend_from_slice(&colour);
        }
    }

    Some(Image {
        width: header.width,
        height: header.height,
        rgba,
    })
}

fn starts_with_signature(bytes: &[u8]) -> bool {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    bytes.get(start..).is_some_and(|rest| rest.starts_with(SIGNATURE))
}

// ── Header ──────────────────────────────────────────────────────────────────

struct Header {
    width: u32,
    height: u32,
    colours: usize,
    chars_per_pixel: usize,
}

impl Header {
    /// Parse the values line: `width height colours chars-per-pixel`, with an
    /// optional hotspot and `XPMEXT` marker after it that nothing here needs.
    fn parse(literal: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(literal).ok()?;
        let mut fields = text.split_ascii_whitespace();

        let width: u32 = fields.next()?.parse().ok()?;
        let height: u32 = fields.next()?.parse().ok()?;
        let colours: usize = fields.next()?.parse().ok()?;
        let chars_per_pixel: usize = fields.next()?.parse().ok()?;

        // Each of these is a bound on an allocation or on the work done below.
        if width == 0 || height == 0 || colours == 0 || chars_per_pixel == 0 {
            return None;
        }
        if width > XPM_MAX_DIMENSION || height > XPM_MAX_DIMENSION {
            debug_log!(ICON, "XPM {width}x{height} exceeds the size bound");
            return None;
        }
        if colours > XPM_MAX_COLORS || chars_per_pixel > XPM_MAX_CHARS_PER_PIXEL {
            debug_log!(ICON, "XPM palette {colours}/{chars_per_pixel} exceeds its bound");
            return None;
        }

        Some(Self {
            width,
            height,
            colours,
            chars_per_pixel,
        })
    }
}

// ── C string literals ───────────────────────────────────────────────────────

/// Collect up to `limit` string literals from C source, skipping comments.
///
/// Comments are tracked rather than ignored because an XPM's own comments —
/// `/* columns rows colors chars-per-pixel */`, `/* pixels */` — sit between the
/// literals, and a stray quotation mark in one would otherwise swallow the
/// rest of the file.
fn string_literals(bytes: &[u8], limit: usize) -> Vec<&[u8]> {
    let mut literals = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() && literals.len() < limit {
        let byte = bytes[index];

        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = match find(bytes, index + 2, b"*/") {
                Some(end) => end + 2,
                None => break,
            };
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index = match bytes[index..].iter().position(|byte| *byte == b'\n') {
                Some(offset) => index + offset + 1,
                None => break,
            };
            continue;
        }
        if byte == b'"' {
            let start = index + 1;
            let mut cursor = start;
            // A backslash escapes the next byte, so an escaped quote does not
            // end the literal. The contents are otherwise taken verbatim:
            // pixel keys are raw bytes and interpreting them would corrupt one.
            while cursor < bytes.len() {
                match bytes[cursor] {
                    b'\\' => cursor += 2,
                    b'"' => break,
                    _ => cursor += 1,
                }
            }
            if cursor >= bytes.len() {
                break;
            }
            if let Some(literal) = bytes.get(start..cursor) {
                literals.push(literal);
            }
            index = cursor + 1;
            continue;
        }

        index += 1;
    }

    literals
}

/// Index of `needle` in `haystack` at or after `from`.
fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let rest = haystack.get(from..)?;
    rest.windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| from + offset)
}

// ── Colour table ────────────────────────────────────────────────────────────

/// Split one colour-table entry into its pixel key and its colour.
///
/// The colour is `None` where the entry names something this decoder cannot
/// resolve; the entry itself is still returned, because the key has to stay in
/// the palette for the pixels that use it to come out transparent rather than
/// picking up whatever colour happens to share their key's hash.
fn parse_colour_entry(entry: &[u8], chars_per_pixel: usize) -> Option<(&[u8], Option<[u8; 4]>)> {
    let key = entry.get(..chars_per_pixel)?;
    let rest = entry.get(chars_per_pixel..)?;
    let text = std::str::from_utf8(rest).ok()?;

    // An entry may specify the colour several ways over — `c` for colour, `m`
    // for monochrome, `s` for a symbolic name — and the value of each runs on
    // until the next key. So the tokens are walked, collecting whatever follows
    // each key, and the most colourful of them wins.
    let mut values: HashMap<&str, String> = HashMap::new();
    let mut current: Option<&str> = None;

    for token in text.split_ascii_whitespace() {
        if COLOUR_KEYS.contains(&token) {
            current = Some(token);
            values.entry(token).or_default();
            continue;
        }
        if let Some(key) = current {
            let value = values.entry(key).or_default();
            if !value.is_empty() {
                // Multi-word names such as `cornflower blue` arrive as separate
                // tokens and mean nothing apart.
                value.push(' ');
            }
            value.push_str(token);
        }
    }

    let colour = COLOUR_KEYS
        .iter()
        .find_map(|key| values.get(*key))
        .filter(|value| !value.is_empty())
        .and_then(|value| parse_colour(value));

    Some((key, colour))
}

/// Resolve one colour specification.
fn parse_colour(value: &str) -> Option<[u8; 4]> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(TRANSPARENT);
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex(hex);
    }
    // `%HHHHSSSSVVVV` is HSV. It is permitted by the format and used by
    // essentially nothing, so it is declined rather than half-implemented.
    if value.starts_with('%') {
        return None;
    }
    parse_name(value)
}

/// Parse `#RGB`, `#RRGGBB`, `#RRRGGGBBB` or `#RRRRGGGGBBBB`.
///
/// XPM allows 1 to 4 hex digits per channel. Only the most significant two are
/// kept, which is what the display can show anyway.
fn parse_hex(hex: &str) -> Option<[u8; 4]> {
    if !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }
    let per_channel = match hex.len() {
        3 | 6 | 9 | 12 => hex.len() / 3,
        _ => return None,
    };

    let mut channels = [0u8; 3];
    for (index, channel) in channels.iter_mut().enumerate() {
        let digits = hex.get(index * per_channel..(index + 1) * per_channel)?;
        *channel = if per_channel == 1 {
            // A single digit names both nibbles: `#F00` is `#FF0000`.
            let value = u8::from_str_radix(digits, 16).ok()?;
            value * 17
        } else {
            u8::from_str_radix(digits.get(..2)?, 16).ok()?
        };
    }

    Some([channels[0], channels[1], channels[2], 0xff])
}

/// Colour names from X11's `rgb.txt`.
///
/// Deliberately a short list rather than the full seven-hundred-odd entries. Of
/// the 2402 colour specifications in the XPM files on a typical desktop, 2399
/// are hexadecimal and three are `None` — not one is a name. These are here so
/// that a hand-written XPM still decodes, not because the table earns its
/// weight in kilobytes.
const NAMED_COLOURS: &[(&str, [u8; 3])] = &[
    ("aqua", [0x00, 0xff, 0xff]),
    ("black", [0x00, 0x00, 0x00]),
    ("blue", [0x00, 0x00, 0xff]),
    ("brown", [0xa5, 0x2a, 0x2a]),
    ("cyan", [0x00, 0xff, 0xff]),
    ("darkblue", [0x00, 0x00, 0x8b]),
    ("darkgray", [0xa9, 0xa9, 0xa9]),
    ("darkgreen", [0x00, 0x64, 0x00]),
    ("darkred", [0x8b, 0x00, 0x00]),
    ("fuchsia", [0xff, 0x00, 0xff]),
    ("gold", [0xff, 0xd7, 0x00]),
    ("gray", [0xbe, 0xbe, 0xbe]),
    ("green", [0x00, 0x80, 0x00]),
    ("lightblue", [0xad, 0xd8, 0xe6]),
    ("lightgray", [0xd3, 0xd3, 0xd3]),
    ("lightgreen", [0x90, 0xee, 0x90]),
    ("lime", [0x00, 0xff, 0x00]),
    ("magenta", [0xff, 0x00, 0xff]),
    ("maroon", [0x80, 0x00, 0x00]),
    ("navy", [0x00, 0x00, 0x80]),
    ("olive", [0x80, 0x80, 0x00]),
    ("orange", [0xff, 0xa5, 0x00]),
    ("pink", [0xff, 0xc0, 0xcb]),
    ("purple", [0x80, 0x00, 0x80]),
    ("red", [0xff, 0x00, 0x00]),
    ("silver", [0xc0, 0xc0, 0xc0]),
    ("teal", [0x00, 0x80, 0x80]),
    ("violet", [0xee, 0x82, 0xee]),
    ("white", [0xff, 0xff, 0xff]),
    ("yellow", [0xff, 0xff, 0x00]),
];

/// Resolve a colour name, ignoring case and the spaces X11 allows inside one.
fn parse_name(name: &str) -> Option<[u8; 4]> {
    let normalised: String = name
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();

    // `grey` and `gray` are interchangeable throughout X11, and both take a
    // percentage suffix: `gray50` is mid-grey. Computed rather than tabulated,
    // which is a hundred entries this file does not have to carry.
    for prefix in ["gray", "grey"] {
        if let Some(level) = normalised.strip_prefix(prefix) {
            if !level.is_empty() {
                let percent: u32 = level.parse().ok()?;
                if percent > 100 {
                    return None;
                }
                let value = ((percent * 255 + 50) / 100) as u8;
                return Some([value, value, value, 0xff]);
            }
        }
    }

    let normalised = normalised.replace("grey", "gray");
    NAMED_COLOURS
        .iter()
        .find(|(candidate, _)| *candidate == normalised)
        .map(|(_, [red, green, blue])| [*red, *green, *blue, 0xff])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-by-two image: red, transparent / white, red.
    const SAMPLE: &str = r#"/* XPM */
static char *sample[] = {
/* columns rows colors chars-per-pixel */
"2 2 3 1",
". 	c #FF0000",
"  	c None",
"X 	c #FFFFFF",
/* pixels */
". ",
"X.",
};
"#;

    #[test]
    fn decodes_a_small_image() {
        let image = decode(SAMPLE.as_bytes()).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.rgba.len(), 2 * 2 * 4);
        assert_eq!(
            image.rgba,
            vec![
                0xff, 0x00, 0x00, 0xff, // red
                0x00, 0x00, 0x00, 0x00, // transparent
                0xff, 0xff, 0xff, 0xff, // white
                0xff, 0x00, 0x00, 0xff, // red
            ]
        );
    }

    #[test]
    fn refuses_anything_without_the_signature() {
        assert!(decode(b"static char *x[] = { \"1 1 1 1\", \". c #fff\", \".\" };").is_none());
        assert!(decode(b"").is_none());
        assert!(decode(b"\x89PNG\r\n\x1a\n").is_none());
        // Leading whitespace before the signature is fine.
        assert!(decode(format!("\n\n  {SAMPLE}").as_bytes()).is_some());
    }

    /// The whole reason this needed a security answer before it was written.
    #[test]
    fn implausible_headers_are_refused_rather_than_allocated() {
        let header = |values: &str| {
            let source = format!("/* XPM */\nstatic char *x[] = {{\n\"{values}\",\n}};\n");
            decode(source.as_bytes())
        };
        // 65535 x 65535 x 4 would be 17 GB.
        assert!(header("65535 65535 2 1").is_none());
        assert!(header("2000 2 2 1").is_none());
        assert!(header("2 2000 2 1").is_none());
        // Colour table and key length.
        assert!(header("2 2 999999 1").is_none());
        assert!(header("2 2 2 99").is_none());
        // Degenerate values.
        assert!(header("0 0 0 0").is_none());
        assert!(header("2 2 2").is_none());
        assert!(header("not a header").is_none());
        // And an input too large to be an icon.
        assert!(decode(&vec![b' '; XPM_MAX_INPUT_BYTES + 1]).is_none());
    }

    #[test]
    fn a_truncated_file_yields_a_short_image_rather_than_a_panic() {
        // Header promises four rows; only one is present.
        let source = "/* XPM */\nstatic char *x[] = {\n\"2 4 2 1\",\n\
                      \". 	c #FF0000\",\n\"  	c None\",\n\".. \",\n};\n";
        let image = decode(source.as_bytes()).unwrap();
        assert_eq!((image.width, image.height), (2, 4));
        // Still exactly the promised number of pixels, the missing ones clear.
        assert_eq!(image.rgba.len(), 2 * 4 * 4);
        assert_eq!(&image.rgba[8..], &[0u8; 24]);
    }

    #[test]
    fn a_row_shorter_than_the_width_is_padded_not_panicked() {
        let source = "/* XPM */\nstatic char *x[] = {\n\"4 1 2 1\",\n\
                      \". 	c #FF0000\",\n\"  	c None\",\n\".\",\n};\n";
        let image = decode(source.as_bytes()).unwrap();
        assert_eq!(image.rgba.len(), 4 * 4);
        assert_eq!(&image.rgba[..4], &[0xff, 0x00, 0x00, 0xff]);
        assert_eq!(&image.rgba[4..], &[0u8; 12]);
    }

    #[test]
    fn hex_colours_of_every_permitted_width() {
        assert_eq!(parse_colour("#F00"), Some([0xff, 0x00, 0x00, 0xff]));
        assert_eq!(parse_colour("#FF0000"), Some([0xff, 0x00, 0x00, 0xff]));
        assert_eq!(parse_colour("#FFF000000"), Some([0xff, 0x00, 0x00, 0xff]));
        assert_eq!(parse_colour("#FFFF00000000"), Some([0xff, 0x00, 0x00, 0xff]));
        assert_eq!(parse_colour("#abcdef"), Some([0xab, 0xcd, 0xef, 0xff]));
        // Not a whole number of channels, or not hex at all.
        assert_eq!(parse_colour("#FF00"), None);
        assert_eq!(parse_colour("#GGGGGG"), None);
        assert_eq!(parse_colour("%FFFF00000000"), None);
    }

    #[test]
    fn transparency_and_names() {
        assert_eq!(parse_colour("None"), Some(TRANSPARENT));
        assert_eq!(parse_colour("none"), Some(TRANSPARENT));
        assert_eq!(parse_colour("red"), Some([0xff, 0x00, 0x00, 0xff]));
        assert_eq!(parse_colour("Light Blue"), Some([0xad, 0xd8, 0xe6, 0xff]));
        // grey and gray are the same colour, and both take a percentage.
        assert_eq!(parse_colour("grey"), parse_colour("gray"));
        assert_eq!(parse_colour("gray100"), Some([0xff, 0xff, 0xff, 0xff]));
        assert_eq!(parse_colour("grey0"), Some([0x00, 0x00, 0x00, 0xff]));
        assert_eq!(parse_colour("gray50"), Some([0x80, 0x80, 0x80, 0xff]));
        assert_eq!(parse_colour("gray200"), None);
        assert_eq!(parse_colour("chartreuse"), None);
    }

    #[test]
    fn an_entry_may_give_several_keys_and_colour_wins() {
        // `s` names a symbolic colour and `m` the monochrome rendering; the
        // real colour is the one under `c`.
        let entry = b". s background m white c #123456";
        let (key, colour) = parse_colour_entry(entry, 1).unwrap();
        assert_eq!(key, b".");
        assert_eq!(colour, Some([0x12, 0x34, 0x56, 0xff]));

        // With no `c`, the monochrome value is better than nothing.
        let (_, colour) = parse_colour_entry(b". m white", 1).unwrap();
        assert_eq!(colour, Some([0xff, 0xff, 0xff, 0xff]));
    }

    #[test]
    fn a_palette_of_nothing_understood_is_refused() {
        // Every colour is an HSV specification, which is declined — so the
        // image would be entirely transparent and must not be offered as one.
        let source = "/* XPM */\nstatic char *x[] = {\n\"1 1 1 1\",\n\
                      \". c %FFFF00000000\",\n\".\",\n};\n";
        assert!(decode(source.as_bytes()).is_none());
    }

    #[test]
    fn comments_and_escapes_do_not_derail_the_scanner() {
        let literals = string_literals(br#"/* a "quote" in a comment */ "real" // "line"
            "second""#, 16);
        assert_eq!(literals, vec![&b"real"[..], &b"second"[..]]);

        // An escaped quote does not end the literal.
        let literals = string_literals(br#""a\"b" "c""#, 16);
        assert_eq!(literals, vec![&br#"a\"b"#[..], &b"c"[..]]);

        // Unterminated constructs stop the scan instead of running away.
        assert!(string_literals(b"/* never closed", 16).is_empty());
        assert!(string_literals(b"\"never closed", 16).is_empty());
    }

    #[test]
    fn the_literal_limit_is_respected() {
        let source = "\"a\" ".repeat(100);
        assert_eq!(string_literals(source.as_bytes(), 7).len(), 7);
    }

    #[test]
    fn multi_byte_keys_are_read_whole() {
        // Two characters per pixel, which is what every real icon uses.
        let source = "/* XPM */\nstatic char *x[] = {\n\"2 1 2 2\",\n\
                      \"ab	c #FF0000\",\n\"cd	c #00FF00\",\n\"abcd\",\n};\n";
        let image = decode(source.as_bytes()).unwrap();
        assert_eq!(
            image.rgba,
            vec![0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff]
        );
    }
}
