// SPDX-License-Identifier: GPL-3.0

//! Reading the metadata dictionary out of a Flatpak bundle.
//!
//! A `.flatpak` bundle is a single GVariant value: an OSTree static-delta
//! superblock whose first member is an `a{sv}` dictionary. Flatpak puts
//! everything worth knowing about the bundle in that dictionary — the ref, the
//! `metadata` file, the AppStream data, the icons, the installed size — and
//! reads it back the same way, so this is the format's own answer to "what is
//! in this file", not a guess made from the outside.
//!
//! There is deliberately no dependency on GLib here, and no attempt at a
//! general GVariant implementation. Only the shapes this one header uses are
//! handled, which is a few hundred bytes of framing rules rather than a
//! type-system.
//!
//! **The dictionary is not read whole.** The compressed payload of the entire
//! bundle is stored as further entries in the same dictionary, so "read the
//! `a{sv}`" would mean reading a file that is routinely hundreds of megabytes.
//! Instead the framing is walked with seeks, each entry's key is recovered from
//! a short probe at its start, and only the handful of entries that are both
//! wanted and small are read.
//!
//! ## The framing rules that matter
//!
//! * A container's *offset size* is the smallest of 1, 2, 4 or 8 bytes that can
//!   hold its total serialised length.
//! * A **tuple** stores the end offset of each variable-size member except the
//!   last, at the end of its buffer, **in reverse order** — so the offset read
//!   from the very end of the file is the end of member zero, the dictionary.
//! * An **array** stores the end offset of every element at the end of its
//!   buffer, **in forward order**. The last of those offsets is therefore also
//!   the position at which the offset table itself begins, which is what gives
//!   the element count.
//! * A `{sv}` entry is a two-member tuple: the key at offset zero, then the
//!   variant padded to its 8-byte alignment, then a single offset giving the
//!   end of the key.
//! * A **variant** is its value, a zero byte, then its type signature.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use super::{Error, Result};
use crate::constants::{
    BUNDLE_KEY_PROBE_BYTES, BUNDLE_MAX_HEADER_ENTRIES, BUNDLE_MAX_VALUE_BYTES,
};
use crate::debug::FLATPAK;
use crate::debug_log;

/// One value out of the metadata dictionary.
///
/// Only the signatures Flatpak actually writes are given a variant of their
/// own; anything else keeps its signature so a caller can report the field
/// without pretending to understand it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// `s`
    Text(String),
    /// `ay`
    Bytes(Vec<u8>),
    /// `y`
    Byte(u8),
    /// `u`
    U32(u32),
    /// `t`
    U64(u64),
    /// Some other signature, recorded but not decoded.
    Other(String),
}

/// The metadata dictionary of a bundle, in file order.
#[derive(Clone, Debug, Default)]
pub struct Header {
    entries: Vec<(String, Value)>,
}

impl Header {
    fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Text(text) => Some(text.as_str()),
            _ => None,
        }
    }

    pub fn bytes(&self, key: &str) -> Option<&[u8]> {
        match self.get(key)? {
            Value::Bytes(bytes) => Some(bytes.as_slice()),
            _ => None,
        }
    }

    /// A `t` field, also accepting a `u` where Flatpak has written the narrower
    /// type for a value that fits.
    pub fn number(&self, key: &str) -> Option<u64> {
        match self.get(key)? {
            Value::U64(value) => Some(*value),
            Value::U32(value) => Some(u64::from(*value)),
            _ => None,
        }
    }

    /// Whether the dictionary carries the marker Flatpak writes into every
    /// bundle it produces. Its absence means this is some other GVariant that
    /// happened to parse, which is worth refusing rather than reporting on.
    pub fn is_flatpak_bundle(&self) -> bool {
        matches!(self.get("flatpak"), Some(Value::U32(_)))
    }
}

