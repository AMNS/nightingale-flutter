//! NGL binary file writer
//!
//! Writes Nightingale N105 format files (inverse of reader.rs).
//!
//! Source references (OG Nightingale):
//! - FileSave.cp: WriteFile() (line 252)
//! - HeapFileIO.cp: WriteHeaps() (line 107), WriteObjHeap() (line 143),
//!   WriteSubHeaps() (line 442), WriteHeapHdr() (line 522)
//! - EndianUtils.cp: Endian conversion functions
//!
//! File layout (all big-endian):
//!   1. Version tag        (4 bytes): "N105"
//!   2. File timestamp     (4 bytes): seconds since 1904
//!   3. Document header    (72 bytes)
//!   4. Score header       (2148 bytes for N105)
//!   5. LASTtype sentinel  (2 bytes): must be 25
//!   6. String pool size   (4 bytes)
//!   7. String pool data   (variable)
//!   8. Subobject heaps    (types 0-23, each: count(2) + HEAP hdr(16) + data)
//!   9. Object heap        (type 24: count(2) + HEAP hdr(16) + size(4) + data)
//!   10. CoreMIDI device   (optional, 'cmdi' header + data)
//!   11. End marker        (4 bytes): 0x00000000
//!
//! CRITICAL IMPLEMENTATION NOTES:
//! - All multi-byte values must be written big-endian (PowerPC format)
//! - LINK values in memory are pointers; must convert to 1-based file indices
//! - Use double-conversion pattern: convert to big-endian, write, convert back
//!   (this preserves the in-memory InterpretedScore for continued use)
//! - Variable-length objects (SUPEROBJECT) are written at actual size, not padded
//! - Fixed-length subobjects are written at full heap obj_size

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::basic_types::Link;
use crate::ngl::interpret::InterpretedScore;
use crate::ngl::reader::NglVersion;

use super::error::{NglError, Result};

// =============================================================================
// Endian Conversion Helpers (FIX_END pattern from OG EndianUtils.cp)
// =============================================================================

/// Convert a i16 value to big-endian bytes (same as to_be_bytes)
#[allow(dead_code)]
fn fix_i16(val: i16) -> i16 {
    val.swap_bytes()
}

/// Convert a u16 value to big-endian bytes
#[allow(dead_code)]
fn fix_u16(val: u16) -> u16 {
    val.swap_bytes()
}

/// Convert a i32 value to big-endian bytes
#[allow(dead_code)]
fn fix_i32(val: i32) -> i32 {
    val.swap_bytes()
}

/// Convert a u32 value to big-endian bytes
#[allow(dead_code)]
fn fix_u32(val: u32) -> u32 {
    val.swap_bytes()
}

// =============================================================================
// Time Utilities (FileSave.cp WriteFile line 269)
// =============================================================================

/// Get current time as seconds since 1904 (Mac epoch)
/// OG Nightingale stores file timestamp as seconds since Jan 1, 1904
#[allow(dead_code)]
fn get_mac_timestamp() -> u32 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            // UNIX epoch is Jan 1, 1970
            // Mac epoch is Jan 1, 1904
            // Difference: 66 years = 2,082,844,800 seconds
            const MAC_EPOCH_OFFSET: u64 = 2_082_844_800;
            (duration.as_secs() + MAC_EPOCH_OFFSET) as u32
        }
        Err(_) => 0, // Fallback to epoch if time is unavailable
    }
}

// =============================================================================
// String Pool Serialization (StringPool.cp + EndianUtils.cp)
// =============================================================================

/// Serialize a collection of strings into binary format with string pool encoding.
///
/// Binary format for each string:
///   - 1 byte: 0x02 (string marker)
///   - 1 byte: string length (u8)
///   - N bytes: UTF-8 encoded string content
///
/// The string pool is a sequential concatenation of these encoded strings.
///
/// Source: OG StringPool.cp - stores text strings with length-prefixed encoding
/// Reference: OG EndianUtils.cp EndianFixStringPool() line 364
#[allow(dead_code)]
fn serialize_string_pool(strings: &[String]) -> Vec<u8> {
    let mut pool = Vec::new();

    for s in strings {
        pool.push(0x02); // String marker byte
        let len = s.len() as u8;
        pool.push(len);
        pool.extend_from_slice(s.as_bytes());
    }

    pool
}

