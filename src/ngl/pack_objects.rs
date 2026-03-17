//! N105 object packing (serialization) for types 0-24.
//!
//! This module provides the inverse of interpret.rs: converting typed Rust objects
//! back to raw N105 binary format for NGL file writing.
//!
//! Object heap serialization is more complex than subobject heap serialization because:
//! 1. Objects are variable-length (not padded to fixed obj_size)
//! 2. LINK pointers must be converted to sequential file indices (1-based)
//! 3. Backpatching is required for LINK fields before writing
//! 4. Special cross-reference handling for SLUR, GRAPHIC, TEMPO, DYNAMIC, ENDING, RPTEND
//! 5. Size backpatching: total heap size is written after all objects serialized
//!
//! Key difference from subobject packing:
//! - Subobjects: Fixed-length records (obj_size from heap header), padding bytes included
//! - Objects: Variable-length records (actual size varies by type), no padding
//!
//! Source: Inverse of interpret.rs and OG HeapFileIO.cp WriteObjHeap (lines 143-313)

#![allow(dead_code)]

use crate::basic_types::Link;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore};
use crate::obj_types::ObjectHeader;
use std::collections::HashMap;

// =============================================================================
// Link Mapping (OG HeapFileIO.cp objA array equivalent)
// =============================================================================

/// LinkMap: Maps in-memory LINK values (pointers) to sequential file indices (1-based).
///
/// In OG Nightingale, the objA array (HeapFileIO.cp:167-180) maps every heap object
/// to a sequential index. In our Rust implementation, we use this HashMap to achieve
/// the same effect:
/// - Key: in-memory Link value (from InterpretedObject.index or cross-references)
/// - Value: sequential file index (starting at 1 for first object)
///
/// This mapping is used during backpatching to convert all LINK fields before writing.
struct LinkMap {
    /// Map from in-memory Link to file index
    map: HashMap<Link, Link>,
    /// Next available file index (incremented as objects are added)
    next_index: Link,
}

impl LinkMap {
    /// Create a new LinkMap, starting with index 1 (0 reserved for NILINK).
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            next_index: 1,
        }
    }

    /// Register a Link value and return its file index.
    fn register(&mut self, in_memory_link: Link) -> Link {
        if in_memory_link == 0 {
            // NILINK (null pointer) stays 0
            return 0;
        }
        if let Some(&file_index) = self.map.get(&in_memory_link) {
            file_index
        } else {
            let file_index = self.next_index;
            self.map.insert(in_memory_link, file_index);
            self.next_index += 1;
            file_index
        }
    }

    /// Convert a Link value using the mapping. Returns 0 if NILINK, or mapped index.
    fn convert(&self, in_memory_link: Link) -> Link {
        if in_memory_link == 0 {
            0
        } else {
            self.map.get(&in_memory_link).copied().unwrap_or(0)
        }
    }
}

// =============================================================================
// Object Header Serialization (N105 format)
// =============================================================================

/// Pack N105 OBJECTHEADER_5 to raw bytes (12 bytes total).
///
/// On-disk layout (big-endian):
/// ```text
/// Offset  Size  Field
/// 0       2     right (LINK)
/// 2       2     left (LINK)
/// 4       2     firstSubObj (LINK)
/// 6       2     xd (DDIST)
/// 8       2     yd (DDIST)
/// 10      1     obj_type
/// 11      1     Bitfield: selected:1 | visible:1 | soft:1 | valid:1 | tweaked:1 | spare:1 | filler:2
/// ```
///
/// Note: OBJECTHEADER in memory is larger (28 bytes), but we only write the
/// essential 12 bytes. Remaining fields (rect, rel_size, n_entries) are written
/// by type-specific packers or as part of the object union.
fn pack_objectheader_n105(header: &ObjectHeader, link_map: &LinkMap, buf: &mut [u8]) {
    // Offset 0-1: right (LINK, big-endian)
    let right_idx = link_map.convert(header.right);
    buf[0..2].copy_from_slice(&right_idx.to_be_bytes());

    // Offset 2-3: left (LINK, big-endian)
    let left_idx = link_map.convert(header.left);
    buf[2..4].copy_from_slice(&left_idx.to_be_bytes());

    // Offset 4-5: firstSubObj (LINK, big-endian)
    let first_sub_obj_idx = link_map.convert(header.first_sub_obj);
    buf[4..6].copy_from_slice(&first_sub_obj_idx.to_be_bytes());

    // Offset 6-7: xd (DDIST, big-endian)
    buf[6..8].copy_from_slice(&header.xd.to_be_bytes());

    // Offset 8-9: yd (DDIST, big-endian)
    buf[8..10].copy_from_slice(&header.yd.to_be_bytes());

    // Offset 10: obj_type
    buf[10] = header.obj_type as u8;

    // Offset 11: Bitfield selected:1 | visible:1 | soft:1 | valid:1 | tweaked:1 | spare:1 | filler:2
    let mut b11: u8 = 0;
    if header.selected {
        b11 |= 0x80; // bit 7
    }
    if header.visible {
        b11 |= 0x40; // bit 6
    }
    if header.soft {
        b11 |= 0x20; // bit 5
    }
    if header.valid {
        b11 |= 0x10; // bit 4
    }
    if header.tweaked {
        b11 |= 0x08; // bit 3
    }
    if header.spare_flag {
        b11 |= 0x04; // bit 2
    }
    buf[11] = b11;
}

// =============================================================================
// Object Serialization (placeholder for type-specific implementation)
// =============================================================================

