//! NGL binary file reader
//!
//! Reads Nightingale N103/N105 format files, extracting:
//! - Version tag and timestamp
//! - Document and score headers (raw bytes)
//! - String pool
//! - All 25 heaps (subobjects for types 0-23, objects for type 24)
//!
//! Source references:
//! - HeapFileIO.cp: ReadHeaps() (line 811), ReadObjHeap() (line 973),
//!   ReadSubHeap() (line 1049), ReadHeapHdr() (line 914)
//! - StringPool.cp: String pool format
//! - Ngale5ProgQuickRef-TN1.txt: File layout and offsets
//!
//! File layout (all big-endian):
//!   1. Version tag        (4 bytes): "N103" or "N105"
//!   2. File timestamp     (4 bytes): seconds since 1904
//!   3. Document header    (72 bytes)
//!   4. Score header       (2064 for N103, 2148 for N105)
//!   5. LASTtype sentinel  (2 bytes): must be 25
//!   6. String pool size   (4 bytes)
//!   7. String pool data   (variable)
//!   8. Subobject heaps    (types 0-23, each: count(2) + HEAP hdr(16) + data)
//!   9. Object heap        (type 24: count(2) + HEAP hdr(16) + size(4) + data)
//!
//! IMPORTANT LESSONS FROM PREVIOUS PORT ATTEMPTS:
//! - All multi-byte values are big-endian (PowerPC format)
//! - LINK values are 1-based (NILINK=0 means "no link")
//! - Slot 0 is never used; file data starts at slot 1
//! - We prepend one obj_size worth of zeros for slot 0 so LINK=K maps
//!   directly to data[K * obj_size]
//! - The obj_size comes from the 16-byte HEAP header in the file, NOT
//!   from a lookup table (the file is self-describing)

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use crate::defs::{HIGH_TYPE, OBJ_TYPE};

use super::error::{NglError, Result};

/// NGL file version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NglVersion {
    /// N101 format (legacy)
    N101,
    /// N102 format (legacy, Nightingale ~3.x)
    N102,
    /// N103 format (legacy, ~2002)
    N103,
    /// N105 format (Nightingale 5.6, current)
    N105,
}

impl NglVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            NglVersion::N101 => "N101",
            NglVersion::N102 => "N102",
            NglVersion::N103 => "N103",
            NglVersion::N105 => "N105",
        }
    }

    /// Returns the size of the score header for this version.
    ///
    /// Header sizes empirically determined from fixture files:
    /// - N101: 1412 bytes
    /// - N102: 1412 bytes (mostly; some older files use 976 bytes, handled via try-again logic)
    /// - N103: 2148 bytes
    /// - N105: 2148 bytes
    ///
    /// Source: FileOpen.cp ReadHeaders() line 169: sizeof(SCOREHEADER_N105)
    /// Source: NDocAndCnfgTypesN105.h NIGHTSCOREHEADER_N105 macro
    pub fn score_header_size(&self) -> usize {
        match self {
            NglVersion::N101 | NglVersion::N102 => 1412,
            NglVersion::N103 | NglVersion::N105 => 2148,
        }
    }
}

/// Raw heap data read from file.
///
/// For subobject heaps (types 0-23): data contains slot-0 padding + file objects.
/// For the object heap (type 24): data contains slot-0 padding + file objects.
/// In both cases, LINK=K maps to data[K * obj_size].
#[derive(Debug, Clone)]
pub struct HeapData {
    /// Heap index (object type 0-24)
    pub heap_index: u8,
    /// Number of objects in file (does NOT include slot 0)
    pub obj_count: u16,
    /// Object size in bytes (from the HEAP header in the file)
    pub obj_size: u16,
    /// Raw data bytes (includes slot 0 prepended as zeros)
    pub obj_data: Vec<u8>,
}

/// Parsed NGL file structure
#[derive(Debug, Clone)]
pub struct NglFile {
    /// File format version
    pub version: NglVersion,
    /// Timestamp when file was written (seconds since 1904, big-endian u32)
    pub timestamp: u32,
    /// Raw document header (72 bytes)
    pub doc_header_raw: Vec<u8>,
    /// Raw score header (2148 bytes for N105, 2064 for N103)
    pub score_header_raw: Vec<u8>,
    /// Raw string pool bytes
    pub string_pool: Vec<u8>,
    /// All heaps (types 0-24)
    pub heaps: Vec<HeapData>,
}

