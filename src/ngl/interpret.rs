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

use crate::basic_types::{
    DPoint, DRect, KsInfo, KsItem, Link, Point, Rect, ShortQd, ShortStd, NILINK,
};
use crate::obj_types::{
    AClef, AConnect, ADynamic, AGraphic, AKeySig, AMeasure, AModNr, ANote, ANoteBeam, ANoteOttava,
    ANoteTuple, APsMeas, ARptEnd, ASlur, AStaff, ATimeSig, BeamSet, Clef, Connect, Dynamic, Ending,
    GrSync, Graphic, Header, KeySig, Measure, ObjectHeader, Ottava, Page, PsMeas, RptEnd, Slur,
    Spacer, Staff, SubObjHeader, Sync, System, Tail, Tempo, TimeSig, Tuplet,
};
use std::collections::HashMap;

/// InterpretedScore: all decoded objects and subobjects from a .ngl file.
///
/// This struct holds the fully interpreted score data, organized by type for efficient access.
#[derive(Debug, Clone)]
pub struct InterpretedScore {
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

    /// Get an object by link (1-based index).
    ///
    /// Returns `None` if link is NILINK or out of range.
    pub fn get(&self, link: Link) -> Option<&InterpretedObject> {
        if link == NILINK || link == 0 {
            return None;
        }
        let idx = (link - 1) as usize;
        self.objects.get(idx)
    }