/// Read the metadata dictionary at the start of `path`.
pub fn read_header(path: &Path) -> Result<Header> {
    let mut file = File::open(path).map_err(|error| Error::Parse {
        detail: format!("cannot open {}: {error}", path.display()),
    })?;

    let total = file
        .metadata()
        .map_err(|error| Error::Parse {
            detail: format!("cannot size {}: {error}", path.display()),
        })?
        .len();

    // The outer tuple's last offset is the end of member zero, the dictionary.
    let outer_offset_size = offset_size(total)?;
    let dictionary_end = read_offset(&mut file, total - outer_offset_size, outer_offset_size)?;
    if dictionary_end == 0 || dictionary_end > total {
        return Err(malformed("dictionary end is outside the file"));
    }

    // The array's own offset table sits at its end, and its last entry doubles
    // as the position where that table starts.
    let array_offset_size = offset_size(dictionary_end)?;
    let table_start = read_offset(
        &mut file,
        dictionary_end - array_offset_size,
        array_offset_size,
    )?;
    if table_start >= dictionary_end {
        return Err(malformed("offset table starts past the dictionary"));
    }

    let table_bytes = dictionary_end - table_start;
    if table_bytes % array_offset_size != 0 {
        return Err(malformed("offset table is not a whole number of offsets"));
    }
    let count = (table_bytes / array_offset_size) as usize;
    if count > BUNDLE_MAX_HEADER_ENTRIES {
        return Err(malformed("implausible number of header entries"));
    }

    let table = read_exact_at(&mut file, table_start, table_bytes)?;
    debug_log!(
        FLATPAK,
        "bundle header: {count} entries, dictionary ends at {dictionary_end} of {total}"
    );

    let mut entries = Vec::new();
    let mut previous_end = 0u64;
    for index in 0..count {
        let start = align8(previous_end);
        let end = decode_offset(&table[index * array_offset_size as usize..], array_offset_size);
        if end <= start || end > table_start {
            return Err(malformed("header entry runs outside the dictionary"));
        }
        previous_end = end;

        if let Some(entry) = read_entry(&mut file, start, end)? {
            entries.push(entry);
        }
    }

    Ok(Header { entries })
}

/// Read one `{sv}` dictionary entry, or `None` when it is of no interest.
///
/// Large entries are identified and skipped without their value ever being
/// read: the bundle's compressed payload lives in entries of exactly this
/// shape, keyed by its position in the repository.
fn read_entry(file: &mut File, start: u64, end: u64) -> Result<Option<(String, Value)>> {
    let length = end - start;

    // The key is a NUL-terminated string at the very start of the entry, so a
    // short read identifies the entry without committing to its size.
    let probe = read_exact_at(file, start, length.min(BUNDLE_KEY_PROBE_BYTES))?;
    let Some(nul) = probe.iter().position(|byte| *byte == 0) else {
        return Ok(None);
    };
    let key = String::from_utf8_lossy(&probe[..nul]).into_owned();
    let key_end = nul as u64 + 1;

    if length > BUNDLE_MAX_VALUE_BYTES {
        debug_log!(FLATPAK, "skipping header entry {key:?} of {length} bytes");
        return Ok(None);
    }

    let entry = read_exact_at(file, start, length)?;
    let entry_offset_size = offset_size(length)?;

    // The entry's single framing offset repeats the end of the key, which is a
    // free consistency check on everything read so far.
    let stored = decode_offset(&entry[(length - entry_offset_size) as usize..], entry_offset_size);
    if stored != key_end {
        return Err(malformed("dictionary entry key length disagrees with its framing"));
    }

    let value_start = align8(key_end);
    let value_end = length - entry_offset_size;
    if value_start >= value_end {
        return Ok(None);
    }

    Ok(Some((
        key,
        decode_variant(&entry[value_start as usize..value_end as usize]),
    )))
}

/// Split a serialised variant into its value and signature and decode it.
fn decode_variant(buffer: &[u8]) -> Value {
    // The signature follows the last zero byte. Searching from the end is what
    // makes this safe for values that contain zeros themselves, which every
    // embedded PNG does.
    let Some(separator) = buffer.iter().rposition(|byte| *byte == 0) else {
        return Value::Other(String::new());
    };
    let value = &buffer[..separator];
    let signature = String::from_utf8_lossy(&buffer[separator + 1..]).into_owned();

    match signature.as_str() {
        // A serialised string carries its own terminating NUL.
        "s" => Value::Text(
            String::from_utf8_lossy(value.strip_suffix(&[0]).unwrap_or(value)).into_owned(),
        ),
        "ay" => Value::Bytes(value.to_vec()),
        "y" if value.len() == 1 => Value::Byte(value[0]),
        "u" if value.len() == 4 => Value::U32(u32::from_le_bytes([
            value[0], value[1], value[2], value[3],
        ])),
        "t" if value.len() == 8 => Value::U64(u64::from_le_bytes([
            value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
        ])),
        _ => Value::Other(signature),
    }
}

