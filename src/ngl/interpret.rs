//! Heap interpreter: decodes N105 binary format to typed Rust structs.
//!
//! This module provides the critical layer between raw .ngl file bytes and the
//! high-level Rust data model. It handles:
//!
//! - **N105 object header unpacking** (23-byte common prefix + type-specific data)
//! - **PowerPC bitfield unpacking** (MSB-first within bytes)
//! - **Subobject decoding** (30-byte ANOTE records, etc.)
//! - **Sequential subobject walking** for BeamSets and Slurs
//!
//! ## Critical N105 Details
//!
//! - **1-based heap indexing**: Slot 0 is unused, NILINK=0 means "no link"
//! - **Big-endian encoding**: All multi-byte values are big-endian
//! - **Bitfield packing**: PowerPC convention = MSB first within bytes
//!
//! ## Example: OBJECTHEADER_5 (23 bytes)
//!
//! ```text
//! Offset  Type      Field
//! ------  --------  -----
//! 0-1     u16       right
//! 2-3     u16       left
//! 4-5     u16       firstSubObj
//! 6-7     i16       xd
//! 8-9     i16       yd
//! 10      i8        type
//! 11      byte      flags: selected:1 | visible:1 | soft:1 | valid:1 | tweaked:1 | spareFlag:1 | filler:2
//! 12-19   Rect      objRect (4 x i16)
//! 20      i8        relSize
//! 21      i8        filler
//! 22      u8        nEntries
//! ```
//!
//! Source: NObjTypesN105.h lines 12-27, Ngale5ProgQuickRef-TN1.txt lines 214-229

use crate::basic_types::{DRect, Link, Rect, NILINK};
use crate::obj_types::{
    AClef, AConnect, ADynamic, AGraphic, AKeySig, AMeasure, AModNr, ANote, ANoteBeam, ANoteOttava,
    ANoteTuple, APsMeas, ARptEnd, ASlur, AStaff, ATimeSig, BeamSet, Clef, Connect, Dynamic, Ending,
    GrSync, Graphic, Header, KeySig, Measure, ObjectHeader, Ottava, Page, PsMeas, RptEnd, Slur,
    Spacer, Staff, Sync, System, Tail, Tempo, TimeSig, Tuplet,
};
use std::collections::HashMap;

// Re-export all unpacking functions from submodules for backward compatibility
pub use super::unpack_headers::{
    unpack_ksinfo_n105, unpack_object_header_n105, unpack_subobj_header_n105,
};
pub use super::unpack_notation::{unpack_aclef_n105, unpack_akeysig_n105, unpack_atimesig_n105};
pub use super::unpack_notes::{unpack_anote_n105, unpack_anotebeam_n105, unpack_anotetuple_n105};
pub use super::unpack_slur::unpack_aslur_n105;
pub use super::unpack_structural::{unpack_ameasure_n105, unpack_astaff_n105};
pub use super::unpack_stubs::{
    unpack_aconnect_n105, unpack_adynamic_n105, unpack_agraphic_n105, unpack_amodnr_n105,
    unpack_anoteottava_n105, unpack_apsmeas_n105, unpack_arptend_n105,
};

/// InterpretedScore: all decoded objects and subobjects from a .ngl file.
///
/// This struct holds the fully interpreted score data, organized by type for efficient access.
#[derive(Debug, Clone)]
pub struct InterpretedScore {
    /// Link to the score HEADER object (start of score linked list).
    /// For NGL files, this comes from ScoreHeader.head_l (same as OG doc->headL).
    /// For Notelist-generated scores, this is the first HEADER link.
    /// The walk() method starts from this object's `right` pointer.
    pub head_l: Link,

    /// All objects in heap order (1-based indexing: slot 0 unused)
    pub objects: Vec<InterpretedObject>,

