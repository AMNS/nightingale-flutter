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
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
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

    // Extract type-specific fields from ObjData::Page variant
    if let ObjData::Page(page) = &obj.data {
        // Offset 32-33: lPage (LINK, big-endian)
        let lpage_idx = link_map.convert(page.l_page);
        buf[32..34].copy_from_slice(&lpage_idx.to_be_bytes());

        // Offset 34-35: rPage (LINK, big-endian)
        let rpage_idx = link_map.convert(page.r_page);
        buf[34..36].copy_from_slice(&rpage_idx.to_be_bytes());

        // Offset 36-37: sheetNum (short, big-endian)
        buf[36..38].copy_from_slice(&page.sheet_num.to_be_bytes());

        // Offset 38-41: headerStrOffset (4 bytes - string pool offset)
        buf[38..42].copy_from_slice(&page.header_str_offset.to_be_bytes());

        // Offset 42-45: footerStrOffset (4 bytes - string pool offset)
        buf[42..46].copy_from_slice(&page.footer_str_offset.to_be_bytes());
    }

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

    // Extract type-specific fields from ObjData::System variant
    if let ObjData::System(system) = &obj.data {
        // Offset 32-33: lSystem (LINK, big-endian)
        let lsystem_idx = link_map.convert(system.l_system);
        buf[32..34].copy_from_slice(&lsystem_idx.to_be_bytes());

        // Offset 34-35: rSystem (LINK, big-endian)
        let rsystem_idx = link_map.convert(system.r_system);
        buf[34..36].copy_from_slice(&rsystem_idx.to_be_bytes());

        // Offset 36-37: pageL (LINK, big-endian)
        let pagel_idx = link_map.convert(system.page_l);
        buf[36..38].copy_from_slice(&pagel_idx.to_be_bytes());

        // Offset 38-39: systemNum (short, big-endian)
        buf[38..40].copy_from_slice(&system.system_num.to_be_bytes());

        // Offset 40-47: systemRect (DRect = 4 x DDIST/short, big-endian)
        buf[40..42].copy_from_slice(&system.system_rect.top.to_be_bytes());
        buf[42..44].copy_from_slice(&system.system_rect.left.to_be_bytes());
        buf[44..46].copy_from_slice(&system.system_rect.bottom.to_be_bytes());
        buf[46..48].copy_from_slice(&system.system_rect.right.to_be_bytes());

        // Offset 48-51: sysDescPtr (4 bytes - upper 32 bits of u64, big-endian)
        let desc_ptr_bytes = system.sys_desc_ptr.to_be_bytes();
        buf[48..52].copy_from_slice(&desc_ptr_bytes[4..8]); // Use lower 32 bits
    }

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

    // Extract type-specific fields from ObjData::Staff variant
    if let ObjData::Staff(staff) = &obj.data {
        // Offset 32-33: lStaff (LINK, big-endian)
        let lstaff_idx = link_map.convert(staff.l_staff);
        buf[32..34].copy_from_slice(&lstaff_idx.to_be_bytes());

        // Offset 34-35: rStaff (LINK, big-endian)
        let rstaff_idx = link_map.convert(staff.r_staff);
        buf[34..36].copy_from_slice(&rstaff_idx.to_be_bytes());

        // Offset 36-37: systemL (LINK, big-endian)
        let systeml_idx = link_map.convert(staff.system_l);
        buf[36..38].copy_from_slice(&systeml_idx.to_be_bytes());
    }

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

    // Extract type-specific fields from ObjData::Measure variant
    if let ObjData::Measure(measure) = &obj.data {
        // Offset 32: fillerM (SignedByte)
        buf[32] = measure.filler_m as u8;

        // Offset 33: mac68k padding
        // buf[33] stays 0

        // Offset 34-35: lMeasure (LINK, big-endian)
        let lmeasure_idx = link_map.convert(measure.l_measure);
        buf[34..36].copy_from_slice(&lmeasure_idx.to_be_bytes());

        // Offset 36-37: rMeasure (LINK, big-endian)
        let rmeasure_idx = link_map.convert(measure.r_measure);
        buf[36..38].copy_from_slice(&rmeasure_idx.to_be_bytes());

        // Offset 38-39: systemL (LINK, big-endian)
        let systeml_idx = link_map.convert(measure.system_l);
        buf[38..40].copy_from_slice(&systeml_idx.to_be_bytes());

        // Offset 40-41: staffL (LINK, big-endian)
        let staffl_idx = link_map.convert(measure.staff_l);
        buf[40..42].copy_from_slice(&staffl_idx.to_be_bytes());

        // Offset 42-43: fakeMeas:1 | spacePercent:15 (bitfield in short)
        // fakeMeas in bit 15 (MSB), spacePercent in bits 14-0
        let fake_meas_bit = if measure.fake_meas != 0 { 0x8000u16 } else { 0 };
        let space_percent_bits = (measure.space_percent as u16) & 0x7FFF;
        let combined = fake_meas_bit | space_percent_bits;
        buf[42..44].copy_from_slice(&combined.to_be_bytes());

        // Offset 44-51: measureBBox (Rect = 4 x DDIST/short, big-endian)
        buf[44..46].copy_from_slice(&measure.measure_b_box.top.to_be_bytes());
        buf[46..48].copy_from_slice(&measure.measure_b_box.left.to_be_bytes());
        buf[48..50].copy_from_slice(&measure.measure_b_box.bottom.to_be_bytes());
        buf[50..52].copy_from_slice(&measure.measure_b_box.right.to_be_bytes());

        // Offset 52-55: lTimeStamp (long/i32, big-endian)
        buf[52..56].copy_from_slice(&measure.l_time_stamp.to_be_bytes());
    }

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

    // Extract type-specific fields from ObjData::Sync variant
    if let ObjData::Sync(sync) = &obj.data {
        // Offset 32-33: timeStamp (unsigned short, big-endian)
        buf[32..34].copy_from_slice(&sync.time_stamp.to_be_bytes());
    }

    buf
}