impl NglFile {
    /// Read an NGL file from disk.
    ///
    /// Reads the entire file into memory first (scores are small, typically
    /// 50-500 KB), then parses from a cursor. This avoids seeking issues.
    ///
    /// Source: HeapFileIO.cp ReadHeaps() (line 811)
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = fs::read(path)?;
        Self::read_from_bytes(&data)
    }

    /// Read an NGL file from a byte slice.
    pub fn read_from_bytes(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // 1. Version tag (4 bytes at offset 0)
        let mut version_buf = [0u8; 4];
        cursor.read_exact(&mut version_buf)?;
        let version_str = std::str::from_utf8(&version_buf)
            .map_err(|_| NglError::InvalidVersion(format!("{:?}", version_buf)))?;

        let version = match version_str {
            "N101" => NglVersion::N101,
            "N102" => NglVersion::N102,
            "N103" => NglVersion::N103,
            "N105" => NglVersion::N105,
            _ => return Err(NglError::InvalidVersion(version_str.to_string())),
        };

        // 2. Timestamp (4 bytes at offset 4, big-endian)
        let mut ts_buf = [0u8; 4];
        cursor.read_exact(&mut ts_buf)?;
        let timestamp = u32::from_be_bytes(ts_buf);

        // 3. Document header (72 bytes at offset 8)
        let mut doc_header_raw = vec![0u8; 72];
        cursor.read_exact(&mut doc_header_raw)?;

        // 4. Score header (version-dependent size, at offset 80)
        let score_header_size = version.score_header_size();
        let mut score_header_raw = vec![0u8; score_header_size];
        cursor.read_exact(&mut score_header_raw)?;

        // 5. LASTtype sentinel (2 bytes, big-endian, should be 25)
        let mut lasttype_buf = [0u8; 2];
        cursor.read_exact(&mut lasttype_buf)?;
        let lasttype = u16::from_be_bytes(lasttype_buf);
        if lasttype != HIGH_TYPE as u16 {
            return Err(NglError::InvalidLastType(lasttype));
        }

        // 6. String pool size (4 bytes, big-endian)
        let mut pool_len_buf = [0u8; 4];
        cursor.read_exact(&mut pool_len_buf)?;
        let pool_len = u32::from_be_bytes(pool_len_buf) as usize;

        // 7. String pool data
        let mut string_pool = vec![0u8; pool_len];
        if pool_len > 0 {
            cursor.read_exact(&mut string_pool)?;
        }

        // 8-9. Read all 25 heaps
        // C++ ReadHeaps() reads subobject heaps 0..23, then object heap 24
        let mut heaps = Vec::with_capacity(HIGH_TYPE as usize);
        for heap_index in 0..HIGH_TYPE {
            let is_object_heap = heap_index == OBJ_TYPE;
            let heap = read_heap(&mut cursor, heap_index, is_object_heap)?;
            heaps.push(heap);
        }

        Ok(NglFile {
            version,
            timestamp,
            doc_header_raw,
            score_header_raw,
            string_pool,
            heaps,
        })
    }
}

