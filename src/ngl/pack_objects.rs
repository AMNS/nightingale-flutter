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
pub struct LinkMap {
    /// Map from in-memory Link to file index
    map: HashMap<Link, Link>,
    /// Next available file index (incremented as objects are added)
    next_index: Link,
}

impl Default for LinkMap {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkMap {
    /// Create a new LinkMap, starting with index 1 (0 reserved for NILINK).
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            next_index: 1,
        }
    }

    /// Register a Link value and return its file index.
    pub fn register(&mut self, in_memory_link: Link) -> Link {
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

    // Offset 12-19: objRect (top, left, bottom, right -- each i16 big-endian)
    // Confirmed: OG OBJECTHEADER_5 macro: Rect objRect at (.+#12)
    buf[12..14].copy_from_slice(&header.obj_rect.top.to_be_bytes());
    buf[14..16].copy_from_slice(&header.obj_rect.left.to_be_bytes());
    buf[16..18].copy_from_slice(&header.obj_rect.bottom.to_be_bytes());
    buf[18..20].copy_from_slice(&header.obj_rect.right.to_be_bytes());

    // Offset 20: relSize (SignedByte)
    buf[20] = header.rel_size as u8;

    // Offset 21: ohdrFiller2 (SignedByte)
    buf[21] = header.ohdr_filler2 as u8;

    // Offset 22: nEntries (Byte) -- confirmed: OG comment (.+#22)
    buf[22] = header.n_entries;
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
pub fn pack_object_n105(obj: &InterpretedObject, link_map: &LinkMap) -> Vec<u8> {
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
    let mut buf = vec![0u8; 24];
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
    let mut buf = vec![0u8; 38];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Extract type-specific fields from ObjData::Page variant
    if let ObjData::Page(page) = &obj.data {
        // Offset 24-25: lPage (LINK, big-endian)
        let lpage_idx = link_map.convert(page.l_page);
        buf[24..26].copy_from_slice(&lpage_idx.to_be_bytes());

        // Offset 26-27: rPage (LINK, big-endian)
        let rpage_idx = link_map.convert(page.r_page);
        buf[26..28].copy_from_slice(&rpage_idx.to_be_bytes());

        // Offset 28-29: sheetNum (short, big-endian)
        buf[28..30].copy_from_slice(&page.sheet_num.to_be_bytes());

        // Offset 30-33: headerStrOffset (4 bytes - string pool offset)
        buf[30..34].copy_from_slice(&page.header_str_offset.to_be_bytes());

        // Offset 34-37: footerStrOffset (4 bytes - string pool offset)
        buf[34..38].copy_from_slice(&page.footer_str_offset.to_be_bytes());
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
    let mut buf = vec![0u8; 44];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Extract type-specific fields from ObjData::System variant
    if let ObjData::System(system) = &obj.data {
        // Offset 24-25: lSystem (LINK, big-endian)
        let lsystem_idx = link_map.convert(system.l_system);
        buf[24..26].copy_from_slice(&lsystem_idx.to_be_bytes());

        // Offset 26-27: rSystem (LINK, big-endian)
        let rsystem_idx = link_map.convert(system.r_system);
        buf[26..28].copy_from_slice(&rsystem_idx.to_be_bytes());

        // Offset 28-29: pageL (LINK, big-endian)
        let pagel_idx = link_map.convert(system.page_l);
        buf[28..30].copy_from_slice(&pagel_idx.to_be_bytes());

        // Offset 30-31: systemNum (short, big-endian)
        buf[30..32].copy_from_slice(&system.system_num.to_be_bytes());

        // Offset 32-39: systemRect (DRect = 4 x DDIST/short, big-endian)
        buf[32..34].copy_from_slice(&system.system_rect.top.to_be_bytes());
        buf[34..36].copy_from_slice(&system.system_rect.left.to_be_bytes());
        buf[36..38].copy_from_slice(&system.system_rect.bottom.to_be_bytes());
        buf[38..40].copy_from_slice(&system.system_rect.right.to_be_bytes());

        // Offset 40-43: sysDescPtr (4 bytes, big-endian)
        let desc_ptr_bytes = system.sys_desc_ptr.to_be_bytes();
        buf[40..44].copy_from_slice(&desc_ptr_bytes[4..8]); // Use lower 32 bits
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
    let mut buf = vec![0u8; 30];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Extract type-specific fields from ObjData::Staff variant
    if let ObjData::Staff(staff) = &obj.data {
        // Offset 24-25: lStaff (LINK, big-endian)
        let lstaff_idx = link_map.convert(staff.l_staff);
        buf[24..26].copy_from_slice(&lstaff_idx.to_be_bytes());

        // Offset 26-27: rStaff (LINK, big-endian)
        let rstaff_idx = link_map.convert(staff.r_staff);
        buf[26..28].copy_from_slice(&rstaff_idx.to_be_bytes());

        // Offset 28-29: systemL (LINK, big-endian)
        let systeml_idx = link_map.convert(staff.system_l);
        buf[28..30].copy_from_slice(&systeml_idx.to_be_bytes());
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
    let mut buf = vec![0u8; 46];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Extract type-specific fields from ObjData::Measure variant
    if let ObjData::Measure(measure) = &obj.data {
        // Offset 23: fillerM (SignedByte)
        buf[23] = measure.filler_m as u8;

        // Offset 24-25: lMeasure (LINK, big-endian)
        let lmeasure_idx = link_map.convert(measure.l_measure);
        buf[24..26].copy_from_slice(&lmeasure_idx.to_be_bytes());

        // Offset 26-27: rMeasure (LINK, big-endian)
        let rmeasure_idx = link_map.convert(measure.r_measure);
        buf[26..28].copy_from_slice(&rmeasure_idx.to_be_bytes());

        // Offset 28-29: systemL (LINK, big-endian)
        let systeml_idx = link_map.convert(measure.system_l);
        buf[28..30].copy_from_slice(&systeml_idx.to_be_bytes());

        // Offset 30-31: staffL (LINK, big-endian)
        let staffl_idx = link_map.convert(measure.staff_l);
        buf[30..32].copy_from_slice(&staffl_idx.to_be_bytes());

        // Offset 32-33: fakeMeas:1 | spacePercent:15 (bitfield in short)
        // fakeMeas in bit 15 (MSB), spacePercent in bits 14-0
        let fake_meas_bit = if measure.fake_meas != 0 { 0x8000u16 } else { 0 };
        let space_percent_bits = (measure.space_percent as u16) & 0x7FFF;
        let combined = fake_meas_bit | space_percent_bits;
        buf[32..34].copy_from_slice(&combined.to_be_bytes());

        // Offset 34-41: measureBBox (Rect = 4 x DDIST/short, big-endian)
        buf[34..36].copy_from_slice(&measure.measure_b_box.top.to_be_bytes());
        buf[36..38].copy_from_slice(&measure.measure_b_box.left.to_be_bytes());
        buf[38..40].copy_from_slice(&measure.measure_b_box.bottom.to_be_bytes());
        buf[40..42].copy_from_slice(&measure.measure_b_box.right.to_be_bytes());

        // Offset 42-45: lTimeStamp (long/i32, big-endian)
        buf[42..46].copy_from_slice(&measure.l_time_stamp.to_be_bytes());
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
    let mut buf = vec![0u8; 26];

    // Offset 0-31: OBJECTHEADER_5
    pack_objectheader_n105(&obj.header, link_map, &mut buf);

    // Extract type-specific fields from ObjData::Sync variant
    if let ObjData::Sync(sync) = &obj.data {
        // Offset 24-25: timeStamp (unsigned short, big-endian)
        // Note: 1 byte mac68k padding at offset 23 between header and timeStamp
        buf[24..26].copy_from_slice(&sync.time_stamp.to_be_bytes());
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
        // RPTEND_5: LINKs align to even after odd OBJ(23)
        // [24..26] firstObj, [26..28] startRpt, [28..30] endRpt, [30] subType, [31] count
        let first_obj_idx = link_map.convert(rptend.first_obj);
        buf[24..26].copy_from_slice(&first_obj_idx.to_be_bytes());

        let start_rpt_idx = link_map.convert(rptend.start_rpt);
        buf[26..28].copy_from_slice(&start_rpt_idx.to_be_bytes());

        let end_rpt_idx = link_map.convert(rptend.end_rpt);
        buf[28..30].copy_from_slice(&end_rpt_idx.to_be_bytes());

        buf[30] = rptend.sub_type as u8;
        buf[31] = rptend.count;
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
        // Offset 23: inMeasure (bool)
        buf[23] = if clef.in_measure { 1 } else { 0 };
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
        // Offset 23: inMeasure (bool)
        buf[23] = if keysig.in_measure { 1 } else { 0 };
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
        // Offset 23: inMeasure (bool)
        buf[23] = if timesig.in_measure { 1 } else { 0 };
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
        // Offset 23: extHeader (u8)
        buf[23] = beamset.ext_header.staffn as u8;

        // Offset 24: voice (i8)
        buf[24] = beamset.voice as u8;

        // Offset 25: bitfield thin:1 | beamRests:1 | feather:2 | grace:1 | firstSystem:1 | crossStaff:1 | crossSystem:1
        let mut b25: u8 = 0;
        b25 |= (beamset.thin & 1) << 7;
        b25 |= (beamset.beam_rests & 1) << 6;
        b25 |= (beamset.feather & 0x03) << 4;
        b25 |= (beamset.grace & 1) << 3;
        b25 |= (beamset.first_system & 1) << 2;
        b25 |= (beamset.cross_staff & 1) << 1;
        b25 |= beamset.cross_system & 1;
        buf[25] = b25;
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
        // CONNECT_5: LINK connFiller aligns to even after OBJ(23) → offset 24
        buf[24..26].copy_from_slice(&(0i16).to_be_bytes());
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
        // Offset 23: dynamicType (i8)
        buf[23] = dynamic.dynamic_type as u8;

        // Offset 24: filler:7 | crossSys:1
        let mut b24: u8 = 0;
        b24 |= if dynamic.cross_sys { 1 } else { 0 };
        buf[24] = b24;

        // DYNAMIC_5: LINKs align to even. [26..28] firstSyncL, [28..30] lastSyncL
        let first_sync_idx = link_map.convert(dynamic.first_sync_l);
        buf[26..28].copy_from_slice(&first_sync_idx.to_be_bytes());

        let last_sync_idx = link_map.convert(dynamic.last_sync_l);
        buf[28..30].copy_from_slice(&last_sync_idx.to_be_bytes());
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
        // Offset 23: staffn (ExtObjHeader)
        buf[23] = graphic.ext_header.staffn as u8;

        // Offset 24: graphicType
        buf[24] = graphic.graphic_type as u8;

        // Offset 25: voice
        buf[25] = graphic.voice as u8;

        // Offset 26: enclosure:2|justify:3|vConstrain:1|hConstrain:1|multiLine:1
        // Reader (interpret.rs) packs enclosure/justify/vConstrain/hConstrain into b26,
        // and uses b26&1 for multiLine — all 5 fields share one byte.
        let mut b26: u8 = 0;
        b26 |= (graphic.enclosure & 0x03) << 6;
        b26 |= (graphic.justify & 0x07) << 3;
        b26 |= (if graphic.v_constrain { 1 } else { 0 }) << 2;
        b26 |= (if graphic.h_constrain { 1 } else { 0 }) << 1;
        b26 |= graphic.multi_line & 1;
        buf[26] = b26;

        // Offset 27: padding byte (reader skips this)

        // Offset 28-29: info (short)
        buf[28..30].copy_from_slice(&graphic.info.to_be_bytes());

        // Offset 30-33: gu union (4 bytes on 32-bit Mac; guThickness at [30-31])
        // OG Handle = 4 bytes (32-bit pointer), not 8.
        buf[30..32].copy_from_slice(&graphic.gu_thickness.to_be_bytes());
        // [32-33] unused union bytes, stay zero

        // Offset 34: fontInd
        buf[34] = graphic.font_ind as u8;

        // Offset 35: relFSize:1 | fontSize:7
        let mut b35: u8 = 0;
        b35 |= (if graphic.rel_f_size != 0 { 1 } else { 0 }) << 7;
        b35 |= graphic.font_size & 0x7F;
        buf[35] = b35;

        // Offset 36-37: fontStyle (short)
        buf[36..38].copy_from_slice(&graphic.font_style.to_be_bytes());

        // Offset 38-39: info2 (short)
        buf[38..40].copy_from_slice(&graphic.info2.to_be_bytes());

        // Offset 40-41: firstObj (LINK)
        let first_obj_idx = link_map.convert(graphic.first_obj);
        buf[40..42].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 42-43: lastObj (LINK)
        let last_obj_idx = link_map.convert(graphic.last_obj);
        buf[42..44].copy_from_slice(&last_obj_idx.to_be_bytes());
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
        // Offset 23: extHeader
        buf[23] = ottava.ext_header.staffn as u8;

        // Offset 24: noCutoff:1 | crossStaff:1 | crossSystem:1 | octSignType:5
        let mut b24: u8 = 0;
        b24 |= (if ottava.no_cutoff != 0 { 1 } else { 0 }) << 7;
        b24 |= (if ottava.cross_staff != 0 { 1 } else { 0 }) << 6;
        b24 |= (if ottava.cross_system != 0 { 1 } else { 0 }) << 5;
        b24 |= ottava.oct_sign_type & 0x1F;
        buf[24] = b24;

        // Offset 25: filler
        buf[25] = ottava.filler as u8;

        // Offset 26: numberVis:1 | unused1:1 | brackVis:1 | unused2:5
        let mut b26: u8 = 0;
        b26 |= (if ottava.number_vis { 1 } else { 0 }) << 7;
        b26 |= (if ottava.unused1 { 1 } else { 0 }) << 6;
        b26 |= (if ottava.brack_vis { 1 } else { 0 }) << 5;
        buf[26] = b26;

        // OTTAVA_5: DDISTs align to even after 4 byte fields at [23..27]
        // [28..30] nxd, [30..32] nyd, [32..34] xdFirst, [34..36] ydFirst
        // [36..38] xdLast, [38..40] ydLast
        buf[28..30].copy_from_slice(&ottava.nxd.to_be_bytes());
        buf[30..32].copy_from_slice(&ottava.nyd.to_be_bytes());
        buf[32..34].copy_from_slice(&ottava.xd_first.to_be_bytes());
        buf[34..36].copy_from_slice(&ottava.yd_first.to_be_bytes());
        buf[36..38].copy_from_slice(&ottava.xd_last.to_be_bytes());
        buf[38..40].copy_from_slice(&ottava.yd_last.to_be_bytes());
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
        // SLUR_5 after OBJECTHEADER_5(23):
        // [23] staffn (EXTOBJHEADER, 1 byte)
        // [24] voice (SignedByte)
        // [25] bitfields: filler:2|crossStaff:1|crossStfBack:1|crossSystem:1|tempFlag:1|used:1|tie:1
        // [26..28] firstSyncL (LINK, aligns to even — 26 is even ✓)
        // [28..30] lastSyncL (LINK)
        buf[23] = slur.ext_header.staffn as u8;
        buf[24] = slur.voice as u8;

        let mut b25: u8 = 0;
        b25 |= (slur.philler & 0x03) << 6;
        b25 |= (if slur.cross_staff != 0 { 1 } else { 0 }) << 5;
        b25 |= (if slur.cross_stf_back != 0 { 1 } else { 0 }) << 4;
        b25 |= (if slur.cross_system != 0 { 1 } else { 0 }) << 3;
        b25 |= (if slur.temp_flag { 1 } else { 0 }) << 2;
        b25 |= (if slur.used { 1 } else { 0 }) << 1;
        b25 |= if slur.tie { 1 } else { 0 };
        buf[25] = b25;

        let first_sync_idx = link_map.convert(slur.first_sync_l);
        buf[26..28].copy_from_slice(&first_sync_idx.to_be_bytes());

        let last_sync_idx = link_map.convert(slur.last_sync_l);
        buf[28..30].copy_from_slice(&last_sync_idx.to_be_bytes());
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
        // TUPLET_5 after OBJECTHEADER_5(23):
        // [23] staffn (EXTOBJHEADER)
        // [24] accNum, [25] accDenom, [26] voice
        // [27] bitfields: numVis:1|denomVis:1|brackVis:1|small:2|filler:3
        // [28..30] acnxd (DDIST, aligns to 28 even ✓)
        // [30..32] acnyd, [32..34] xdFirst, [34..36] ydFirst
        // [36..38] xdLast, [38..40] ydLast
        buf[23] = tuplet.ext_header.staffn as u8;
        buf[24] = tuplet.acc_num;
        buf[25] = tuplet.acc_denom;
        buf[26] = tuplet.voice as u8;

        let mut b27: u8 = 0;
        b27 |= (tuplet.num_vis & 1) << 7;
        b27 |= (tuplet.denom_vis & 1) << 6;
        b27 |= (tuplet.brack_vis & 1) << 5;
        b27 |= (tuplet.small & 0x03) << 3;
        b27 |= tuplet.filler & 0x07;
        buf[27] = b27;

        buf[28..30].copy_from_slice(&tuplet.acnxd.to_be_bytes());
        buf[30..32].copy_from_slice(&tuplet.acnyd.to_be_bytes());
        buf[32..34].copy_from_slice(&tuplet.xd_first.to_be_bytes());
        buf[34..36].copy_from_slice(&tuplet.yd_first.to_be_bytes());
        buf[36..38].copy_from_slice(&tuplet.xd_last.to_be_bytes());
        buf[38..40].copy_from_slice(&tuplet.yd_last.to_be_bytes());
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
        // Offset 23: extHeader
        buf[23] = tempo.ext_header.staffn as u8;

        // Offset 24: subType (beat unit)
        buf[24] = tempo.sub_type as u8;

        // Offset 25: expanded:1 | noMM:1 | filler:4 | dotted:1 | hideMM:1
        let mut b25: u8 = 0;
        b25 |= (if tempo.expanded { 1 } else { 0 }) << 7;
        b25 |= (if tempo.no_mm { 1 } else { 0 }) << 6;
        b25 |= (tempo.filler & 0x0F) << 2;
        b25 |= (if tempo.dotted { 1 } else { 0 }) << 1;
        b25 |= if tempo.hide_mm { 1 } else { 0 };
        buf[25] = b25;

        // Offset 26-27: tempoMM (i16)
        buf[26..28].copy_from_slice(&tempo.tempo_mm.to_be_bytes());

        // Offset 28-31: strOffset (u32)
        buf[28..32].copy_from_slice(&tempo.str_offset.to_be_bytes());

        // Offset 32-33: firstObjL (LINK)
        let first_obj_idx = link_map.convert(tempo.first_obj_l);
        buf[32..34].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 34-37: metroStrOffset (u32)
        buf[34..38].copy_from_slice(&tempo.metro_str_offset.to_be_bytes());
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
        // Offset 23: extHeader
        buf[23] = spacer.ext_header.staffn as u8;

        // Offset 24: bottomStaff
        buf[24] = spacer.bottom_staff as u8;

        // SPACER_5: STDIST(short) aligns to even: [26..28] spWidth
        buf[26..28].copy_from_slice(&spacer.sp_width.to_be_bytes());
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
        // Offset 23: extHeader
        buf[23] = ending.ext_header.staffn as u8;

        // Offset 24-25: firstObjL (LINK)
        let first_obj_idx = link_map.convert(ending.first_obj_l);
        buf[24..26].copy_from_slice(&first_obj_idx.to_be_bytes());

        // Offset 26-27: lastObjL (LINK)
        let last_obj_idx = link_map.convert(ending.last_obj_l);
        buf[26..28].copy_from_slice(&last_obj_idx.to_be_bytes());

        // Offset 28: noLCutoff:1 | noRCutoff:1 | endNum:6
        let mut b28: u8 = 0;
        b28 |= (if ending.no_l_cutoff != 0 { 1 } else { 0 }) << 7;
        b28 |= (if ending.no_r_cutoff != 0 { 1 } else { 0 }) << 6;
        b28 |= ending.end_num & 0x3F;
        buf[28] = b28;

        // ENDING_5: DDIST endxd aligns to even after byte at 28: [30..32]
        buf[30..32].copy_from_slice(&ending.endxd.to_be_bytes());
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
        // Offset 23: filler
        buf[23] = psmeas.filler as u8;
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

    // OG sizeAllObjsFile = bytes of object data only (does NOT include itself).
    // Confirmed from HeapFileIO.cp line 298:
    //   sizeAllObjsFile = endPosition - startPosition - sizeof(long)
    // where startPosition is captured before writing the 4-byte placeholder.
    let total_size = object_data.len() as u32;

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

        let idx1 = map.register(42);
        let idx2 = map.register(100);
        let idx3 = map.register(42); // duplicate

        assert_eq!(idx1, 1);
        assert_eq!(idx2, 2);
        assert_eq!(idx3, 1);

        assert_eq!(map.convert(42), 1);
        assert_eq!(map.convert(100), 2);
        assert_eq!(map.convert(0), 0); // NILINK stays 0
        assert_eq!(map.convert(999), 0); // unregistered returns 0
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

        let mut buf = vec![0u8; 23];
        pack_objectheader_n105(&header, &map, &mut buf);

        assert_eq!(buf.len(), 23);

        // LINKs converted to sequential file indices
        assert_eq!(i16::from_be_bytes([buf[0], buf[1]]), 1); // right
        assert_eq!(i16::from_be_bytes([buf[2], buf[3]]), 2); // left
        assert_eq!(i16::from_be_bytes([buf[4], buf[5]]), 3); // firstSubObj

        // Coordinates
        assert_eq!(i16::from_be_bytes([buf[6], buf[7]]), 100); // xd
        assert_eq!(i16::from_be_bytes([buf[8], buf[9]]), 200); // yd

        // Type byte
        assert_eq!(buf[10], 2);

        // Bitfield: selected (bit 7) and visible (bit 6) set
        assert_eq!(buf[11] & 0xC0, 0xC0);
    }

    #[test]
    fn test_pack_objectheader_nilink() {
        let header = ObjectHeader {
            right: 0,
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
        let mut buf = vec![0u8; 23];
        pack_objectheader_n105(&header, &map, &mut buf);

        // NILINK (0) stays 0
        assert_eq!(i16::from_be_bytes([buf[0], buf[1]]), 0);
        assert_eq!(i16::from_be_bytes([buf[2], buf[3]]), 0);
        assert_eq!(i16::from_be_bytes([buf[4], buf[5]]), 0);

        // Bitfield: soft (bit 5) and tweaked (bit 3) set
        assert_eq!(buf[11] & 0x28, 0x28);
    }
}
