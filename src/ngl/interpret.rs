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

use crate::basic_types::{DPoint, Link, Point, Rect, ShortQd, NILINK};
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

pub fn unpack_astaff_n105(_data: &[u8]) -> Result<AStaff, String> {
    // TODO: Implement full ASTAFF_5 unpacking (50 bytes, bitfields in byte 3 and final byte)
    Err("ASTAFF unpacking not yet implemented".to_string())
}

pub fn unpack_ameasure_n105(_data: &[u8]) -> Result<AMeasure, String> {
    // TODO: Implement full AMEASURE_5 unpacking (40 bytes, bitfields in byte 4 and measure_num)
    Err("AMEASURE unpacking not yet implemented".to_string())
}

pub fn unpack_aclef_n105(_data: &[u8]) -> Result<AClef, String> {
    // TODO: Implement full ACLEF_5 unpacking (10 bytes, bitfields in byte 4)
    Err("ACLEF unpacking not yet implemented".to_string())
}

pub fn unpack_akeysig_n105(_data: &[u8]) -> Result<AKeySig, String> {
    // TODO: Implement full AKEYSIG_5 unpacking (24 bytes, bitfields in byte 4)
    Err("AKEYSIG unpacking not yet implemented".to_string())
}

pub fn unpack_atimesig_n105(_data: &[u8]) -> Result<ATimeSig, String> {
    // TODO: Implement full ATIMESIG_5 unpacking (12 bytes, bitfields in byte 4)
    Err("ATIMESIG unpacking not yet implemented".to_string())
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
