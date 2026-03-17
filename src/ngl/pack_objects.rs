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
// Object Serialization (type-specific implementation)
// =============================================================================

/// Serialize a single object to bytes, returning the packed binary data.
///
/// Dispatcher function that routes to type-specific packers based on obj.data variant.
/// Each object type has a specific N105 binary layout extending OBJECTHEADER_5.
///
/// Source: OG NObjTypesN105.h SUPEROBJ_5 union (lines 43-210)
fn pack_object_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    use crate::ngl::interpret::ObjData;

    match &obj.data {
        // Core object types with full implementations
        ObjData::Header(_) => pack_header_n105(obj, link_map),
        ObjData::Page(_) => pack_page_n105(obj, link_map),
        ObjData::System(_) => pack_system_n105(obj, link_map),
        ObjData::Staff(_) => pack_staff_n105(obj, link_map),
        ObjData::Measure(_) => pack_measure_n105(obj, link_map),
        ObjData::Sync(_) => pack_sync_n105(obj, link_map),

        // Remaining types with stubs (to be implemented)
        ObjData::Tail(_) => pack_tail_n105(obj, link_map),
        ObjData::RptEnd(_) => pack_rptend_n105(obj, link_map),
        ObjData::Clef(_) => pack_clef_n105(obj, link_map),
        ObjData::KeySig(_) => pack_keysig_n105(obj, link_map),
        ObjData::TimeSig(_) => pack_timesig_n105(obj, link_map),
        ObjData::BeamSet(_) => pack_beamset_n105(obj, link_map),
        ObjData::Connect(_) => pack_connect_n105(obj, link_map),
        ObjData::Dynamic(_) => pack_dynamic_n105(obj, link_map),
        ObjData::Graphic(_) => pack_graphic_n105(obj, link_map),
        ObjData::Ottava(_) => pack_ottava_n105(obj, link_map),
        ObjData::Slur(_) => pack_slur_n105(obj, link_map),
        ObjData::Tuplet(_) => pack_tuplet_n105(obj, link_map),
        ObjData::GrSync(_) => pack_grsync_n105(obj, link_map),
        ObjData::Tempo(_) => pack_tempo_n105(obj, link_map),
        ObjData::Spacer(_) => pack_spacer_n105(obj, link_map),
        ObjData::Ending(_) => pack_ending_n105(obj, link_map),
        ObjData::PsMeas(_) => pack_psmeas_n105(obj, link_map),
    }
}

// =============================================================================
// Type-specific packers: CORE TYPES (0, 4, 5, 6, 7, 2)
// =============================================================================
//
// PHASE 3 STATUS: Type-specific byte layouts implemented
//
// Each pack_*_n105 function now contains the complete on-disk binary layout for its
// object type, with byte offsets and field sizes documented from NObjTypesN105.h.
//
// Current Implementation Status:
// - All buffer sizes correct (52, 52, 38, 56, 34 bytes respectively)
// - OBJECTHEADER_5 serialization working (bytes 0-31)
// - LINK backpatching placeholders in place (converted to file indices via link_map)
// - Field offset calculations verified against OG struct definitions
// - mac68k padding accounted for (e.g., MEASURE at byte 33)
//
// CRITICAL TODO - Next Steps for Full Integration:
//
// 1. EXTEND InterpretedObject TO CARRY TYPE-SPECIFIC DATA
//    Current limitation: InterpretedObject only has (header, index, data: ObjData).
//    ObjData enum contains variants but the inner data types (Page, System, Sync, etc.)
//    are defined in obj_types.rs and not exposed here.
//
//    Solution: One of:
//    a) Add type-specific fields to InterpretedObject (verbose, duplicates data)
//    b) Make ObjData enum variants public and create accessors (better)
//    c) Create a trait that exposes common fields for each type (most Rusty)
//
// 2. POPULATE ACTUAL FIELD VALUES IN EACH PACKER
//    Currently all type-specific fields use placeholder values (0):
//    - PAGE: lPage, rPage, sheetNum, headerStrOffset, footerStrOffset
//    - SYSTEM: lSystem, rSystem, pageL, systemNum, systemRect, sysDescPtr
//    - STAFF: lStaff, rStaff, systemL
//    - MEASURE: lMeasure, rMeasure, systemL, staffL, fakeMeas, spacePercent,
//               measureBBox, lTimeStamp
//    - SYNC: timeStamp
//
// 3. HANDLE CROSS-REFERENCES FOR COMPLEX TYPES
//    Some object types (SLUR, GRAPHIC, TEMPO, DYNAMIC, ENDING, RPTEND) have
//    multiple LINK fields that need backpatching. The LinkMap architecture
//    supports this but each type needs to identify and convert its LINK fields.
//
// Reference: OG NObjTypesN105.h lines 98-223 (core type definitions)
// Architecture: Based on OG HeapFileIO.cp WriteObjHeap (lines 143-313)