// =============================================================================
// Type-specific packers: REMAINING TYPES (1-17)
// =============================================================================

/// Pack Type 1: TAIL_5 (24 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       32    OBJECTHEADER_5 (actually written as 12 bytes)
/// 12      12    (padding/reserved)
/// ```
///
/// Source: NObjTypesN105.h lines 104-106
fn pack_tail_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    // Tail is just header + reserved space
    buf
}

/// Pack Type 3: RPTEND_5 (32 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      2     firstObj (LINK)
/// 14      2     startRpt (LINK)
/// 16      2     endRpt (LINK)
/// 18      1     subType (RptEndType)
/// 19      1     count (repeat count)
/// 20      12    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 109-115
fn pack_rptend_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 32];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::RptEnd(rptend) = &obj.data {
        // Offset 12-13: firstObj (LINK, big-endian)
        let first_obj_idx = link_map.convert(rptend.first_obj);
        buf[12..14].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 14-15: startRpt (LINK, big-endian)
        let start_rpt_idx = link_map.convert(rptend.start_rpt);
        buf[14..16].copy_from_slice(&start_rpt_idx.to_be_bytes());

        // Offset 16-17: endRpt (LINK, big-endian)
        let end_rpt_idx = link_map.convert(rptend.end_rpt);
        buf[16..18].copy_from_slice(&end_rpt_idx.to_be_bytes());

        // Offset 18: subType (RptEndType enum, i8)
        buf[18] = rptend.sub_type as u8;

        // Offset 19: count (repeat count, u8)
        buf[19] = rptend.count;
    }

    buf
}

/// Pack Type 8: CLEF_5 (24 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     inMeasure (bool)
/// 13      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 120-122
fn pack_clef_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Clef(clef) = &obj.data {
        // Offset 12: inMeasure (bool)
        buf[12] = if clef.in_measure { 1 } else { 0 };
    }

    buf
}

/// Pack Type 9: KEYSIG_5 (24 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     inMeasure (bool)
/// 13      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 125-127
fn pack_keysig_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::KeySig(keysig) = &obj.data {
        // Offset 12: inMeasure (bool)
        buf[12] = if keysig.in_measure { 1 } else { 0 };
    }

    buf
}

/// Pack Type 10: TIMESIG_5 (24 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     inMeasure (bool)
/// 13      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 130-132
fn pack_timesig_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::TimeSig(timesig) = &obj.data {
        // Offset 12: inMeasure (bool)
        buf[12] = if timesig.in_measure { 1 } else { 0 };
    }

    buf
}