/// Read a single heap from the file stream.
///
/// Each heap in the file has:
/// - 2 bytes: nFObjs (number of objects written to file, big-endian)
/// - 16 bytes: HEAP header struct
///     - `[0..3]` Handle (runtime pointer, ignored)
///     - `[4..5]` objSize (i16, big-endian)
///     - `[6..7]` type (i16, big-endian)
///     - `[8..9]` firstFree (u16)
///     - `[10..11]` nObjs (u16)
///     - `[12..13]` nFree (u16)
///     - `[14..15]` lockLevel (u16)
/// - For subobject heaps (0..23): nFObjs * objSize bytes of data
/// - For object heap (24): 4 bytes total size, then that many bytes of data
///
/// Source: HeapFileIO.cp ReadHeapHdr() (line 914), ReadSubHeap() (line 1049),
///         ReadObjHeap() (line 973)
///
/// CRITICAL: We prepend one obj_size worth of zeros for slot 0, because
/// Nightingale's LINK values are 1-based (NILINK=0 means "no link").
/// This way LINK=K maps directly to data[K * obj_size].
fn read_heap<R: Read>(reader: &mut R, heap_index: u8, is_object_heap: bool) -> Result<HeapData> {
    // Read nFObjs (2 bytes, big-endian)
    // Source: ReadHeapHdr() calls FSRead for nFObjs
    let mut count_buf = [0u8; 2];
    reader.read_exact(&mut count_buf)?;
    let file_obj_count = u16::from_be_bytes(count_buf);

    // Read 16-byte HEAP header struct
    // Source: ReadHeapHdr() calls FSRead for HEAP struct
    let mut heap_hdr = [0u8; 16];
    reader.read_exact(&mut heap_hdr)?;

    // Extract obj_size from the HEAP header (bytes 4-5, big-endian i16)
    let obj_size = u16::from_be_bytes([heap_hdr[4], heap_hdr[5]]);

    // Read heap data
    let obj_data = if is_object_heap {
        // Object heap (type 24): variable-length entries preceded by 4-byte total size
        // Source: ReadObjHeap() reads sizeAllObjsFile (4 bytes) then FSRead(data)
        let mut size_buf = [0u8; 4];
        reader.read_exact(&mut size_buf)?;
        let total_size = u32::from_be_bytes(size_buf) as usize;

        // Prepend slot 0 (obj_size bytes of zeros) + read file data into slots 1..N
        // Source: ReadObjHeap() line 1020: "pLink1 = *(objHeap->block); pLink1 += objHeap->objSize;"
        let obj_sz = obj_size as usize;
        let mut buf = vec![0u8; obj_sz + total_size]; // slot 0 + file data
        if total_size > 0 {
            reader.read_exact(&mut buf[obj_sz..])?;
        }
        buf
    } else if file_obj_count > 0 && obj_size > 0 {
        // Subobject heap (types 0-23): fixed-size entries
        // Source: ReadSubHeap() line 1077: "sizeAllInFile = nFObjs*subObjLength_5[iHp]"
        // Source: ReadSubHeap() line 1100: "pLink1 = *(myHeap->block); pLink1 += myHeap->objSize;"
        let obj_sz = obj_size as usize;
        let file_bytes = file_obj_count as usize * obj_sz;

        // Prepend slot 0 (obj_size bytes of zeros) + read file data into slots 1..N
        let mut buf = vec![0u8; obj_sz + file_bytes]; // slot 0 + file entries
        reader.read_exact(&mut buf[obj_sz..])?; // read into slots 1..N
        buf
    } else {
        // Empty heap (no objects and/or zero obj_size)
        Vec::new()
    };

    Ok(HeapData {
        heap_index,
        obj_count: file_obj_count,
        obj_size,
        obj_data,
    })
}

/// Decode a string from the string pool at the given offset.
///
/// String pool format: `02 <length_byte> <string_bytes>` entries, concatenated.
/// Offset 0 is the canonical empty string.
///
/// NGL files use Mac Roman encoding (the native encoding on classic Mac OS).
/// Bytes 0x00-0x7F map to ASCII; bytes 0x80-0xFF map to specific Unicode
/// code points via the MAC_ROMAN_HIGH table.
///
/// Source: StringPool.cp CAddrInPool() (line 551)
pub fn decode_string(pool: &[u8], offset: i32) -> Option<String> {
    if offset <= 0 || (offset as usize) >= pool.len() {
        return Some(String::new()); // Empty string for invalid/zero offsets
    }

    let idx = offset as usize;

    // Read type byte (should be 2 for C string)
    let type_byte = pool[idx];
    if type_byte != 2 {
        return None; // Not a C string
    }

    // Read length byte
    if idx + 1 >= pool.len() {
        return None;
    }
    let len = pool[idx + 1] as usize;

    // Read string bytes
    if idx + 2 + len > pool.len() {
        return None;
    }

    let string_bytes = &pool[idx + 2..idx + 2 + len];
    Some(mac_roman_to_string(string_bytes))
}

/// Convert a Mac Roman byte slice to a UTF-8 String.
///
/// Bytes 0x00-0x7F are ASCII-identical. Bytes 0x80-0xFF are mapped to their
/// Unicode equivalents per Apple's Mac OS Roman encoding specification.
pub fn mac_roman_to_string(bytes: &[u8]) -> String {
    // Fast path: if all bytes are ASCII, just return as-is
    if bytes.iter().all(|&b| b < 0x80) {
        // SAFETY: all bytes < 0x80 are valid UTF-8
        return String::from_utf8(bytes.to_vec()).unwrap();
    }

    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        if b < 0x80 {
            s.push(b as char);
        } else {
            s.push(MAC_ROMAN_HIGH[(b - 0x80) as usize]);
        }
    }
    s
}