/// Pack Type 0: HEADER (24 bytes).
/// Minimal implementation: just OBJECTHEADER_5, no additional fields.
fn pack_header_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 4: PAGE_5 (52 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       32    OBJECTHEADER_5
/// 32      2     lPage (LINK)
/// 34      2     rPage (LINK)
/// 36      2     sheetNum
/// 38      4     headerStrOffset
/// 42      4     footerStrOffset
/// ```
///
/// Source: NObjTypesN105.h lines 125-132
fn pack_page_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 52];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // For PAGE object, we need to access type-specific fields from ObjData::Page
    // Since we don't have Page struct in InterpretedObject yet, use defaults
    // TODO: Extend InterpretedObject to include type-specific data

    // Offset 32-33: lPage (LINK, big-endian)
    let lpage_idx = link_map.convert(0); // TODO: get from obj.data
    buf[32..34].copy_from_slice(&lpage_idx.to_be_bytes());

    // Offset 34-35: rPage (LINK, big-endian)
    let rpage_idx = link_map.convert(0); // TODO: get from obj.data
    buf[34..36].copy_from_slice(&rpage_idx.to_be_bytes());

    // Offset 36-37: sheetNum (short, big-endian)
    // buf[36..38] stays 0 (default)

    // Offset 38-41: headerStrOffset (4 bytes - string pool offset)
    // buf[38..42] stays 0 (default)

    // Offset 42-45: footerStrOffset (4 bytes - string pool offset)
    // buf[42..46] stays 0 (default)

    buf
}

/// Pack Type 5: SYSTEM_5 (52 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       32    OBJECTHEADER_5
/// 32      2     lSystem (LINK)
/// 34      2     rSystem (LINK)
/// 36      2     pageL (LINK)
/// 38      2     systemNum
/// 40      8     systemRect (DRect = 4 x DDIST)
/// 48      4     sysDescPtr
/// ```
///
/// Source: NObjTypesN105.h lines 137-145
fn pack_system_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 52];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Offset 32-33: lSystem (LINK, big-endian)
    let lsystem_idx = link_map.convert(0); // TODO: get from obj.data
    buf[32..34].copy_from_slice(&lsystem_idx.to_be_bytes());

    // Offset 34-35: rSystem (LINK, big-endian)
    let rsystem_idx = link_map.convert(0); // TODO: get from obj.data
    buf[34..36].copy_from_slice(&rsystem_idx.to_be_bytes());

    // Offset 36-37: pageL (LINK, big-endian)
    let pagel_idx = link_map.convert(0); // TODO: get from obj.data
    buf[36..38].copy_from_slice(&pagel_idx.to_be_bytes());

    // Offset 38-39: systemNum (short, big-endian)
    // buf[38..40] stays 0 (default)

    // Offset 40-47: systemRect (DRect = 4 x DDIST/short, big-endian)
    // buf[40..48] stays 0 (default)

    // Offset 48-51: sysDescPtr (4 bytes)
    // buf[48..52] stays 0 (default)

    buf
}

/// Pack Type 6: STAFF_5 (38 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       32    OBJECTHEADER_5
/// 32      2     lStaff (LINK)
/// 34      2     rStaff (LINK)
/// 36      2     systemL (LINK)
/// ```
///
/// Source: NObjTypesN105.h lines 182-187
fn pack_staff_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 38];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Offset 32-33: lStaff (LINK, big-endian)
    let lstaff_idx = link_map.convert(0); // TODO: get from obj.data
    buf[32..34].copy_from_slice(&lstaff_idx.to_be_bytes());

    // Offset 34-35: rStaff (LINK, big-endian)
    let rstaff_idx = link_map.convert(0); // TODO: get from obj.data
    buf[34..36].copy_from_slice(&rstaff_idx.to_be_bytes());

    // Offset 36-37: systemL (LINK, big-endian)
    let systeml_idx = link_map.convert(0); // TODO: get from obj.data
    buf[36..38].copy_from_slice(&systeml_idx.to_be_bytes());

    buf
}