/// Extract all strings from an InterpretedScore that need to be in the string pool.
///
/// Collects (in order):
/// 1. Font names from font_names table (deferred to document header phase)
/// 2. Text content from graphic_strings
/// 3. Tempo verbal and metronome strings
///
/// Returns a deduplicated list of unique strings, preserving insertion order.
/// Empty strings are skipped.
///
/// Source: OG FileSave.cp WriteFile() - collects all strings before serialization
#[allow(dead_code)]
fn collect_strings_from_score(score: &InterpretedScore) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::new();

    // Add font names (deferred - will be in document header implementation)
    // score.font_names is populated from DOCUMENTHDR.fontNameTbl
    // Will be added when we implement document header serialization

    // Add graphic text strings
    for text in score.graphic_strings.values() {
        if !text.is_empty() && seen.insert(text.clone()) {
            result.push(text.clone());
        }
    }

    // Add tempo strings (both verbal and metronome)
    for (verbal, metro) in score.tempo_strings.values() {
        if !verbal.is_empty() && seen.insert(verbal.clone()) {
            result.push(verbal.clone());
        }
        if !metro.is_empty() && seen.insert(metro.clone()) {
            result.push(metro.clone());
        }
    }

    result
}

// =============================================================================
// LINK Conversion Infrastructure (HeapFileIO.cp InitTrackingLinks + WriteObjHeap)
// =============================================================================

/// Maps memory LINK values to file indices for serialization.
///
/// During file writing, all LINK values (which are memory pointers/indices in the
/// in-memory InterpretedScore) must be converted to sequential file indices (1, 2, 3, ...).
/// This mapping stores that conversion and allows both directions:
/// - memory_to_file[link]: converts a memory LINK to its file index
/// - file_to_memory[index]: reverse mapping for verification/testing
///
/// Source: OG HeapFileIO.cp lines 765-776 (InitTrackingLinks)
///         and lines 167-233 (WriteObjHeap with backpatching)
#[derive(Debug, Clone)]
pub struct LinkMap {
    /// Map from memory LINK → file index (1-based)
    memory_to_file: HashMap<Link, Link>,
    /// Map from file index → memory LINK (for reverse lookups)
    file_to_memory: HashMap<Link, Link>,
}

impl LinkMap {
    /// Create a new empty LinkMap
    pub fn new() -> Self {
        Self {
            memory_to_file: HashMap::new(),
            file_to_memory: HashMap::new(),
        }
    }

    /// Build LinkMap by walking the object list in order.
    ///
    /// Algorithm (from OG HeapFileIO.cp lines 772-776):
    /// 1. Walk main object list (starting at head_l)
    /// 2. For each object, assign a sequential file index (1, 2, 3, ...)
    /// 3. Store bidirectional mapping
    ///
    /// This ensures:
    /// - File contains sequential indices independent of memory layout
    /// - In-memory pointers are preserved for continued use (double-conversion pattern)
    /// - Objects can be restored to original state after writing
    pub fn from_interpreted_score(score: &InterpretedScore) -> Result<Self> {
        let mut map = LinkMap::new();
        let mut file_index: Link = 1;

        // Walk all objects in heap order
        // Note: InterpretedScore.objects[0] is unused (index 0 reserved for NILINK)
        for memory_link in 1..score.objects.len() {
            map.insert(memory_link as Link, file_index);
            file_index += 1;
        }

        Ok(map)
    }

    /// Insert a mapping from memory LINK → file index
    fn insert(&mut self, memory_link: Link, file_index: Link) {
        self.memory_to_file.insert(memory_link, file_index);
        self.file_to_memory.insert(file_index, memory_link);
    }

    /// Convert a memory LINK to its file index.
    ///
    /// Returns the file index (1-based) for this memory link.
    /// If the link is not in the map, returns NILINK (0).
    ///
    /// Usage: Before writing an object field that contains a LINK,
    /// convert it using this method.
    pub fn convert_link(&self, memory_link: Link) -> Link {
        self.memory_to_file.get(&memory_link).copied().unwrap_or(0)
    }

    /// Get the count of mapped objects
    pub fn object_count(&self) -> usize {
        self.memory_to_file.len()
    }

    /// Iterator over (memory_link, file_index) pairs for testing
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (&Link, &Link)> {
        self.memory_to_file.iter()
    }
}

impl Default for LinkMap {
    fn default() -> Self {
        Self::new()
    }
}

/// NGL file writer
///
/// TODO: This is a foundational implementation. Full implementation requires:
/// - LINK → file index conversion for all object/subobject references
/// - Endian conversion for all multi-byte fields
/// - String pool serialization
/// - Object/subobject packing (inverse of unpack_*.rs modules)
/// - HEAP header generation with correct obj_size values
/// - Variable-length object sizing
///
/// Current status: SKELETON ONLY - not yet functional
pub struct NglWriter {
    _version: NglVersion,
}

impl NglWriter {
    /// Create a new writer for N105 format
    pub fn new() -> Self {
        Self {
            _version: NglVersion::N105,
        }
    }