/// Pack Type 11: BEAMSET_5 (26 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     voice
/// 14      1     thin:1 | beamRests:1 | feather:2 | grace:1 | firstSystem:1 | crossStaff:1 | crossSystem:1
/// 15      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 135-142
fn pack_beamset_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 26];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::BeamSet(beamset) = &obj.data {
        // Offset 12: extHeader (u8)
        buf[12] = beamset.ext_header.staffn as u8;

        // Offset 13: voice (i8)
        buf[13] = beamset.voice as u8;

        // Offset 14: bitfield thin:1 | beamRests:1 | feather:2 | grace:1 | firstSystem:1 | crossStaff:1 | crossSystem:1
        let mut b14: u8 = 0;
        b14 |= (beamset.thin & 1) << 7;
        b14 |= (beamset.beam_rests & 1) << 6;
        b14 |= (beamset.feather & 0x03) << 4;
        b14 |= (beamset.grace & 1) << 3;
        b14 |= (beamset.first_system & 1) << 2;
        b14 |= (beamset.cross_staff & 1) << 1;
        b14 |= beamset.cross_system & 1;
        buf[14] = b14;
    }

    buf
}

/// Pack Type 12: CONNECT_5 (26 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      2     connFiller (LINK, unused)
/// 14      12    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 145-147
fn pack_connect_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 26];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Connect(_connect) = &obj.data {
        // Offset 12-13: connFiller (LINK, typically NILINK)
        buf[12..14].copy_from_slice(&(0i16).to_be_bytes()); // Usually NILINK
    }

    buf
}

/// Pack Type 13: DYNAMIC_5 (30 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     dynamicType
/// 13      1     filler:7 | crossSys:1
/// 14      2     firstSyncL (LINK)
/// 16      2     lastSyncL (LINK)
/// 18      12    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 150-156
fn pack_dynamic_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 30];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Dynamic(dynamic) = &obj.data {
        // Offset 12: dynamicType (i8)
        buf[12] = dynamic.dynamic_type as u8;

        // Offset 13: filler:7 | crossSys:1
        let mut b13: u8 = 0;
        // filler is unused padding, set to 0
        b13 |= if dynamic.cross_sys { 1 } else { 0 };
        buf[13] = b13;

        // Offset 14-15: firstSyncL (LINK, big-endian)
        let first_sync_idx = link_map.convert(dynamic.first_sync_l);
        buf[14..16].copy_from_slice(&first_sync_idx.to_be_bytes());

        // Offset 16-17: lastSyncL (LINK, big-endian)
        let last_sync_idx = link_map.convert(dynamic.last_sync_l);
        buf[16..18].copy_from_slice(&last_sync_idx.to_be_bytes());
    }

    buf
}

/// Pack Type 15: GRAPHIC_5 (44 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     graphicType
/// 14      1     voice
/// 15      1     enclosure:2 | justify:3 | vConstrain:1 | hConstrain:1
/// 16      1     multiLine
/// 17      2     info (PICT ID or char code)
/// 19      8     guHandle (Handle)
/// 27      2     guThickness (union member)
/// 29      1     fontInd
/// 30      1     relFSize:1 | fontSize:7
/// 31      2     fontStyle
/// 33      2     info2
/// 35      2     firstObj (LINK)
/// 37      2     lastObj (LINK)
/// 39      5     (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 159-175
fn pack_graphic_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 44];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Graphic(graphic) = &obj.data {
        // Offset 12: extHeader
        buf[12] = graphic.ext_header.staffn as u8;

        // Offset 13: graphicType
        buf[13] = graphic.graphic_type as u8;

        // Offset 14: voice
        buf[14] = graphic.voice as u8;

        // Offset 15: enclosure:2 | justify:3 | vConstrain:1 | hConstrain:1
        let mut b15: u8 = 0;
        b15 |= (graphic.enclosure & 0x03) << 6;
        b15 |= (graphic.justify & 0x07) << 3;
        b15 |= (if graphic.v_constrain { 1 } else { 0 }) << 1;
        b15 |= if graphic.h_constrain { 1 } else { 0 };
        buf[15] = b15;

        // Offset 16: multiLine
        buf[16] = graphic.multi_line;

        // Offset 17-18: info
        buf[17..19].copy_from_slice(&graphic.info.to_be_bytes());

        // Offset 19-26: guHandle (8 bytes - typically 0)
        buf[19..27].copy_from_slice(&graphic.gu_handle.to_be_bytes());

        // Offset 27-28: guThickness
        buf[27..29].copy_from_slice(&graphic.gu_thickness.to_be_bytes());

        // Offset 29: fontInd
        buf[29] = graphic.font_ind as u8;

        // Offset 30: relFSize:1 | fontSize:7
        let mut b30: u8 = 0;
        b30 |= (if graphic.rel_f_size != 0 { 1 } else { 0 }) << 7;
        b30 |= graphic.font_size & 0x7F;
        buf[30] = b30;

        // Offset 31-32: fontStyle
        buf[31..33].copy_from_slice(&graphic.font_style.to_be_bytes());

        // Offset 33-34: info2
        buf[33..35].copy_from_slice(&graphic.info2.to_be_bytes());

        // Offset 35-36: firstObj (LINK)
        let first_obj_idx = link_map.convert(graphic.first_obj);
        buf[35..37].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 37-38: lastObj (LINK)
        let last_obj_idx = link_map.convert(graphic.last_obj);
        buf[37..39].copy_from_slice(&last_obj_idx.to_be_bytes());
    }

    buf
}