    /// Walk objects in linked-list order (following `right` links).
    ///
    /// Returns an iterator that traverses the object list from head to tail.
    pub fn walk(&self) -> impl Iterator<Item = &InterpretedObject> {
        ObjectWalker {
            score: self,
            current: self.objects.first().map(|obj| obj.header.right),
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
// N105 Unpacking Functions
// =============================================================================

/// Unpack N105 OBJECTHEADER_5 from raw bytes (23-byte common prefix).
///
/// Source: NObjTypesN105.h lines 12-27
///
/// **Bitfield unpacking for byte 11 (flags)**:
/// ```text
/// Bit 7 (MSB): selected
/// Bit 6:       visible
/// Bit 5:       soft
/// Bit 4:       valid
/// Bit 3:       tweaked
/// Bit 2:       spareFlag
/// Bits 1-0:    filler
/// ```
pub fn unpack_object_header_n105(data: &[u8]) -> Result<ObjectHeader, String> {
    if data.len() < 23 {
        return Err(format!("Object header too short: {} bytes", data.len()));
    }

    let right = u16::from_be_bytes([data[0], data[1]]);
    let left = u16::from_be_bytes([data[2], data[3]]);
    let first_sub_obj = u16::from_be_bytes([data[4], data[5]]);
    let xd = i16::from_be_bytes([data[6], data[7]]);
    let yd = i16::from_be_bytes([data[8], data[9]]);
    let obj_type = data[10] as i8;

    // Byte 11: bitfield flags (MSB-first)
    let flags = data[11];
    let selected = (flags & 0x80) != 0;
    let visible = (flags & 0x40) != 0;
    let soft = (flags & 0x20) != 0;
    let valid = (flags & 0x10) != 0;
    let tweaked = (flags & 0x08) != 0;
    let spare_flag = (flags & 0x04) != 0;
    let ohdr_filler1 = ((flags & 0x03) as i8) << 6 >> 6; // Sign-extend 2 bits

    // Bytes 12-19: objRect (4 x i16)
    let obj_rect = Rect {
        top: i16::from_be_bytes([data[12], data[13]]),
        left: i16::from_be_bytes([data[14], data[15]]),
        bottom: i16::from_be_bytes([data[16], data[17]]),
        right: i16::from_be_bytes([data[18], data[19]]),
    };

    let rel_size = data[20] as i8;
    let ohdr_filler2 = data[21] as i8;
    let n_entries = data[22];

    Ok(ObjectHeader {
        right,
        left,
        first_sub_obj,
        xd,
        yd,
        obj_type,
        selected,
        visible,
        soft,
        valid,
        tweaked,
        spare_flag,
        ohdr_filler1,
        obj_rect,
        rel_size,
        ohdr_filler2,
        n_entries,
    })
}

/// Unpack N105 SUBOBJHEADER_5 from raw bytes (5 bytes).
///
/// Source: NObjTypesN105.h lines 29-35
///
/// **Bitfield unpacking for byte 4 (flags)**:
/// ```text
/// Bit 7 (MSB): selected
/// Bit 6:       visible
/// Bit 5:       soft
/// Bits 4-0:    (varies by subobject type)
/// ```
pub fn unpack_subobj_header_n105(data: &[u8]) -> Result<SubObjHeader, String> {
    if data.len() < 5 {
        return Err(format!("Subobject header too short: {} bytes", data.len()));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);
    let staffn = data[2] as i8;
    let sub_type = data[3] as i8;

    // Byte 4: bitfield (first 3 bits are header, rest vary)
    let flags = data[4];
    let selected = (flags & 0x80) != 0;
    let visible = (flags & 0x40) != 0;
    let soft = (flags & 0x20) != 0;

    Ok(SubObjHeader {
        next,
        staffn,
        sub_type,
        selected,
        visible,
        soft,
    })
}

/// Unpack KsInfo from raw bytes (7 bytes for N105 WHOLE_KSINFO).
///
/// KsInfo stores a key signature as an array of KsItem structs.
/// The N105 binary format uses a compact representation.
///
/// Source: NBasicTypes.h lines 64-67, NObjTypesN105.h WHOLE_KSINFO
fn unpack_ksinfo_n105(data: &[u8], offset: usize) -> KsInfo {
    // Return empty key signature if not enough data
    if data.len() <= offset {
        return KsInfo {
            ks_item: [KsItem::default(); crate::basic_types::MAX_KSITEMS],
            n_ks_items: 0,
        };
    }

    let n_ks_items = data[offset] as i8;

    // In N105, key signature items are stored compactly.
    // For now, create a default KsInfo with the count.
    // Full implementation would unpack the individual KsItem structs.
    let mut ks_info = KsInfo {
        ks_item: [KsItem::default(); crate::basic_types::MAX_KSITEMS],
        n_ks_items,
    };

    // Unpack individual key signature items (simplified for now)
    // N105 stores them as a packed array, but we'll just read what we can
    // Limit to available bytes and MAX_KSITEMS
    // Ensure we never exceed the array bounds (0..MAX_KSITEMS-1)
    let max_items = n_ks_items
        .min((crate::basic_types::MAX_KSITEMS - 1) as i8)
        .min(((data.len().saturating_sub(offset + 1)) / 2) as i8)
        .max(0); // Ensure non-negative

    for i in 0..=max_items as usize {
        if i >= crate::basic_types::MAX_KSITEMS {
            break; // Safety check
        }
        let item_offset = offset + 1 + i * 2;
        if item_offset + 1 < data.len() {
            ks_info.ks_item[i] = KsItem {
                letcode: data[item_offset] as i8,
                sharp: data[item_offset + 1],
            };
        }
    }

    ks_info
}

/// Unpack N105 ANOTE_5 from raw bytes (30 bytes).
///
/// This is the most complex subobject, with extensive bitfield packing.
///
/// Source: NObjTypesN105.h lines 56-96
///
/// **Bitfield layout**:
/// - Byte 4 bits 4-0: inChord, rest, unpitched, beamed, otherStemSide
/// - Byte 21 bits 7-4: tiedL, tiedR, ymovedots (2 bits), ndots (4 bits)
/// - Byte 23 bits 7-0: rspIgnore, accident (3 bits), accSoft, playAsCue, micropitch (2 bits)
/// - Byte 24: xmoveAcc (5 bits), merged, courtesyAcc, doubleDur
/// - Byte 25 bits 7-3: headShape (5 bits), xmovedots (3 bits)
/// - Byte 28: slurredL (2), slurredR (2), inTuplet, inOttava, small, tempFlag
pub fn unpack_anote_n105(data: &[u8]) -> Result<ANote, String> {
    if data.len() < 30 {
        return Err(format!("ANOTE too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4 bits 4-0: inChord, rest, unpitched, beamed, otherStemSide
    let b4 = data[4];
    let in_chord = (b4 & 0x10) != 0;
    let rest = (b4 & 0x08) != 0;
    let unpitched = (b4 & 0x04) != 0;
    let beamed = (b4 & 0x02) != 0;
    let other_stem_side = (b4 & 0x01) != 0;

    let yqpit = data[5] as ShortQd;
    let xd = i16::from_be_bytes([data[6], data[7]]);
    let yd = i16::from_be_bytes([data[8], data[9]]);
    let ystem = i16::from_be_bytes([data[10], data[11]]);
    let play_time_delta = i16::from_be_bytes([data[12], data[13]]);
    let play_dur = i16::from_be_bytes([data[14], data[15]]);
    let p_time = i16::from_be_bytes([data[16], data[17]]);
    let note_num = data[18];
    let on_velocity = data[19];
    let off_velocity = data[20];

    // Byte 21: tiedL:1 | tiedR:1 | ymovedots:2 | ndots:4
    let b21 = data[21];
    let tied_l = (b21 & 0x80) != 0;
    let tied_r = (b21 & 0x40) != 0;
    let y_move_dots = (b21 >> 4) & 0x03;
    let ndots = b21 & 0x0F;

    let voice = data[22] as i8;

    // Byte 23: rspIgnore:1 | accident:3 | accSoft:1 | playAsCue:1 | micropitch:2
    let b23 = data[23];
    let rsp_ignore = (b23 >> 7) & 0x01;
    let accident = (b23 >> 4) & 0x07;
    let acc_soft = (b23 & 0x08) != 0;
    let play_as_cue = (b23 & 0x04) != 0;
    let micropitch = b23 & 0x03;

    // Byte 24: xmoveAcc:5 | merged:1 | courtesyAcc:1 | doubleDur:1
    let b24 = data[24];
    let xmove_acc = b24 >> 3;
    let merged = (b24 & 0x04) != 0;
    let courtesy_acc = if (b24 & 0x02) != 0 { 1 } else { 0 };
    let double_dur = if (b24 & 0x01) != 0 { 1 } else { 0 };

    // Byte 25: headShape:5 | xmovedots:3
    let b25 = data[25];
    let head_shape = b25 >> 3;
    let x_move_dots = b25 & 0x07;

    let first_mod = u16::from_be_bytes([data[26], data[27]]);

    // Byte 28: slurredL:2 | slurredR:2 | inTuplet:1 | inOttava:1 | small:1 | tempFlag:1
    let b28 = data[28];
    let slurred_l = (b28 & 0x80) != 0; // Only using 1 bit for now (extra bit reserved)
    let slurred_r = (b28 & 0x20) != 0; // Bit 5 (2-bit field at bits 5-4)
    let in_tuplet = (b28 & 0x08) != 0;
    let in_ottava = (b28 & 0x04) != 0;
    let small = (b28 & 0x02) != 0;
    let temp_flag = b28 & 0x01;

    let _filler_n = data[29] as i8;

    Ok(ANote {
        header,
        in_chord,
        rest,
        unpitched,
        beamed,
        other_stem_side,
        yqpit,
        xd,
        yd,
        ystem,
        play_time_delta,
        play_dur,
        p_time,
        note_num,
        on_velocity,
        off_velocity,
        tied_l,
        tied_r,
        x_move_dots,
        y_move_dots,
        ndots,
        voice,
        rsp_ignore,
        accident,
        acc_soft,
        courtesy_acc,
        xmove_acc,
        play_as_cue,
        micropitch,
        merged: if merged { 1 } else { 0 },
        double_dur: if double_dur == 1 { 1 } else { 0 },
        head_shape,
        first_mod,
        slurred_l,
        slurred_r,
        in_tuplet,
        in_ottava,
        small,
        temp_flag,
        art_harmonic: 0,
        user_id: 0,
        nh_segment: [0; 6],
        reserved_n: 0,
    })
}

/// Unpack N105 ANOTEBEAM_5 from raw bytes (6 bytes).
///
/// Source: NObjTypesN105.h lines 298-305
///
/// **Bitfield unpacking for byte 4**:
/// ```text
/// Bits 7-5:    fracs (3 bits)
/// Bit 4:       fracGoLeft
/// Bits 3-0:    filler
/// ```
pub fn unpack_anotebeam_n105(data: &[u8]) -> Result<ANoteBeam, String> {
    if data.len() < 6 {
        return Err(format!("ANOTEBEAM too short: {} bytes", data.len()));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);
    let bp_sync = u16::from_be_bytes([data[2], data[3]]);
    let startend = data[4] as i8; // Actually signed

    // Byte 5: fracs:3 | fracGoLeft:1 | filler:4
    let b5 = data[5];
    let fracs = (b5 >> 5) & 0x07;
    let frac_go_left = (b5 & 0x10) != 0;

    Ok(ANoteBeam {
        next,
        bp_sync,
        startend,
        fracs,
        frac_go_left: if frac_go_left { 1 } else { 0 },
        filler: 0,
    })
}

/// Unpack N105 ASLUR_5 from raw bytes (42 bytes).
///
/// Source: NObjTypesN105.h lines 456-470
///
/// **Bitfield unpacking for byte 4**:
/// ```text
/// Bit 7:       selected
/// Bit 6:       visible
/// Bit 5:       soft
/// Bits 4-3:    dashed (2 bits)
/// Bits 2-0:    filler
/// ```
pub fn unpack_aslur_n105(data: &[u8]) -> Result<ASlur, String> {
    if data.len() < 42 {
        return Err(format!("ASLUR too short: {} bytes", data.len()));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);

    // Byte 2-4: selected, visible, soft, dashed, filler
    let b2 = data[2];
    let selected = (b2 & 0x80) != 0;
    let visible = (b2 & 0x40) != 0;
    let soft = (b2 & 0x20) != 0;
    let dashed = (b2 & 0x18) != 0; // 2-bit field at bits 4-3
    let filler = (b2 & 0x07) != 0;

    // Bytes 3-10: bounds (Rect)
    let bounds = Rect {
        top: i16::from_be_bytes([data[3], data[4]]),
        left: i16::from_be_bytes([data[5], data[6]]),
        bottom: i16::from_be_bytes([data[7], data[8]]),
        right: i16::from_be_bytes([data[9], data[10]]),
    };

    let first_ind = data[11] as i8;
    let last_ind = data[12] as i8;
    let reserved = i32::from_be_bytes([data[13], data[14], data[15], data[16]]);

    // SplineSeg: 3 x DPoint (each 4 bytes) = 12 bytes
    let seg_knot = DPoint {
        v: i16::from_be_bytes([data[17], data[18]]),
        h: i16::from_be_bytes([data[19], data[20]]),
    };
    let seg_c0 = DPoint {
        v: i16::from_be_bytes([data[21], data[22]]),
        h: i16::from_be_bytes([data[23], data[24]]),
    };
    let seg_c1 = DPoint {
        v: i16::from_be_bytes([data[25], data[26]]),
        h: i16::from_be_bytes([data[27], data[28]]),
    };

    // Point startPt, endPt (each 4 bytes)
    let start_pt = Point {
        v: i16::from_be_bytes([data[29], data[30]]),
        h: i16::from_be_bytes([data[31], data[32]]),
    };
    let end_pt = Point {
        v: i16::from_be_bytes([data[33], data[34]]),
        h: i16::from_be_bytes([data[35], data[36]]),
    };

    // DPoint endKnot
    let end_knot = DPoint {
        v: i16::from_be_bytes([data[37], data[38]]),
        h: i16::from_be_bytes([data[39], data[40]]),
    };

    Ok(ASlur {
        next,
        selected,
        visible,
        soft,
        dashed,
        filler,
        bounds,
        first_ind,
        last_ind,
        reserved,
        seg: crate::obj_types::SplineSeg {
            knot: seg_knot,
            c0: seg_c0,
            c1: seg_c1,
        },
        start_pt,
        end_pt,
        end_knot,
    })
}

// Stub unpackers for other subobject types (to be fully implemented):

/// Unpack N105 ASTAFF_5 from raw bytes (50 bytes).
///
/// Source: NObjTypesN105.h lines 180-220
pub fn unpack_astaff_n105(data: &[u8]) -> Result<AStaff, String> {
    if data.len() < 50 {
        return Err(format!("ASTAFF too short: {} bytes", data.len()));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);
    let staffn = data[2] as i8;

    // Byte 3: selected:1 | visible:1 | filler:6
    let b3 = data[3];
    let selected = (b3 & 0x80) != 0;
    let visible = (b3 & 0x40) != 0;
    let filler_stf = (b3 & 0x20) != 0;

    let staff_top = i16::from_be_bytes([data[4], data[5]]);
    let staff_left = i16::from_be_bytes([data[6], data[7]]);
    let staff_right = i16::from_be_bytes([data[8], data[9]]);
    let staff_height = i16::from_be_bytes([data[10], data[11]]);
    let staff_lines = data[12] as i8;
    let font_size = i16::from_be_bytes([data[13], data[14]]);
    let flag_leading = i16::from_be_bytes([data[15], data[16]]);
    let min_stem_free = i16::from_be_bytes([data[17], data[18]]);
    let ledger_width = i16::from_be_bytes([data[19], data[20]]);
    let note_head_width = i16::from_be_bytes([data[21], data[22]]);
    let frac_beam_width = i16::from_be_bytes([data[23], data[24]]);
    let space_below = i16::from_be_bytes([data[25], data[26]]);
    let clef_type = data[27] as i8;
    let dynamic_type = data[28] as i8;

    // KsInfo: 7 bytes starting at offset 29
    let ks_info = unpack_ksinfo_n105(data, 29);

    let time_sig_type = data[36] as i8;
    let numerator = data[37] as i8;
    let denominator = data[38] as i8;
    let filler = data[39];

    // Byte 40: showLedgers:1 | showLines:7
    let b40 = data[40];
    let show_ledgers = (b40 & 0x80) != 0;
    let show_lines = b40 & 0x7F;

    Ok(AStaff {
        next,
        staffn,
        selected,
        visible,
        filler_stf,
        staff_top,
        staff_left,
        staff_right,
        staff_height,
        staff_lines,
        font_size,
        flag_leading,
        min_stem_free,
        ledger_width,
        note_head_width,
        frac_beam_width,
        space_below,
        clef_type,
        dynamic_type,
        ks_info,
        time_sig_type,
        numerator,
        denominator,
        filler,
        show_ledgers: if show_ledgers { 1 } else { 0 },
        show_lines,
    })
}

/// Unpack N105 AMEASURE_5 from raw bytes (40 bytes).
///
/// Source: NObjTypesN105.h lines 222-253
pub fn unpack_ameasure_n105(data: &[u8]) -> Result<AMeasure, String> {
    if data.len() < 40 {
        return Err(format!("AMEASURE too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4: measureVisible:1 | connAbove:1 | filler1:6
    let b4 = data[4];
    let measure_visible = (b4 & 0x80) != 0;
    let conn_above = (b4 & 0x40) != 0;
    let filler1 = b4 & 0x3F;

    let filler2 = data[5] as i8;
    let reserved_m = i16::from_be_bytes([data[6], data[7]]);
    let measure_num = i16::from_be_bytes([data[8], data[9]]);

    // DRect: 4 x i16
    let meas_size_rect = DRect {
        top: i16::from_be_bytes([data[10], data[11]]),
        left: i16::from_be_bytes([data[12], data[13]]),
        bottom: i16::from_be_bytes([data[14], data[15]]),
        right: i16::from_be_bytes([data[16], data[17]]),
    };

    let conn_staff = data[18] as i8;
    let clef_type = data[19] as i8;
    let dynamic_type = data[20] as i8;

    // KsInfo: 7 bytes starting at offset 21
    let ks_info = unpack_ksinfo_n105(data, 21);

    let time_sig_type = data[28] as i8;
    let numerator = data[29] as i8;
    let denominator = data[30] as i8;
    let x_mn_std_offset = i16::from_be_bytes([data[31], data[32]]) as ShortStd;
    let y_mn_std_offset = i16::from_be_bytes([data[33], data[34]]) as ShortStd;

    Ok(AMeasure {
        header,
        measure_visible,
        conn_above,
        filler1,
        filler2,
        reserved_m,
        measure_num,
        meas_size_rect,
        conn_staff,
        clef_type,
        dynamic_type,
        ks_info,
        time_sig_type,
        numerator,
        denominator,
        x_mn_std_offset,
        y_mn_std_offset,
    })
}

/// Unpack N105 ACLEF_5 from raw bytes (10 bytes).
///
/// Source: NObjTypesN105.h lines 255-266
pub fn unpack_aclef_n105(data: &[u8]) -> Result<AClef, String> {
    if data.len() < 10 {
        return Err(format!("ACLEF too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    let filler1 = data[4];
    let small = data[5];
    let filler2 = data[6];
    let xd = i16::from_be_bytes([data[6], data[7]]);
    let yd = i16::from_be_bytes([data[8], data[9]]);

    Ok(AClef {
        header,
        filler1,
        small,
        filler2,
        xd,
        yd,
    })
}

/// Unpack N105 AKEYSIG_5 from raw bytes (24 bytes).
///
/// Source: NObjTypesN105.h lines 268-293
pub fn unpack_akeysig_n105(data: &[u8]) -> Result<AKeySig, String> {
    if data.len() < 24 {
        return Err(format!("AKEYSIG too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4: nonstandard:1 | filler1:7
    let b4 = data[4];
    let nonstandard = (b4 & 0x80) != 0;
    let filler1 = b4 & 0x7F;

    let small = data[5];
    let filler2 = data[6] as i8;
    let xd = i16::from_be_bytes([data[7], data[8]]);

    // KsInfo: 7 bytes starting at offset 9
    let ks_info = unpack_ksinfo_n105(data, 9);

    Ok(AKeySig {
        header,
        nonstandard: if nonstandard { 1 } else { 0 },
        filler1,
        small,
        filler2,
        xd,
        ks_info,
    })
}

/// Unpack N105 ATIMESIG_5 from raw bytes (12 bytes).
///
/// Source: NObjTypesN105.h lines 295-308
pub fn unpack_atimesig_n105(data: &[u8]) -> Result<ATimeSig, String> {
    if data.len() < 12 {
        return Err(format!("ATIMESIG too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    let filler = data[4];
    let small = data[5];
    let conn_staff = data[6] as i8;
    let xd = i16::from_be_bytes([data[7], data[8]]);
    let yd = i16::from_be_bytes([data[9], data[10]]);
    let numerator = data[11] as i8;
    let denominator = if data.len() > 12 { data[12] as i8 } else { 4 };

    Ok(ATimeSig {
        header,
        filler,
        small,
        conn_staff,
        xd,
        yd,
        numerator,
        denominator,
    })
}

pub fn unpack_aconnect_n105(_data: &[u8]) -> Result<AConnect, String> {
    // TODO: Implement full ACONNECT_5 unpacking (12 bytes, bitfields in byte 2)
    Err("ACONNECT unpacking not yet implemented".to_string())
}

pub fn unpack_adynamic_n105(_data: &[u8]) -> Result<ADynamic, String> {
    // TODO: Implement full ADYNAMIC_5 unpacking (12 bytes, bitfields in bytes 4-5)
    Err("ADYNAMIC unpacking not yet implemented".to_string())
}

pub fn unpack_amodnr_n105(_data: &[u8]) -> Result<AModNr, String> {
    // TODO: Implement full AMODNR_5 unpacking (6 bytes, bitfields in byte 4)
    Err("AMODNR unpacking not yet implemented".to_string())
}

pub fn unpack_agraphic_n105(_data: &[u8]) -> Result<AGraphic, String> {
    // TODO: Implement full AGRAPHIC_5 unpacking (6 bytes)
    Err("AGRAPHIC unpacking not yet implemented".to_string())
}

pub fn unpack_anoteottava_n105(_data: &[u8]) -> Result<ANoteOttava, String> {
    // TODO: Implement full ANOTEOTTAVA_5 unpacking (4 bytes)
    Err("ANOTEOTTAVA unpacking not yet implemented".to_string())
}

pub fn unpack_anotetuple_n105(_data: &[u8]) -> Result<ANoteTuple, String> {
    // TODO: Implement full ANOTETUPLE_5 unpacking (4 bytes)
    Err("ANOTETUPLE unpacking not yet implemented".to_string())
}

pub fn unpack_arptend_n105(_data: &[u8]) -> Result<ARptEnd, String> {
    // TODO: Implement full ARPTEND_5 unpacking (6 bytes, bitfields in byte 4)
    Err("ARPTEND unpacking not yet implemented".to_string())
}

pub fn unpack_apsmeas_n105(_data: &[u8]) -> Result<APsMeas, String> {
    // TODO: Implement full APSMEAS_5 unpacking (6 bytes, bitfields in byte 4)
    Err("APSMEAS unpacking not yet implemented".to_string())
}

// =============================================================================
// Heap Interpretation
// =============================================================================

use crate::defs::*;
use crate::ngl::reader::NglFile;

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