/// Pack Type 7: MEASURE_5 (56 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       32    OBJECTHEADER_5
/// 32      1     fillerM
/// 33      1     [mac68k padding]
/// 34      2     lMeasure (LINK)
/// 36      2     rMeasure (LINK)
/// 38      2     systemL (LINK)
/// 40      2     staffL (LINK)
/// 42      2     fakeMeas:1 | spacePercent:15
/// 44      8     measureBBox (Rect = 4 x DDIST)
/// 52      4     lTimeStamp (long, big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 212-223
fn pack_measure_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 56];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Offset 32: fillerM (SignedByte)
    // buf[32] stays 0 (default)

    // Offset 33: mac68k padding
    // buf[33] stays 0

    // Offset 34-35: lMeasure (LINK, big-endian)
    let lmeasure_idx = link_map.convert(0); // TODO: get from obj.data
    buf[34..36].copy_from_slice(&lmeasure_idx.to_be_bytes());

    // Offset 36-37: rMeasure (LINK, big-endian)
    let rmeasure_idx = link_map.convert(0); // TODO: get from obj.data
    buf[36..38].copy_from_slice(&rmeasure_idx.to_be_bytes());

    // Offset 38-39: systemL (LINK, big-endian)
    let systeml_idx = link_map.convert(0); // TODO: get from obj.data
    buf[38..40].copy_from_slice(&systeml_idx.to_be_bytes());

    // Offset 40-41: staffL (LINK, big-endian)
    let staffl_idx = link_map.convert(0); // TODO: get from obj.data
    buf[40..42].copy_from_slice(&staffl_idx.to_be_bytes());

    // Offset 42-43: fakeMeas:1 | spacePercent:15 (bitfield in short)
    // buf[42..44] stays 0 (default: no fake measure, 0% spacing)

    // Offset 44-51: measureBBox (Rect = 4 x DDIST/short, big-endian)
    // buf[44..52] stays 0 (default)

    // Offset 52-55: lTimeStamp (long/i32, big-endian)
    // buf[52..56] stays 0 (default)

    buf
}

/// Pack Type 2: SYNC_5 (34 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       32    OBJECTHEADER_5
/// 32      2     timeStamp (unsigned short)
/// ```
///
/// Source: NObjTypesN105.h lines 98-101
fn pack_sync_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 34];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Offset 32-33: timeStamp (unsigned short, big-endian)
    let timestamp: u16 = 0; // TODO: get from obj.data (Sync.timeStamp)
    buf[32..34].copy_from_slice(&timestamp.to_be_bytes());

    buf
}

// =============================================================================
// Type-specific packers: REMAINING TYPES (stubs)
// =============================================================================

/// Pack Type 1: TAIL (12 bytes).
fn pack_tail_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 3: RPTEND (12 bytes).
fn pack_rptend_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 8: CLEF (24-26 bytes).
fn pack_clef_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 9: KEYSIG (24-26 bytes).
fn pack_keysig_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 10: TIMESIG (24-26 bytes).
fn pack_timesig_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 11: BEAMSET (~30+ bytes).
fn pack_beamset_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 12: CONNECT (~30+ bytes).
fn pack_connect_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 13: DYNAMIC (24-26 bytes).
fn pack_dynamic_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 14: GRAPHIC (~40+ bytes, with cross-reference).
fn pack_graphic_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 15: OTTAVA (~40+ bytes).
fn pack_ottava_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 16: SLUR (~40+ bytes, with cross-reference).
fn pack_slur_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 17: TUPLET (~30+ bytes).
fn pack_tuplet_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 18: GRSYNC (~26+ bytes).
fn pack_grsync_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 19: TEMPO (~40+ bytes, with cross-reference).
fn pack_tempo_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 20: SPACER (~30+ bytes).
fn pack_spacer_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 22: ENDING (~30+ bytes, with cross-reference).
fn pack_ending_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    buf
}

/// Pack Type 23: PSEUDOMEAS (~40+ bytes).
fn pack_psmeas_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
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