    // Subobject storage by type
    /// Type 0 subobjects: PARTINFO (62 bytes)
    pub part_infos: HashMap<Link, Vec<u8>>, // Raw bytes for now, full decode TBD
    /// Type 2 subobjects: ANOTE (30 bytes)
    pub notes: HashMap<Link, Vec<ANote>>,
    /// Type 3 subobjects: ARPTEND (6 bytes)
    pub rptend_subs: HashMap<Link, Vec<ARptEnd>>,
    /// Type 6 subobjects: ASTAFF (50 bytes)
    pub staffs: HashMap<Link, Vec<AStaff>>,
    /// Type 7 subobjects: AMEASURE (40 bytes)
    pub measures: HashMap<Link, Vec<AMeasure>>,
    /// Type 8 subobjects: ACLEF (10 bytes)
    pub clefs: HashMap<Link, Vec<AClef>>,
    /// Type 9 subobjects: AKEYSIG (24 bytes)
    pub keysigs: HashMap<Link, Vec<AKeySig>>,
    /// Type 10 subobjects: ATIMESIG (12 bytes)
    pub timesigs: HashMap<Link, Vec<ATimeSig>>,
    /// Type 11 subobjects: ANOTEBEAM (6 bytes)
    pub notebeams: HashMap<Link, Vec<ANoteBeam>>,
    /// Type 12 subobjects: ACONNECT (12 bytes)
    pub connects: HashMap<Link, Vec<AConnect>>,
    /// Type 13 subobjects: ADYNAMIC (12 bytes)
    pub dynamics: HashMap<Link, Vec<ADynamic>>,
    /// Type 14 subobjects: AMODNR (6 bytes)
    pub modnrs: HashMap<Link, Vec<AModNr>>,
    /// Type 15 subobjects: AGRAPHIC (6 bytes)
    pub graphics: HashMap<Link, Vec<AGraphic>>,
    /// Type 16 subobjects: ANOTEOTTAVA (4 bytes)
    pub ottavas: HashMap<Link, Vec<ANoteOttava>>,
    /// Type 17 subobjects: ASLUR (42 bytes)
    pub slurs: HashMap<Link, Vec<ASlur>>,
    /// Type 18 subobjects: ANOTETUPLE (4 bytes)
    pub tuplets: HashMap<Link, Vec<ANoteTuple>>,
    /// Type 19 subobjects: AGRNOTE (30 bytes, same as ANOTE)
    pub grnotes: HashMap<Link, Vec<ANote>>,
    /// Type 23 subobjects: APSMEAS (6 bytes)
    pub psmeas_subs: HashMap<Link, Vec<APsMeas>>,
}

/// InterpretedObject: a single object with its header and type-specific data.
#[derive(Debug, Clone)]
pub struct InterpretedObject {
    /// 1-based index in heap (matches LINK values)
    pub index: Link,
    /// Common object header
    pub header: ObjectHeader,
    /// Type-specific data
    pub data: ObjData,
}

/// ObjData: type-specific object data (enum of all object types).
#[derive(Debug, Clone)]
pub enum ObjData {
    Header(Header),
    Tail(Tail),
    Sync(Sync),
    RptEnd(RptEnd),
    Page(Page),
    System(System),
    Staff(Staff),
    Measure(Measure),
    Clef(Clef),
    KeySig(KeySig),
    TimeSig(TimeSig),
    BeamSet(BeamSet),
    Connect(Connect),
    Dynamic(Dynamic),
    Graphic(Graphic),
    Ottava(Ottava),
    Slur(Slur),
    Tuplet(Tuplet),
    GrSync(GrSync),
    Tempo(Tempo),
    Spacer(Spacer),
    Ending(Ending),
    PsMeas(PsMeas),
}

impl Default for InterpretedScore {
    fn default() -> Self {
        Self::new()
    }
}

impl InterpretedScore {
    /// Create a new empty InterpretedScore.
    pub fn new() -> Self {
        Self {
            head_l: NILINK,
            objects: Vec::new(),
            part_infos: HashMap::new(),
            notes: HashMap::new(),
            rptend_subs: HashMap::new(),
            staffs: HashMap::new(),
            measures: HashMap::new(),
            clefs: HashMap::new(),
            keysigs: HashMap::new(),
            timesigs: HashMap::new(),
            notebeams: HashMap::new(),
            connects: HashMap::new(),
            dynamics: HashMap::new(),
            modnrs: HashMap::new(),
            graphics: HashMap::new(),
            ottavas: HashMap::new(),
            slurs: HashMap::new(),
            tuplets: HashMap::new(),
            grnotes: HashMap::new(),
            psmeas_subs: HashMap::new(),
        }
    }

    /// Get an object by link.
    ///
    /// First tries fast index-based lookup (link == index + 1, true for NGL-parsed scores).
    /// Falls back to linear search for synthesized scores where links may not match indices.
    ///
    /// Returns `None` if link is NILINK or not found.
    pub fn get(&self, link: Link) -> Option<&InterpretedObject> {
        if link == NILINK || link == 0 {
            return None;
        }
        // Fast path: check if link == index + 1 (true for NGL binary files)
        let idx = (link - 1) as usize;
        if let Some(obj) = self.objects.get(idx) {
            if obj.index == link {
                return Some(obj);
            }
        }
        // Slow path: linear search (for synthesized scores with non-sequential links)
        self.objects.iter().find(|obj| obj.index == link)
    }

    /// Walk objects in linked-list order (following `right` links).
    ///
    /// Starts from the HEADER object at `head_l` (equivalent to OG `doc->headL`)
    /// and follows `right` pointers through to TAIL. This correctly skips
    /// the master page object list which shares the same heap.
    ///
    /// Reference: HeapFileIO.cp, WriteHeap() — score list starts at headL,
    /// master page list starts at masterHeadL.
    pub fn walk(&self) -> impl Iterator<Item = &InterpretedObject> {
        // Start from the HEADER object identified by head_l.
        // The first object yielded is the one HEADER.right points to
        // (usually the first PAGE object).
        let start = if self.head_l != NILINK {
            self.get(self.head_l).map(|obj| obj.header.right)
        } else {
            // Fallback: if head_l not set, use first object (legacy behavior)
            self.objects.first().map(|obj| obj.header.right)
        };
        ObjectWalker {
            score: self,
            current: start,
        }
    }