/// The number of bytes GVariant uses for a framing offset in a container of
/// `size` bytes.
fn offset_size(size: u64) -> Result<u64> {
    Ok(match size {
        0 => return Err(malformed("zero-length container")),
        1..=0xff => 1,
        0x100..=0xffff => 2,
        0x1_0000..=0xffff_ffff => 4,
        _ => 8,
    })
}

/// GVariant offsets are unsigned little-endian integers of the container's
/// offset size.
fn decode_offset(buffer: &[u8], size: u64) -> u64 {
    let mut value = 0u64;
    for index in (0..size as usize).rev() {
        value = (value << 8) | u64::from(buffer[index]);
    }
    value
}

fn read_offset(file: &mut File, position: u64, size: u64) -> Result<u64> {
    let buffer = read_exact_at(file, position, size)?;
    Ok(decode_offset(&buffer, size))
}

fn read_exact_at(file: &mut File, position: u64, length: u64) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(position))
        .map_err(|error| malformed(&format!("seek to {position} failed: {error}")))?;
    let mut buffer = vec![0u8; length as usize];
    file.read_exact(&mut buffer)
        .map_err(|error| malformed(&format!("short read at {position}: {error}")))?;
    Ok(buffer)
}

/// Round up to the 8-byte boundary every dictionary entry and variant starts on.
fn align8(value: u64) -> u64 {
    (value + 7) & !7
}

fn malformed(detail: &str) -> Error {
    Error::Parse {
        detail: format!("not a readable Flatpak bundle: {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_sizes_follow_the_container_length() {
        assert_eq!(offset_size(1).unwrap(), 1);
        assert_eq!(offset_size(0xff).unwrap(), 1);
        assert_eq!(offset_size(0x100).unwrap(), 2);
        assert_eq!(offset_size(0xffff).unwrap(), 2);
        assert_eq!(offset_size(0x1_0000).unwrap(), 4);
        assert_eq!(offset_size(0xffff_ffff).unwrap(), 4);
        assert_eq!(offset_size(0x1_0000_0000).unwrap(), 8);
        assert!(offset_size(0).is_err());
    }

    #[test]
    fn decodes_the_signatures_a_bundle_uses() {
        // value, 0x00, signature — the layout every variant has.
        assert_eq!(
            decode_variant(b"app/org.example.Hello/x86_64/stable\0\0s"),
            Value::Text("app/org.example.Hello/x86_64/stable".to_string())
        );
        assert_eq!(decode_variant(b"\0\0s"), Value::Text(String::new()));
        assert_eq!(decode_variant(&[0x01, 0x00, 0x89, 0xe5, 0x00, b'u']), Value::U32(0xe589_0001));
        assert_eq!(decode_variant(&[0x6c, 0x00, b'y']), Value::Byte(b'l'));
        assert_eq!(
            decode_variant(&[0x10, 0, 0, 0, 0, 0, 0, 0, 0x00, b't']),
            Value::U64(16)
        );
    }

    #[test]
    fn a_value_containing_zeros_keeps_them() {
        // A PNG begins with a zero-rich signature, and searching for the
        // separator from the front would truncate it to nothing.
        let png = [0x89u8, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00];
        let mut buffer = png.to_vec();
        buffer.extend_from_slice(b"\0ay");
        assert_eq!(decode_variant(&buffer), Value::Bytes(png.to_vec()));
    }

    #[test]
    fn offsets_are_little_endian() {
        assert_eq!(decode_offset(&[0x0f, 0x00], 2), 15);
        assert_eq!(decode_offset(&[0x11, 0x0d], 2), 3345);
        assert_eq!(decode_offset(&[0xff], 1), 255);
    }

    #[test]
    fn alignment_rounds_up_to_eight() {
        assert_eq!(align8(0), 0);
        assert_eq!(align8(1), 8);
        assert_eq!(align8(8), 8);
        assert_eq!(align8(15), 16);
    }
}