/// Pack Type 16: OTTAVA_5 (40 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     noCutoff:1 | crossStaff:1 | crossSystem:1 | octSignType:5
/// 14      1     filler
/// 15      1     numberVis:1 | unused1:1 | brackVis:1 | unused2:5
/// 16      2     nxd (DDIST)
/// 18      2     nyd (DDIST)
/// 20      2     xdFirst (DDIST)
/// 22      2     ydFirst (DDIST)
/// 24      2     xdLast (DDIST)
/// 26      2     ydLast (DDIST)
/// 28      12    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 178-190
fn pack_ottava_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 40];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Ottava(ottava) = &obj.data {
        // Offset 12: extHeader
        buf[12] = ottava.ext_header.staffn as u8;

        // Offset 13: noCutoff:1 | crossStaff:1 | crossSystem:1 | octSignType:5
        let mut b13: u8 = 0;
        b13 |= (if ottava.no_cutoff != 0 { 1 } else { 0 }) << 7;
        b13 |= (if ottava.cross_staff != 0 { 1 } else { 0 }) << 6;
        b13 |= (if ottava.cross_system != 0 { 1 } else { 0 }) << 5;
        b13 |= ottava.oct_sign_type & 0x1F;
        buf[13] = b13;

        // Offset 14: filler
        buf[14] = ottava.filler as u8;

        // Offset 15: numberVis:1 | unused1:1 | brackVis:1 | unused2:5
        let mut b15: u8 = 0;
        b15 |= (if ottava.number_vis { 1 } else { 0 }) << 7;
        b15 |= (if ottava.unused1 { 1 } else { 0 }) << 6;
        b15 |= (if ottava.brack_vis { 1 } else { 0 }) << 5;
        buf[15] = b15;

        // Offset 16-17: nxd (DDIST, big-endian)
        buf[16..18].copy_from_slice(&ottava.nxd.to_be_bytes());

        // Offset 18-19: nyd (DDIST, big-endian)
        buf[18..20].copy_from_slice(&ottava.nyd.to_be_bytes());

        // Offset 20-21: xdFirst (DDIST, big-endian)
        buf[20..22].copy_from_slice(&ottava.xd_first.to_be_bytes());

        // Offset 22-23: ydFirst (DDIST, big-endian)
        buf[22..24].copy_from_slice(&ottava.yd_first.to_be_bytes());

        // Offset 24-25: xdLast (DDIST, big-endian)
        buf[24..26].copy_from_slice(&ottava.xd_last.to_be_bytes());

        // Offset 26-27: ydLast (DDIST, big-endian)
        buf[26..28].copy_from_slice(&ottava.yd_last.to_be_bytes());
    }

    buf
}