/// Mac Roman high-byte (0x80-0xFF) to Unicode mapping.
/// Source: Apple Mac OS Roman encoding specification.
#[rustfmt::skip]
const MAC_ROMAN_HIGH: [char; 128] = [
    // 0x80-0x8F
    '\u{00C4}', '\u{00C5}', '\u{00C7}', '\u{00C9}', '\u{00D1}', '\u{00D6}', '\u{00DC}', '\u{00E1}',
    '\u{00E0}', '\u{00E2}', '\u{00E4}', '\u{00E3}', '\u{00E5}', '\u{00E7}', '\u{00E9}', '\u{00E8}',
    // 0x90-0x9F
    '\u{00EA}', '\u{00EB}', '\u{00ED}', '\u{00EC}', '\u{00EE}', '\u{00EF}', '\u{00F1}', '\u{00F3}',
    '\u{00F2}', '\u{00F4}', '\u{00F6}', '\u{00F5}', '\u{00FA}', '\u{00F9}', '\u{00FB}', '\u{00FC}',
    // 0xA0-0xAF
    '\u{2020}', '\u{00B0}', '\u{00A2}', '\u{00A3}', '\u{00A7}', '\u{2022}', '\u{00B6}', '\u{00DF}',
    '\u{00AE}', '\u{00A9}', '\u{2122}', '\u{00B4}', '\u{00A8}', '\u{2260}', '\u{00C6}', '\u{00D8}',
    // 0xB0-0xBF
    '\u{221E}', '\u{00B1}', '\u{2264}', '\u{2265}', '\u{00A5}', '\u{00B5}', '\u{2202}', '\u{2211}',
    '\u{220F}', '\u{03C0}', '\u{222B}', '\u{00AA}', '\u{00BA}', '\u{03A9}', '\u{00E6}', '\u{00F8}',
    // 0xC0-0xCF
    '\u{00BF}', '\u{00A1}', '\u{00AC}', '\u{221A}', '\u{0192}', '\u{2248}', '\u{2206}', '\u{00AB}',
    '\u{00BB}', '\u{2026}', '\u{00A0}', '\u{00C0}', '\u{00C3}', '\u{00D5}', '\u{0152}', '\u{0153}',
    // 0xD0-0xDF
    '\u{2013}', '\u{2014}', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '\u{00F7}', '\u{25CA}',
    '\u{00FF}', '\u{0178}', '\u{2044}', '\u{20AC}', '\u{2039}', '\u{203A}', '\u{FB01}', '\u{FB02}',
    // 0xE0-0xEF
    '\u{2021}', '\u{00B7}', '\u{201A}', '\u{201E}', '\u{2030}', '\u{00C2}', '\u{00CA}', '\u{00C1}',
    '\u{00CB}', '\u{00C8}', '\u{00CD}', '\u{00CE}', '\u{00CF}', '\u{00CC}', '\u{00D3}', '\u{00D4}',
    // 0xF0-0xFF
    '\u{F8FF}', '\u{00D2}', '\u{00DA}', '\u{00DB}', '\u{00D9}', '\u{0131}', '\u{02C6}', '\u{02DC}',
    '\u{00AF}', '\u{02D8}', '\u{02D9}', '\u{02DA}', '\u{00B8}', '\u{02DD}', '\u{02DB}', '\u{02C7}',
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ngl_version() {
        assert_eq!(NglVersion::N105.as_str(), "N105");
        assert_eq!(NglVersion::N103.as_str(), "N103");
        assert_eq!(NglVersion::N105.score_header_size(), 2148);
        // N103 files we have use the same 2148-byte N105 layout
        assert_eq!(NglVersion::N103.score_header_size(), 2148);
    }

    #[test]
    fn test_decode_empty_string() {
        let pool = vec![0, 0]; // Empty pool
        assert_eq!(decode_string(&pool, 0), Some(String::new()));
        assert_eq!(decode_string(&pool, -1), Some(String::new()));
    }

    #[test]
    fn test_decode_string() {
        // Format: type_byte(2), length, string_bytes
        let pool = vec![
            0, 0, // Empty string at offset 0
            2, 5, b'H', b'e', b'l', b'l', b'o', // "Hello" at offset 2
        ];
        assert_eq!(decode_string(&pool, 2), Some("Hello".to_string()));
    }

    #[test]
    fn test_decode_string_mac_roman() {
        // Mac Roman 0x8A = ä, 0x85 = Ö, 0x9A = ü
        // "Für Elise" in Mac Roman: F=0x46, ü=0x9F, r=0x72
        let pool = vec![
            0, 0, 2, 9, 0x46, 0x9F, 0x72, 0x20, 0x45, 0x6C, 0x69, 0x73, 0x65,
        ];
        assert_eq!(decode_string(&pool, 2), Some("Für Elise".to_string()));
    }

    #[test]
    fn test_mac_roman_to_string_ascii() {
        assert_eq!(mac_roman_to_string(b"Hello"), "Hello");
    }

    #[test]
    fn test_mac_roman_to_string_high_bytes() {
        // 0x80 = Ä, 0x87 = á, 0x8E = é, 0xD2 = \u{201C} (left double quote)
        let bytes = [0x80, 0x87, 0x8E, 0xD2];
        assert_eq!(mac_roman_to_string(&bytes), "Äáé\u{201C}");
    }
}