    /// Get notes for a Sync (or GrSync).
    ///
    /// Returns the notes starting at `first_sub` in the notes HashMap.
    pub fn get_notes(&self, first_sub: Link) -> Vec<ANote> {
        self.notes.get(&first_sub).cloned().unwrap_or_default()
    }

    /// Get notebeam subobjects for a BeamSet (sequential, not chained).
    ///
    /// BeamSets use **sequential storage**: subobjects are stored consecutively
    /// starting at `first_sub`, NOT linked by next-pointers.
    pub fn get_notebeam_subs(&self, first_sub: Link, count: u8) -> Vec<ANoteBeam> {
        self.notebeams
            .get(&first_sub)
            .map(|beams| beams.iter().take(count as usize).cloned().collect())
            .unwrap_or_default()
    }

    /// Get slur subobjects for a Slur (sequential, not chained).
    ///
    /// Slurs use **sequential storage**: subobjects are stored consecutively
    /// starting at `first_sub`, NOT linked by next-pointers.
    pub fn get_slur_subs(&self, first_sub: Link, count: u8) -> Vec<ASlur> {
        self.slurs
            .get(&first_sub)
            .map(|slurs| slurs.iter().take(count as usize).cloned().collect())
            .unwrap_or_default()
    }

    /// Get the head (first) object of the score list.
    ///
    /// Returns the first HEADER object if present (index 1 in typical files).
    pub fn head(&self) -> Option<&InterpretedObject> {
        self.objects.first()
    }

    /// Get the tail (last) object of the score list.
    ///
    /// Returns the last object following the linked list from head.
    /// In practice, this walks the `right` links until finding NILINK.
    pub fn tail(&self) -> Option<&InterpretedObject> {
        if self.objects.is_empty() {
            return None;
        }

        // Walk from head following right links until we find the tail
        let mut current = self.objects.first()?;
        while current.header.right != NILINK {
            current = self.get(current.header.right)?;
        }
        Some(current)
    }

    /// Count the number of staves in the score.
    ///
    /// This counts AStaff subobjects in the first Staff object found in the score.
    /// All Staff objects in a Nightingale score have the same number of staves.
    pub fn num_staves(&self) -> usize {
        // Find the first Staff object (type 6)
        for obj in &self.objects {
            if obj.header.obj_type == STAFF_TYPE as i8 {
                return obj.header.n_entries as usize;
            }
        }
        0
    }

    /// Get the score object list (HEADER→...→TAIL) as a Vec.
    ///
    /// Returns all objects in the main score list by walking the `right` links.
    pub fn score_list(&self) -> Vec<&InterpretedObject> {
        let mut result = Vec::new();
        if self.objects.is_empty() {
            return result;
        }

        // Start from the first object (should be HEADER)
        let mut current_link = 1;
        while let Some(obj) = self.get(current_link) {
            result.push(obj);
            if obj.header.right == NILINK {
                break;
            }
            current_link = obj.header.right;
        }
        result
    }

    /// Get the master page list (second HEADER→...→TAIL) as a Vec.
    ///
    /// The master page list typically starts after the main score list.
    /// We identify it by finding a second HEADER object.
    pub fn master_page_list(&self) -> Vec<&InterpretedObject> {
        let mut result = Vec::new();

        // Find the second HEADER (master page list head)
        let mut header_count = 0;
        let mut start_link = NILINK;

        for obj in &self.objects {
            if obj.header.obj_type == HEADER_TYPE as i8 {
                header_count += 1;
                if header_count == 2 {
                    start_link = obj.index;
                    break;
                }
            }
        }

        if start_link == NILINK {
            return result;
        }

        // Walk the master page list
        let mut current_link = start_link;
        while let Some(obj) = self.get(current_link) {
            result.push(obj);
            if obj.header.right == NILINK {
                break;
            }
            current_link = obj.header.right;
        }
        result
    }

    /// Count objects by type.
    ///
    /// Returns the number of objects with the given type byte.
    pub fn count_by_type(&self, obj_type: u8) -> usize {
        self.objects
            .iter()
            .filter(|obj| obj.header.obj_type == obj_type as i8)
            .count()
    }

    /// Get all SYNCs (note/rest containers) from the score list.
    ///
    /// Returns only objects with type SYNC_TYPE (2).
    pub fn syncs(&self) -> Vec<&InterpretedObject> {
        self.objects
            .iter()
            .filter(|obj| obj.header.obj_type == SYNC_TYPE as i8)
            .collect()
    }

    /// Get all MEASUREs from the score list.
    ///
    /// Returns only objects with type MEASURE_TYPE (7).
    pub fn measure_objects(&self) -> Vec<&InterpretedObject> {
        self.objects
            .iter()
            .filter(|obj| obj.header.obj_type == MEASURE_TYPE as i8)
            .collect()
    }