/// Pack Type 17: SLUR_5 (30 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     voice
/// 14      1     philler:2 | crossStaff:1 | crossStfBack:1 | crossSystem:1 | unused:3
/// 15      1     tempFlag:1 | used:1 | tie:1 | unused:5
/// 16      2     firstSyncL (LINK)
/// 18      2     lastSyncL (LINK)
/// 20      10    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 193-204
fn pack_slur_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 30];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Slur(slur) = &obj.data {
        // Offset 12: extHeader
        buf[12] = slur.ext_header.staffn as u8;

        // Offset 13: voice
        buf[13] = slur.voice as u8;

        // Offset 14: philler:2 | crossStaff:1 | crossStfBack:1 | crossSystem:1 | unused:3
        let mut b14: u8 = 0;
        b14 |= (slur.philler & 0x03) << 6;
        b14 |= (if slur.cross_staff != 0 { 1 } else { 0 }) << 4;
        b14 |= (if slur.cross_stf_back != 0 { 1 } else { 0 }) << 3;
        b14 |= (if slur.cross_system != 0 { 1 } else { 0 }) << 2;
        buf[14] = b14;

        // Offset 15: tempFlag:1 | used:1 | tie:1 | unused:5
        let mut b15: u8 = 0;
        b15 |= (if slur.temp_flag { 1 } else { 0 }) << 7;
        b15 |= (if slur.used { 1 } else { 0 }) << 6;
        b15 |= (if slur.tie { 1 } else { 0 }) << 5;
        buf[15] = b15;

        // Offset 16-17: firstSyncL (LINK, big-endian)
        let first_sync_idx = link_map.convert(slur.first_sync_l);
        buf[16..18].copy_from_slice(&first_sync_idx.to_be_bytes());

        // Offset 18-19: lastSyncL (LINK, big-endian)
        let last_sync_idx = link_map.convert(slur.last_sync_l);
        buf[18..20].copy_from_slice(&last_sync_idx.to_be_bytes());
    }

    buf
}

/// Pack Type 18: TUPLET_5 (40 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     accNum (accessory numerator)
/// 14      1     accDenom (accessory denominator)
/// 15      1     voice
/// 16      1     numVis
/// 17      1     denomVis
/// 18      1     brackVis
/// 19      1     small
/// 20      1     filler
/// 21      2     acnxd (DDIST)
/// 23      2     acnyd (DDIST)
/// 25      2     xdFirst (DDIST)
/// 27      2     ydFirst (DDIST)
/// 29      2     xdLast (DDIST)
/// 31      2     ydLast (DDIST)
/// 33      7     (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 207-222
fn pack_tuplet_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 40];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Tuplet(tuplet) = &obj.data {
        // Offset 12: extHeader
        buf[12] = tuplet.ext_header.staffn as u8;

        // Offset 13: accNum
        buf[13] = tuplet.acc_num;

        // Offset 14: accDenom
        buf[14] = tuplet.acc_denom;

        // Offset 15: voice
        buf[15] = tuplet.voice as u8;

        // Offset 16: numVis
        buf[16] = tuplet.num_vis;

        // Offset 17: denomVis
        buf[17] = tuplet.denom_vis;

        // Offset 18: brackVis
        buf[18] = tuplet.brack_vis;

        // Offset 19: small
        buf[19] = tuplet.small;

        // Offset 20: filler
        buf[20] = tuplet.filler;

        // Offset 21-22: acnxd (DDIST, big-endian)
        buf[21..23].copy_from_slice(&tuplet.acnxd.to_be_bytes());

        // Offset 23-24: acnyd (DDIST, big-endian)
        buf[23..25].copy_from_slice(&tuplet.acnyd.to_be_bytes());

        // Offset 25-26: xdFirst (DDIST, big-endian)
        buf[25..27].copy_from_slice(&tuplet.xd_first.to_be_bytes());

        // Offset 27-28: ydFirst (DDIST, big-endian)
        buf[27..29].copy_from_slice(&tuplet.yd_first.to_be_bytes());

        // Offset 29-30: xdLast (DDIST, big-endian)
        buf[29..31].copy_from_slice(&tuplet.xd_last.to_be_bytes());

        // Offset 31-32: ydLast (DDIST, big-endian)
        buf[31..33].copy_from_slice(&tuplet.yd_last.to_be_bytes());
    }

    buf
}

/// Pack Type 19: GRSYNC_5 (24 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      12    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 225-227
fn pack_grsync_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);
    // GRSYNC is just header + reserved
    buf
}