/// Serialize a single object to bytes, returning the packed binary data.
///
/// Currently a placeholder. In full implementation, each object type (0-24)
/// will have specific binary layout that extends the OBJECTHEADER with
/// type-specific fields.
///
/// Source: OG NObjTypesN105.h SUPEROBJ_5 union (lines 43-210)
fn pack_object_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    // Start with enough space for OBJECTHEADER_5 (12 bytes)
    let mut buf = vec![0u8; 12];

    // Pack the common OBJECTHEADER_5 first
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // TODO: Type-specific packing based on obj.data variant
    // For now, just return the header. Full implementation will:
    // 1. Match on obj.data to determine type
    // 2. Extend buf with type-specific fields
    // 3. Handle unions (e.g., SYNC vs PAGE have different layouts)
    // 4. Pack embedded structures (Rect, Point, DRect, etc.)
    // 5. Handle cross-references for special types (SLUR, GRAPHIC, etc.)

    buf
}

// =============================================================================
// Object Heap Serialization (main entry point)
// =============================================================================

/// Serialize the object heap to raw bytes with link backpatching and size calculation.
///
/// Algorithm (based on OG HeapFileIO.cp WriteObjHeap lines 143-313):
/// 1. Build LinkMap: Register all object indices (head_l and all InterpretedObject.index values)
/// 2. Backpatch: For each object, convert LINK fields using LinkMap before packing
/// 3. Write: Pack each object to bytes, accumulating in output buffer
/// 4. Size backpatch: Calculate total size and write to heap header
/// 5. Restore: (In memory - not done here, caller is responsible)
///
/// Returns a tuple (object_heap_bytes, total_size_with_header)
#[allow(private_interfaces)]
pub fn serialize_object_heap(score: &InterpretedScore, mut link_map: LinkMap) -> (Vec<u8>, u32) {
    // Step 1: Register all object indices in LinkMap
    // This ensures every object gets a sequential file index
    link_map.register(score.head_l); // Head object
    for obj in &score.objects {
        link_map.register(obj.index);
    }

    // Step 2 & 3: Pack all objects with backpatched links
    let mut object_data = Vec::new();
    for obj in &score.objects {
        let packed = pack_object_n105(obj, &link_map);
        object_data.extend_from_slice(&packed);
    }

    // Calculate total size: 4-byte size field + object data
    let total_size = 4 + object_data.len() as u32;

    // Build final heap: size field (4 bytes) + object data
    let mut heap_bytes = Vec::new();
    heap_bytes.extend_from_slice(&total_size.to_be_bytes());
    heap_bytes.extend_from_slice(&object_data);

    (heap_bytes, total_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj_types::ObjectHeader;

    #[test]
    fn test_link_map_basic() {
        let mut map = LinkMap::new();

        // Register some links
        let idx1 = map.register(42);
        let idx2 = map.register(100);
        let idx3 = map.register(42); // Duplicate should return same index

        // Verify sequential indexing
        assert_eq!(idx1, 1);
        assert_eq!(idx2, 2);
        assert_eq!(idx3, 1); // Same as first

        // Verify conversion
        assert_eq!(map.convert(42), 1);
        assert_eq!(map.convert(100), 2);
        assert_eq!(map.convert(0), 0); // NILINK stays 0
        assert_eq!(map.convert(999), 0); // Unregistered returns 0
    }

    #[test]
    fn test_pack_objectheader_basic() {
        let header = ObjectHeader {
            right: 10,
            left: 20,
            first_sub_obj: 30,
            xd: 100,
            yd: 200,
            obj_type: 2,
            selected: true,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ..Default::default()
        };

        let mut map = LinkMap::new();
        map.register(10);
        map.register(20);
        map.register(30);

        let mut buf = vec![0u8; 12];
        pack_objectheader_n105(&header, &map, &mut buf);

        // Verify header structure
        assert_eq!(buf.len(), 12);
        // Check LINK conversions (should be sequential indices)
        let right_idx = i16::from_be_bytes([buf[0], buf[1]]);
        let left_idx = i16::from_be_bytes([buf[2], buf[3]]);
        let first_sub_idx = i16::from_be_bytes([buf[4], buf[5]]);
        assert_eq!(right_idx, 1); // First registered
        assert_eq!(left_idx, 2); // Second registered
        assert_eq!(first_sub_idx, 3); // Third registered

        // Check coordinates
        let xd = i16::from_be_bytes([buf[6], buf[7]]);
        let yd = i16::from_be_bytes([buf[8], buf[9]]);
        assert_eq!(xd, 100);
        assert_eq!(yd, 200);

        // Check type
        assert_eq!(buf[10], 2);

        // Check bitfield (selected and visible should be set)
        assert_eq!(buf[11] & 0xC0, 0xC0);
    }

    #[test]
    fn test_pack_objectheader_nilink() {
        let header = ObjectHeader {
            right: 0, // NILINK
            left: 0,
            first_sub_obj: 0,
            xd: 50,
            yd: 75,
            obj_type: 1,
            selected: false,
            visible: false,
            soft: true,
            valid: false,
            tweaked: true,
            spare_flag: false,
            ..Default::default()
        };

        let map = LinkMap::new();
        let mut buf = vec![0u8; 12];
        pack_objectheader_n105(&header, &map, &mut buf);

        // NILINK values (0) should stay 0
        let right_idx = i16::from_be_bytes([buf[0], buf[1]]);
        let left_idx = i16::from_be_bytes([buf[2], buf[3]]);
        let first_sub_idx = i16::from_be_bytes([buf[4], buf[5]]);
        assert_eq!(right_idx, 0);
        assert_eq!(left_idx, 0);
        assert_eq!(first_sub_idx, 0);

        // Bitfield: soft (bit 5) and tweaked (bit 3) should be set
        assert_eq!(buf[11] & 0x28, 0x28);
    }
}