    /// Decode a string from the string pool at the given offset.
    ///
    /// This is a convenience wrapper around the reader's decode_string function.
    /// The string pool is typically from NglFile::string_pool.
    ///
    /// Returns Some(String) if successful, None if the offset is invalid.
    pub fn decode_string(pool: &[u8], offset: i32) -> Option<String> {
        reader_decode_string(pool, offset)
    }
}

struct ObjectWalker<'a> {
    score: &'a InterpretedScore,
    current: Option<Link>,
}

impl<'a> Iterator for ObjectWalker<'a> {
    type Item = &'a InterpretedObject;

    fn next(&mut self) -> Option<Self::Item> {
        let link = self.current?;
        let obj = self.score.get(link)?;
        self.current = if obj.header.right != NILINK {
            Some(obj.header.right)
        } else {
            None
        };
        Some(obj)
    }
}

// =============================================================================
// Heap Interpretation
// =============================================================================

// NOTE: All N105 unpacking functions have been moved to dedicated submodules:
//   unpack_headers.rs    — ObjectHeader, SubObjHeader, KsInfo
//   unpack_notes.rs      — ANote, ANoteBeam, ANoteTuple
//   unpack_structural.rs — AStaff, AMeasure
//   unpack_notation.rs   — AClef, AKeySig, ATimeSig
//   unpack_slur.rs       — ASlur
//   unpack_stubs.rs      — AConnect, ADynamic, AModNr, AGraphic, ANoteOttava, ARptEnd, APsMeas
// They are re-exported via `pub use` at the top of this file for backward compatibility.

use crate::defs::*;
use crate::ngl::reader::{decode_string as reader_decode_string, NglFile};