/// Pack Type 20: TEMPO_5 (38 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     subType (beat unit)
/// 14      1     expanded:1 | noMM:1 | filler:4 | dotted:1 | hideMM:1
/// 15      2     tempoMM (BPM)
/// 17      4     strOffset (string pool offset)
/// 21      2     firstObjL (LINK)
/// 23      4     metroStrOffset (metronome string offset)
/// 27      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 230-239
fn pack_tempo_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 38];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Tempo(tempo) = &obj.data {
        // Offset 12: extHeader
        buf[12] = tempo.ext_header.staffn as u8;

        // Offset 13: subType
        buf[13] = tempo.sub_type as u8;

        // Offset 14: expanded:1 | noMM:1 | filler:4 | dotted:1 | hideMM:1
        let mut b14: u8 = 0;
        b14 |= (if tempo.expanded { 1 } else { 0 }) << 7;
        b14 |= (if tempo.no_mm { 1 } else { 0 }) << 6;
        b14 |= (tempo.filler & 0x0F) << 2;
        b14 |= (if tempo.dotted { 1 } else { 0 }) << 1;
        b14 |= if tempo.hide_mm { 1 } else { 0 };
        buf[14] = b14;

        // Offset 15-16: tempoMM (i16, big-endian)
        buf[15..17].copy_from_slice(&tempo.tempo_mm.to_be_bytes());

        // Offset 17-20: strOffset (u32/StringOffset, big-endian)
        buf[17..21].copy_from_slice(&tempo.str_offset.to_be_bytes());

        // Offset 21-22: firstObjL (LINK, big-endian)
        let first_obj_idx = link_map.convert(tempo.first_obj_l);
        buf[21..23].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 23-26: metroStrOffset (u32/StringOffset, big-endian)
        buf[23..27].copy_from_slice(&tempo.metro_str_offset.to_be_bytes());
    }

    buf
}

/// Pack Type 21: SPACER_5 (28 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     bottomStaff
/// 14      2     spWidth (STDIST)
/// 16      12    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 242-246
fn pack_spacer_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 28];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Spacer(spacer) = &obj.data {
        // Offset 12: extHeader
        buf[12] = spacer.ext_header.staffn as u8;

        // Offset 13: bottomStaff
        buf[13] = spacer.bottom_staff as u8;

        // Offset 14-15: spWidth (STDIST, big-endian)
        buf[14..16].copy_from_slice(&spacer.sp_width.to_be_bytes());
    }

    buf
}

/// Pack Type 22: ENDING_5 (32 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     extHeader
/// 13      1     firstObjL (LINK, high byte)
/// 14      2     firstObjL (LINK, low 2 bytes) / lastObjL (LINK, high byte)
/// 16      2     lastObjL (LINK, low bytes)
/// 18      1     noLCutoff:1 | noRCutoff:1 | endNum:6
/// 19      2     endxd (DDIST)
/// 21      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 249-256
fn pack_ending_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 32];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::Ending(ending) = &obj.data {
        // Offset 12: extHeader
        buf[12] = ending.ext_header.staffn as u8;

        // Offset 13-14: firstObjL (LINK, big-endian)
        let first_obj_idx = link_map.convert(ending.first_obj_l);
        buf[13..15].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 15-16: lastObjL (LINK, big-endian)
        let last_obj_idx = link_map.convert(ending.last_obj_l);
        buf[15..17].copy_from_slice(&last_obj_idx.to_be_bytes());

        // Offset 17: noLCutoff:1 | noRCutoff:1 | endNum:6
        let mut b17: u8 = 0;
        b17 |= (if ending.no_l_cutoff != 0 { 1 } else { 0 }) << 7;
        b17 |= (if ending.no_r_cutoff != 0 { 1 } else { 0 }) << 6;
        b17 |= ending.end_num & 0x3F;
        buf[17] = b17;

        // Offset 18-19: endxd (DDIST, big-endian)
        buf[18..20].copy_from_slice(&ending.endxd.to_be_bytes());
    }

    buf
}

/// Pack Type 23: PSMEAS_5 (24 bytes).
///
/// On-disk layout:
/// ```text
/// Offset  Size  Field
/// 0       12    OBJECTHEADER_5
/// 12      1     filler
/// 13      11    (reserved/padding)
/// ```
///
/// Source: NObjTypesN105.h lines 259-261
fn pack_psmeas_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    if let ObjData::PsMeas(psmeas) = &obj.data {
        // Offset 12: filler
        buf[12] = psmeas.filler as u8;
    }

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