    /// Write an InterpretedScore to disk as N105 format.
    ///
    /// Source: FileSave.cp WriteFile() (line 252)
    ///
    /// TODO: Complete implementation. This skeleton shows the structure but
    /// is not yet functional. Full implementation requires:
    /// 1. Document header serialization (72 bytes)
    /// 2. Score header serialization (2148 bytes for N105)
    /// 3. String pool serialization
    /// 4. Heap serialization with LINK conversion
    /// 5. Object/subobject packing (inverse of unpack_* modules)
    /// 6. Endian conversion
    pub fn write_to_file<P: AsRef<Path>>(&self, _score: &InterpretedScore, path: P) -> Result<()> {
        let mut file = File::create(path)?;

        // 1. Write version tag (4 bytes)
        file.write_all(b"N105")?;

        // 2. Write timestamp (4 bytes, big-endian)
        // TODO: Get current time as seconds since 1904
        let timestamp = 0u32; // Placeholder
        file.write_all(&timestamp.to_be_bytes())?;

        // 3. Write document header (72 bytes)
        // TODO: Serialize DOCUMENTHDR with endian conversion
        // Source: EndianUtils.cp EndianFixDocumentHdr() (line 149)
        let doc_header = vec![0u8; 72]; // Placeholder
        file.write_all(&doc_header)?;

        // 4. Write score header (2148 bytes for N105)
        // TODO: Serialize SCOREHEADER with endian conversion
        // Source: EndianUtils.cp EndianFixScoreHdr() (line 176)
        let score_header = vec![0u8; 2148]; // Placeholder
        file.write_all(&score_header)?;

        // 5. Write LASTtype sentinel (2 bytes, value 25)
        file.write_all(&25u16.to_be_bytes())?;

        // 6. Write string pool size + data
        // TODO: Serialize string pool with endian conversion
        // Source: StringPool.cp EndianFixStringPool() (line 364)
        let string_pool = Vec::new(); // Placeholder
        file.write_all(&(string_pool.len() as u32).to_be_bytes())?;
        file.write_all(&string_pool)?;

        // 7. Write all subobject heaps (types 0-23)
        // TODO: For each heap type:
        //   - Count objects
        //   - Write HEAP header (16 bytes) with endian conversion
        //   - Convert LINKs to file indices
        //   - Pack and write subobjects with endian conversion
        //   - Restore LINKs to memory pointers
        //
        // Source: HeapFileIO.cp WriteSubHeaps() (line 442)
        // Source: HeapFileIO.cp WriteHeapHdr() (line 522)
        // Source: EndianUtils.cp EndianFixSubobj() (line 374)

        // 8. Write object heap (type 24)
        // TODO:
        //   - Count objects (traverse main object list + master page list)
        //   - Write HEAP header (16 bytes)
        //   - Write total byte count (4 bytes)
        //   - Convert LINKs to file indices
        //   - Pack and write objects with endian conversion at actual size
        //   - Restore LINKs to memory pointers
        //
        // Source: HeapFileIO.cp WriteObjHeap() (line 143)
        // Source: HeapFileIO.cp WriteObject() (line 659)
        // Source: EndianUtils.cp EndianFixObject() (line 245)

        // 9. Write CoreMIDI device list (optional)
        // TODO: Write 'cmdi' header + device data if present
        // Source: FileSave.cp WriteFile() (line 252+)

        // 10. Write end marker (4 bytes, all zeros)
        file.write_all(&0u32.to_be_bytes())?;

        Ok(())
    }

    /// Write an InterpretedScore to a byte vector
    ///
    /// TODO: Implement once write_to_file is complete
    pub fn write_to_bytes(&self, _score: &InterpretedScore) -> Result<Vec<u8>> {
        Err(NglError::NotImplemented(
            "NGL writer not yet implemented".to_string(),
        ))
    }
}

impl Default for NglWriter {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: Helper functions needed for full implementation:
//
// fn write_heap_header(file: &mut File, obj_count: u16, obj_size: u16) -> io::Result<()>
// fn convert_links_to_indices(score: &mut InterpretedScore) -> LinkMap
// fn restore_links_from_indices(score: &mut InterpretedScore, map: &LinkMap)
// fn pack_sync(sync: &InterpretedSync) -> Vec<u8>
// fn pack_note(note: &InterpretedANote) -> Vec<u8>
// fn pack_staff(staff: &InterpretedStaff) -> Vec<u8>
// ... (one packer for each object/subobject type)
//
// fn endian_convert_u16(val: &mut u16)
// fn endian_convert_i16(val: &mut i16)
// fn endian_convert_u32(val: &mut u32)
// ... (endian converters for all field types)

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // =========================================================================
    // LinkMap tests
    // =========================================================================

    #[test]
    fn test_linkmap_creation() {
        let map = LinkMap::new();
        assert_eq!(map.object_count(), 0);
    }

    #[test]
    fn test_linkmap_convert_link_unmapped() {
        let map = LinkMap::new();
        // Unmapped links should return NILINK (0)
        assert_eq!(map.convert_link(5), 0);
        assert_eq!(map.convert_link(100), 0);
    }