/// Interpret all heaps from an NGL file into an InterpretedScore.
///
/// This is the main entry point for converting raw .ngl binary data into
/// typed Rust structs. It:
/// 1. Walks the object heap (heap 24) and unpacks all objects
/// 2. For each object with subobjects, unpacks the subobject heap
/// 3. Stores everything in InterpretedScore for efficient access
///
/// **Critical**: The object heap (type 24) stores objects in **variable-length**
/// format in the file. Each object uses only its type-specific byte count
/// (from N105_OBJ_SIZES), NOT the uniform obj_size stride. The C++ reader
/// calls MoveObjSubobjs() to expand objects to uniform slots after reading.
/// We replicate this by walking the packed data and assigning sequential
/// 1-based indices.
///
/// Source: HeapFileIO.cp ReadObjHeap() (line 973), WriteObject() (line 659)
pub fn interpret_heap(ngl: &NglFile) -> Result<InterpretedScore, String> {
    let mut score = InterpretedScore::new();

    // Parse head_l from score header — equivalent to OG doc->headL.
    // This is the first field of ScoreHeader (2 bytes, big-endian u16).
    // Reference: NObjTypesN105.h DOCUMENTHDR, ScoreHeader.head_l
    if ngl.score_header_raw.len() >= 2 {
        score.head_l = u16::from_be_bytes([ngl.score_header_raw[0], ngl.score_header_raw[1]]);
    }

    // Get the object heap (type 24)
    let obj_heap = &ngl.heaps[OBJ_TYPE as usize];
    let obj_size = obj_heap.obj_size as usize; // uniform in-memory size (e.g. 46)
    let obj_data = &obj_heap.obj_data;

    // The reader prepends obj_size bytes of zeros for slot 0 (NILINK),
    // then the rest is sizeAllObjsFile bytes of variable-length packed objects.
    // We walk the packed region, reading each object's type byte at offset 10
    // to determine its actual file size from N105_OBJ_SIZES.

    let data_start = obj_size; // skip slot 0 padding
    let data_end = obj_data.len();
    let mut cursor = data_start;
    let mut obj_idx: u16 = 1; // 1-based index matching C++ LINK values

    while cursor < data_end && obj_idx <= obj_heap.obj_count {
        // Need at least 23 bytes for the object header
        if cursor + 23 > data_end {
            break;
        }

        // Read the type byte at offset 10 within the object header
        let obj_type = obj_data[cursor + 10];

        // Look up the actual file size for this object type
        let file_obj_size = if (obj_type as usize) < crate::obj_types::N105_OBJ_SIZES.len() {
            crate::obj_types::N105_OBJ_SIZES[obj_type as usize] as usize
        } else {
            // Invalid type — bail out since data is corrupt
            eprintln!(
                "Warning: Object {} at offset {} has invalid type {}, stopping",
                obj_idx, cursor, obj_type as i8
            );
            break;
        };

        if file_obj_size == 0 {
            // Type 14 (MODNR) has 0 object size — no MODNR objects exist
            eprintln!(
                "Warning: Object {} has zero-length type {}, stopping",
                obj_idx, obj_type
            );
            break;
        }

        if cursor + file_obj_size > data_end {
            eprintln!(
                "Warning: Object {} at offset {} truncated (need {} bytes, have {})",
                obj_idx,
                cursor,
                file_obj_size,
                data_end - cursor
            );
            break;
        }

        // Pad to uniform obj_size for header unpacking (some unpackers check len >= obj_size)
        let mut obj_bytes_padded = vec![0u8; obj_size.max(file_obj_size)];
        obj_bytes_padded[..file_obj_size]
            .copy_from_slice(&obj_data[cursor..cursor + file_obj_size]);
        let obj_bytes = &obj_bytes_padded[..];

        // Unpack the 23-byte object header
        let header = unpack_object_header_n105(obj_bytes)?;

        // Based on obj_type, unpack the type-specific data after byte 23
        let data = match header.obj_type as u8 {
            HEADER_TYPE => ObjData::Header(Header {
                header: header.clone(),
            }),
            TAIL_TYPE => ObjData::Tail(Tail {
                header: header.clone(),
            }),

            SYNC_TYPE => {
                let time_stamp = if obj_bytes.len() >= 25 {
                    u16::from_be_bytes([obj_bytes[23], obj_bytes[24]])
                } else {
                    0
                };
                ObjData::Sync(Sync {
                    header: header.clone(),
                    time_stamp,
                })
            }

            MEASURE_TYPE => {
                // Measure: 46 bytes total, 23 bytes after header
                let filler_m = if obj_bytes.len() > 23 {
                    obj_bytes[23] as i8
                } else {
                    0
                };
                let l_measure = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_measure = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let system_l = if obj_bytes.len() >= 30 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                let staff_l = if obj_bytes.len() >= 32 {
                    u16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                } else {
                    NILINK
                };
                let fake_meas = if obj_bytes.len() >= 34 {
                    i16::from_be_bytes([obj_bytes[32], obj_bytes[33]])
                } else {
                    0
                };
                let space_percent = if obj_bytes.len() >= 36 {
                    i16::from_be_bytes([obj_bytes[34], obj_bytes[35]])
                } else {
                    100
                };
                let measure_b_box = if obj_bytes.len() >= 44 {
                    Rect {
                        top: i16::from_be_bytes([obj_bytes[36], obj_bytes[37]]),
                        left: i16::from_be_bytes([obj_bytes[38], obj_bytes[39]]),
                        bottom: i16::from_be_bytes([obj_bytes[40], obj_bytes[41]]),
                        right: i16::from_be_bytes([obj_bytes[42], obj_bytes[43]]),
                    }
                } else {
                    Rect {
                        top: 0,
                        left: 0,
                        bottom: 0,
                        right: 0,
                    }
                };
                let l_time_stamp = if obj_bytes.len() >= 48 {
                    i32::from_be_bytes([obj_bytes[44], obj_bytes[45], obj_bytes[46], obj_bytes[47]])
                } else {
                    0
                };
                ObjData::Measure(Measure {
                    header: header.clone(),
                    filler_m,
                    l_measure,
                    r_measure,
                    system_l,
                    staff_l,
                    fake_meas,
                    space_percent,
                    measure_b_box,
                    l_time_stamp,
                })
            }

            STAFF_TYPE => {
                let l_staff = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_staff = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let system_l = if obj_bytes.len() >= 30 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                ObjData::Staff(Staff {
                    header: header.clone(),
                    l_staff,
                    r_staff,
                    system_l,
                })
            }

            SYSTEM_TYPE => {
                let l_system = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_system = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let page_l = if obj_bytes.len() >= 30 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                let system_num = if obj_bytes.len() >= 32 {
                    i16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                } else {
                    0
                };
                let system_rect = if obj_bytes.len() >= 40 {
                    DRect {
                        top: i16::from_be_bytes([obj_bytes[32], obj_bytes[33]]),
                        left: i16::from_be_bytes([obj_bytes[34], obj_bytes[35]]),
                        bottom: i16::from_be_bytes([obj_bytes[36], obj_bytes[37]]),
                        right: i16::from_be_bytes([obj_bytes[38], obj_bytes[39]]),
                    }
                } else {
                    DRect {
                        top: 0,
                        left: 0,
                        bottom: 0,
                        right: 0,
                    }
                };
                ObjData::System(System {
                    header: header.clone(),
                    l_system,
                    r_system,
                    page_l,
                    system_num,
                    system_rect,
                    sys_desc_ptr: 0,
                })
            }

            PAGE_TYPE => {
                let l_page = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_page = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let sheet_num = if obj_bytes.len() >= 30 {
                    i16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    0
                };
                let header_str_offset = if obj_bytes.len() >= 34 {
                    i32::from_be_bytes([obj_bytes[30], obj_bytes[31], obj_bytes[32], obj_bytes[33]])
                } else {
                    0
                };
                let footer_str_offset = if obj_bytes.len() >= 38 {
                    i32::from_be_bytes([obj_bytes[34], obj_bytes[35], obj_bytes[36], obj_bytes[37]])
                } else {
                    0
                };
                ObjData::Page(Page {
                    header: header.clone(),
                    l_page,
                    r_page,
                    sheet_num,
                    header_str_offset,
                    footer_str_offset,
                })
            }

            CLEF_TYPE => {
                let in_measure = if obj_bytes.len() > 23 {
                    obj_bytes[23] != 0
                } else {
                    false
                };
                ObjData::Clef(Clef {
                    header: header.clone(),
                    in_measure,
                })
            }

            KEYSIG_TYPE => {
                let in_measure = if obj_bytes.len() > 23 {
                    obj_bytes[23] != 0
                } else {
                    false
                };
                ObjData::KeySig(KeySig {
                    header: header.clone(),
                    in_measure,
                })
            }

            TIMESIG_TYPE => {
                let in_measure = if obj_bytes.len() > 23 {
                    obj_bytes[23] != 0
                } else {
                    false
                };
                ObjData::TimeSig(TimeSig {
                    header: header.clone(),
                    in_measure,
                })
            }

            BEAMSET_TYPE | SLUR_TYPE | TUPLET_TYPE | GRAPHIC_TYPE | OTTAVA_TYPE | SPACER_TYPE
            | ENDING_TYPE | TEMPO_TYPE => {
                // These all have ExtObjHeader (staffn byte at offset 23)
                // For now, create minimal objects - full implementation can be added later
                match header.obj_type as u8 {
                    BEAMSET_TYPE => {
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        ObjData::BeamSet(BeamSet {
                            header: header.clone(),
                            ext_header,
                            voice: if obj_bytes.len() > 24 {
                                obj_bytes[24] as i8
                            } else {
                                1
                            },
                            thin: 0,
                            beam_rests: 0,
                            feather: 0,
                            grace: 0,
                            first_system: 0,
                            cross_staff: 0,
                            cross_system: 0,
                        })
                    }
                    SLUR_TYPE => {
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        ObjData::Slur(Slur {
                            header: header.clone(),
                            ext_header,
                            voice: if obj_bytes.len() > 24 {
                                obj_bytes[24] as i8
                            } else {
                                1
                            },
                            philler: 0,
                            cross_staff: 0,
                            cross_stf_back: 0,
                            cross_system: 0,
                            temp_flag: false,
                            used: false,
                            tie: false,
                            first_sync_l: NILINK,
                            last_sync_l: NILINK,
                        })
                    }
                    TUPLET_TYPE => {
                        // N105 TUPLET object: 40 bytes total
                        // 0-22: OBJECTHEADER_5 (23 bytes)
                        // 23:   staffn (EXTOBJHEADER)
                        // 24:   accNum
                        // 25:   accDenom
                        // 26:   voice (SignedByte)
                        // 27:   numVis
                        // 28:   denomVis
                        // 29:   brackVis
                        // 30:   small
                        // 31:   filler
                        // 32-33: xdFirst (DDIST)
                        // 34-35: ydFirst (DDIST)
                        // 36-37: xdLast (DDIST)
                        // 38-39: ydLast (DDIST)
                        // Note: acnxd/acnyd ("now unused") are NOT in N105 disk format
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        let acc_num = if obj_bytes.len() > 24 {
                            obj_bytes[24]
                        } else {
                            3
                        };
                        let acc_denom = if obj_bytes.len() > 25 {
                            obj_bytes[25]
                        } else {
                            2
                        };
                        let voice = if obj_bytes.len() > 26 {
                            obj_bytes[26] as i8
                        } else {
                            1
                        };
                        let num_vis = if obj_bytes.len() > 27 {
                            obj_bytes[27]
                        } else {
                            1
                        };
                        let denom_vis = if obj_bytes.len() > 28 {
                            obj_bytes[28]
                        } else {
                            0
                        };
                        let brack_vis = if obj_bytes.len() > 29 {
                            obj_bytes[29]
                        } else {
                            1
                        };
                        let small = if obj_bytes.len() > 30 {
                            obj_bytes[30]
                        } else {
                            0
                        };
                        let filler = if obj_bytes.len() > 31 {
                            obj_bytes[31]
                        } else {
                            0
                        };
                        let xd_first = if obj_bytes.len() > 33 {
                            i16::from_be_bytes([obj_bytes[32], obj_bytes[33]])
                        } else {
                            0
                        };
                        let yd_first = if obj_bytes.len() > 35 {
                            i16::from_be_bytes([obj_bytes[34], obj_bytes[35]])
                        } else {
                            0
                        };
                        let xd_last = if obj_bytes.len() > 37 {
                            i16::from_be_bytes([obj_bytes[36], obj_bytes[37]])
                        } else {
                            0
                        };
                        let yd_last = if obj_bytes.len() > 39 {
                            i16::from_be_bytes([obj_bytes[38], obj_bytes[39]])
                        } else {
                            0
                        };
                        ObjData::Tuplet(crate::obj_types::Tuplet {
                            header: header.clone(),
                            ext_header,
                            acc_num,
                            acc_denom,
                            voice,
                            num_vis,
                            denom_vis,
                            brack_vis,
                            small,
                            filler,
                            acnxd: 0, // not stored in N105
                            acnyd: 0, // not stored in N105
                            xd_first,
                            yd_first,
                            xd_last,
                            yd_last,
                        })
                    }
                    _ => ObjData::GrSync(GrSync {
                        header: header.clone(),
                    }),
                }
            }

            GRSYNC_TYPE => ObjData::GrSync(GrSync {
                header: header.clone(),
            }),
            PSMEAS_TYPE => ObjData::PsMeas(PsMeas {
                header: header.clone(),
                filler: 0,
            }),

            RPTEND_TYPE | CONNECT_TYPE | DYNAMIC_TYPE => {
                // Simple objects with minimal unpacking for now
                ObjData::GrSync(GrSync {
                    header: header.clone(),
                })
            }

            _ => {
                // Should not happen — we already validated the type above
                eprintln!(
                    "Warning: Skipping object {} with unhandled type: {}",
                    obj_idx, header.obj_type
                );
                // Still advance past this object (we know its size from the type lookup)
                cursor += file_obj_size;
                obj_idx += 1;
                continue;
            }
        };

        score.objects.push(InterpretedObject {
            index: obj_idx,
            header,
            data,
        });

        // Advance cursor past this variable-length object
        cursor += file_obj_size;
        obj_idx += 1;
    }

    // Now unpack subobject heaps for objects that have subobjects
    for obj in &score.objects {
        if obj.header.first_sub_obj == NILINK || obj.header.first_sub_obj == 0 {
            continue;
        }

        let heap_type = obj.header.obj_type as usize;
        if heap_type >= ngl.heaps.len() {
            continue;
        }

        let subobj_heap = &ngl.heaps[heap_type];
        if subobj_heap.obj_count == 0 {
            continue;
        }

        let sub_size = subobj_heap.obj_size as usize;
        let sub_data = &subobj_heap.obj_data;

        // Unpack subobjects based on type
        match obj.header.obj_type as u8 {
            SYNC_TYPE | GRSYNC_TYPE => {
                // Unpack ANOTE subobjects
                let mut notes = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(note) = unpack_anote_n105(&sub_data[offset..offset + sub_size]) {
                            notes.push(note);
                        }
                    }
                }
                if !notes.is_empty() {
                    if obj.header.obj_type as u8 == SYNC_TYPE {
                        score.notes.insert(obj.header.first_sub_obj, notes);
                    } else {
                        score.grnotes.insert(obj.header.first_sub_obj, notes);
                    }
                }
            }

            STAFF_TYPE => {
                // Unpack ASTAFF subobjects
                let mut staffs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(staff) = unpack_astaff_n105(&sub_data[offset..offset + sub_size])
                        {
                            staffs.push(staff);
                        }
                    }
                }
                if !staffs.is_empty() {
                    score.staffs.insert(obj.header.first_sub_obj, staffs);
                }
            }

            MEASURE_TYPE => {
                // Unpack AMEASURE subobjects
                let mut measures = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(meas) = unpack_ameasure_n105(&sub_data[offset..offset + sub_size])
                        {
                            measures.push(meas);
                        }
                    }
                }
                if !measures.is_empty() {
                    score.measures.insert(obj.header.first_sub_obj, measures);
                }
            }

            CLEF_TYPE => {
                // Unpack ACLEF subobjects
                let mut clefs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(clef) = unpack_aclef_n105(&sub_data[offset..offset + sub_size]) {
                            clefs.push(clef);
                        }
                    }
                }
                if !clefs.is_empty() {
                    score.clefs.insert(obj.header.first_sub_obj, clefs);
                }
            }

            KEYSIG_TYPE => {
                // Unpack AKEYSIG subobjects
                let mut keysigs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(ks) = unpack_akeysig_n105(&sub_data[offset..offset + sub_size]) {
                            keysigs.push(ks);
                        }
                    }
                }
                if !keysigs.is_empty() {
                    score.keysigs.insert(obj.header.first_sub_obj, keysigs);
                }
            }

            TIMESIG_TYPE => {
                // Unpack ATIMESIG subobjects
                let mut timesigs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(ts) = unpack_atimesig_n105(&sub_data[offset..offset + sub_size]) {
                            timesigs.push(ts);
                        }
                    }
                }
                if !timesigs.is_empty() {
                    score.timesigs.insert(obj.header.first_sub_obj, timesigs);
                }
            }

            BEAMSET_TYPE => {
                // Unpack ANOTEBEAM subobjects
                let mut notebeams = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(beam) =
                            unpack_anotebeam_n105(&sub_data[offset..offset + sub_size])
                        {
                            notebeams.push(beam);
                        }
                    }
                }
                if !notebeams.is_empty() {
                    score.notebeams.insert(obj.header.first_sub_obj, notebeams);
                }
            }

            SLUR_TYPE => {
                // Unpack ASLUR subobjects
                let mut slurs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(slur) = unpack_aslur_n105(&sub_data[offset..offset + sub_size]) {
                            slurs.push(slur);
                        }
                    }
                }
                if !slurs.is_empty() {
                    score.slurs.insert(obj.header.first_sub_obj, slurs);
                }
            }

            TUPLET_TYPE => {
                // Unpack ANOTETUPLE subobjects (4 bytes each: next + tpSync)
                let mut notetuples = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(nt) = unpack_anotetuple_n105(&sub_data[offset..offset + sub_size])
                        {
                            notetuples.push(nt);
                        }
                    }
                }
                if !notetuples.is_empty() {
                    score.tuplets.insert(obj.header.first_sub_obj, notetuples);
                }
            }

            _ => {
                // Other subobject types not yet implemented
            }
        }
    }

    Ok(score)
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpack_object_header_basic() {
        // Minimal valid header: 23 bytes
        let data = vec![
            0x00, 0x02, // right = 2
            0x00, 0x01, // left = 1
            0x00, 0x03, // firstSubObj = 3
            0x00, 0x10, // xd = 16
            0x00, 0x20, // yd = 32
            0x02, // type = SYNC
            0xE0, // flags: selected=1, visible=1, soft=1, valid=0, tweaked=0, spare=0, filler=0
            0x00, 0x00, 0x01, 0x00, // objRect.top=0, left=256
            0x02, 0x00, 0x03, 0x00, // objRect.bottom=512, right=768
            0x00, // relSize = 0
            0x00, // filler = 0
            0x05, // nEntries = 5
        ];

        let hdr = unpack_object_header_n105(&data).unwrap();
        assert_eq!(hdr.right, 2);
        assert_eq!(hdr.left, 1);
        assert_eq!(hdr.first_sub_obj, 3);
        assert_eq!(hdr.xd, 16);
        assert_eq!(hdr.yd, 32);
        assert_eq!(hdr.obj_type, 2);
        assert!(hdr.selected);
        assert!(hdr.visible);
        assert!(hdr.soft);
        assert!(!hdr.valid);
        assert_eq!(hdr.n_entries, 5);
    }

    #[test]
    fn test_unpack_anote_minimal() {
        // Minimal ANOTE: 30 bytes (F#5 8th note from example in TN1)
        let data = vec![
            0x00, 0x00, // next = 0
            0x01, // staffn = 1
            0x05, // subType = EIGHTH_L_DUR
            0x40, // flags: selected=0, visible=1, soft=0, inChord=0, rest=0, unpitched=0, beamed=0, otherStemSide=0
            0xEC, // yqpit = -20 (0xEC as signed)
            0x00, 0x00, // xd = 0
            0x00, 0x00, // yd = 0
            0x01, 0x50, // ystem = 336
            0x00, 0x00, // playTimeDelta = 0
            0x00, 0xE4, // playDur = 228
            0x00, 0x00, // pTime = 0
            0x4E, // noteNum = 78 (F#5)
            0x4B, // onVelocity = 75
            0x40, // offVelocity = 64
            0x10, // tiedL=0, tiedR=0, ymovedots=1, ndots=0
            0x01, // voice = 1
            0x40, // rspIgnore=0, accident=4 (sharp), accSoft=0, playAsCue=0, micropitch=0
            0x28, // xmoveAcc=5, merged=0, courtesyAcc=0, doubleDur=0
            0x0B, // headShape=1 (NORMAL_VIS), xmovedots=3
            0x00, 0x00, // firstMod = 0
            0x00, // slurredL=0, slurredR=0, inTuplet=0, inOttava=0, small=0, tempFlag=0
            0x00, // fillerN = 0
        ];

        let note = unpack_anote_n105(&data).unwrap();
        assert_eq!(note.header.staffn, 1);
        assert_eq!(note.header.sub_type, 5); // EIGHTH_L_DUR
        assert_eq!(note.yqpit, -20);
        assert_eq!(note.note_num, 78); // F#5
        assert_eq!(note.on_velocity, 75);
        assert_eq!(note.accident, 4); // Sharp
        assert_eq!(note.voice, 1);
    }

    #[test]
    fn test_interpreted_score_get() {
        let mut score = InterpretedScore::new();

        // Add a dummy object at index 1
        let hdr = ObjectHeader {
            right: 2,
            left: 0,
            first_sub_obj: 0,
            xd: 0,
            yd: 0,
            obj_type: 0,
            selected: false,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ohdr_filler1: 0,
            obj_rect: Rect {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
            },
            rel_size: 0,
            ohdr_filler2: 0,
            n_entries: 0,
        };
        score.objects.push(InterpretedObject {
            index: 1,
            header: hdr.clone(),
            data: ObjData::Header(Header { header: hdr }),
        });

        // Test get() with valid link
        assert!(score.get(1).is_some());
        assert_eq!(score.get(1).unwrap().index, 1);

        // Test get() with NILINK
        assert!(score.get(NILINK).is_none());
        assert!(score.get(0).is_none());
    }
}