    // DEFERRED: Integration test for LinkMap::from_interpreted_score()
    // This requires constructing a full InterpretedScore, which has many complex fields.
    // For now, test LinkMap.insert() directly as a substitute.
    // Full integration test should be added once we have fixture-based roundtrip tests.

    #[test]
    fn test_linkmap_sequential_file_indices() {
        let map = LinkMap::new();
        let mut map = map;

        // Manually insert some mappings
        map.insert(5, 1);
        map.insert(10, 2);
        map.insert(3, 3);

        // File indices should follow insertion order, not memory order
        assert_eq!(map.convert_link(5), 1);
        assert_eq!(map.convert_link(10), 2);
        assert_eq!(map.convert_link(3), 3);
    }

    // =========================================================================
    // Endian conversion tests
    // =========================================================================

    #[test]
    fn test_fix_u16_endian() {
        let val: u16 = 0x1234;
        let converted = fix_u16(val);
        assert_eq!(converted, 0x3412);
        // Double conversion should restore original
        let restored = fix_u16(converted);
        assert_eq!(restored, val);
    }

    #[test]
    fn test_fix_u32_endian() {
        let val: u32 = 0x12345678;
        let converted = fix_u32(val);
        assert_eq!(converted, 0x78563412);
        let restored = fix_u32(converted);
        assert_eq!(restored, val);
    }

    // =========================================================================
    // Timestamp tests
    // =========================================================================

    #[test]
    fn test_mac_timestamp() {
        let ts = get_mac_timestamp();
        // Timestamp should be nonzero and reasonable
        // Mac epoch started in 1904, so any timestamp after 1970 should be > 2M seconds
        assert!(ts > 2_000_000);
    }

    // =========================================================================
    // String pool serialization tests
    // =========================================================================

    #[test]
    fn test_serialize_string_pool_empty() {
        let pool = serialize_string_pool(&[]);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_serialize_string_pool_single() {
        let strings = vec!["hello".to_string()];
        let pool = serialize_string_pool(&strings);
        // Format: 0x02 <length> <bytes>
        // Expected: [0x02, 0x05, 'h', 'e', 'l', 'l', 'o']
        assert_eq!(pool[0], 0x02);
        assert_eq!(pool[1], 5);
        assert_eq!(&pool[2..], b"hello");
    }

    #[test]
    fn test_serialize_string_pool_multiple() {
        let strings = vec!["hi".to_string(), "bye".to_string()];
        let pool = serialize_string_pool(&strings);
        // Expected: [0x02, 0x02, 'h', 'i', 0x02, 0x03, 'b', 'y', 'e']
        assert_eq!(pool[0], 0x02);
        assert_eq!(pool[1], 2);
        assert_eq!(&pool[2..4], b"hi");
        assert_eq!(pool[4], 0x02);
        assert_eq!(pool[5], 3);
        assert_eq!(&pool[6..9], b"bye");
    }

    #[test]
    fn test_serialize_string_pool_long_string() {
        // Test with a longer string to verify length encoding
        let text = "This is a longer string";
        let strings = vec![text.to_string()];
        let pool = serialize_string_pool(&strings);

        assert_eq!(pool[0], 0x02);
        assert_eq!(pool[1] as usize, text.len());
        assert_eq!(&pool[2..2 + text.len()], text.as_bytes());
    }

    #[test]
    fn test_serialize_string_pool_preserves_order() {
        // Verify that string order is preserved in the pool
        let strings = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        let pool = serialize_string_pool(&strings);

        // Find each string in the pool
        let mut idx = 0;
        for s in &strings {
            assert_eq!(pool[idx], 0x02);
            assert_eq!(pool[idx + 1] as usize, s.len());
            assert_eq!(&pool[idx + 2..idx + 2 + s.len()], s.as_bytes());
            idx += 2 + s.len();
        }
    }

    // =========================================================================
    // Full implementation tests (deferred, marked as ignore)
    // =========================================================================

    #[test]
    #[ignore = "NGL writer not yet implemented"]
    fn test_write_basic_score() {
        // TODO: Create minimal InterpretedScore, write to bytes, verify structure
        // This test should:
        // 1. Create a minimal score (1 staff, 1 measure, 1 note)
        // 2. Write to bytes
        // 3. Read back with NglFile::read_from_bytes
        // 4. Verify round-trip fidelity
    }

    #[test]
    #[ignore = "NGL writer not yet implemented"]
    fn test_roundtrip_all_fixtures() {
        // TODO: For each NGL fixture:
        // 1. Read with NglFile::read_from_bytes
        // 2. Interpret with interpret_heap
        // 3. Write with NglWriter
        // 4. Read again and verify byte-for-byte equality
    }
}
