//! N105 subobject packing (serialization) for types 0-23.
//!
//! This module provides the inverse of unpack_*.rs: converting typed Rust structs
//! back to raw N105 binary format for NGL file writing.
//!
//! Each pack function:
//! - Takes a typed struct (e.g., &AStaff)
//! - Returns Vec<u8> with binary N105 format
//! - Uses big-endian byte ordering (PowerPC)
//! - Respects mac68k struct alignment (padding bytes where needed)
//!
//! Source: Inverse of unpack_*.rs and NObjTypesN105.h

use crate::basic_types::{KsInfo, Link};
use crate::obj_types::{
    AClef, AConnect, ADynamic, AGraphic, AKeySig, AMeasure, AModNr, ANote, ANoteBeam, ANoteOttava,
    ANoteTuple, APsMeas, ARptEnd, ASlur, AStaff, ATimeSig, PartInfo, SubObjHeader,
};
use std::collections::HashMap;

// =============================================================================
// Subobject LINK Mapping (HeapFileIO.cp WriteSubObjs equivalent)
// =============================================================================

/// SubobjLinkMap: Maps in-memory subobject LINK values to sequential file indices.
///
/// Unlike object LINKs (which use a single global namespace), subobject LINKs are
/// scoped per heap type. Each heap type (ASTAFF, AMEASURE, etc.) has its own
/// sequential numbering starting at 1.
///
/// This matches OG Nightingale's WriteSubObjs() function (HeapFileIO.cp:562-651),
/// which temporarily backpatches the `next` field before writing each subobject.
///
/// Source: OG HeapFileIO.cp lines 590-595 (nextL = link+1; *(LINK *)LinkToPtr = nextL++)
pub struct SubobjLinkMap {
    /// Map from in-memory Link to file index, per heap type
    map: HashMap<Link, Link>,
    /// Next available file index (incremented as subobjects are added)
    next_index: Link,
}

impl Default for SubobjLinkMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SubobjLinkMap {
    /// Create a new SubobjLinkMap, starting with index 1 (0 reserved for NILINK).
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
    pub fn convert(&self, in_memory_link: Link) -> Link {
        if in_memory_link == 0 {
            0
        } else {
            self.map.get(&in_memory_link).copied().unwrap_or(0)
        }
    }
}

/// Pack N105 ANOTE_5 to raw bytes (30 bytes total).
///
/// This is the most complex subobject, with extensive bitfield packing.
///
/// Layout (30 bytes):
/// ```text
/// 0-4   SUBOBJHEADER_5 (next, staffn, sub_type, flags)
/// 4     Bitfield: inChord|rest|unpitched|beamed|otherStemSide (bits 4-0)
/// 5     yqpit (ShortQd / i8)
/// 6-7   xd (DDIST, big-endian)
/// 8-9   yd (DDIST, big-endian)
/// 10-11 ystem (DDIST, big-endian)
/// 12-13 playTimeDelta (short, big-endian)
/// 14-15 playDur (short, big-endian)
/// 16-17 pTime (short, big-endian)
/// 18    noteNum (Byte)
/// 19    onVelocity (Byte)
/// 20    offVelocity (Byte)
/// 21    Bitfield: tiedL|tiedR|ymovedots(2)|ndots(4)
/// 22    voice (SignedByte)
/// 23    Bitfield: rspIgnore|accident(3)|accSoft|playAsCue|micropitch(2)
/// 24    Bitfield: xmoveAcc(5)|merged|courtesyAcc|doubleDur
/// 25    Bitfield: headShape(5)|xmovedots(3)
/// 26-27 firstMod (LINK, big-endian)
/// 28    Bitfield: slurredL(2)|slurredR(2)|inTuplet|inOttava|small|tempFlag
/// 29    fillerN (padding byte)
/// ```
///
/// Source: NObjTypesN105.h lines 56-96
pub fn pack_anote_n105(note: &ANote, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 30];

    // Offset 0-4: SUBOBJHEADER_5 (writes bytes 0-4)
    pack_subobj_header_n105(&note.header, link_map, &mut buf);

    // Byte 4: Overlay note flags onto header flags (bits 4-0)
    let mut b4 = buf[4]; // Keep selected|visible|soft from header (bits 7-5)
    if note.in_chord {
        b4 |= 0x10; // bit 4
    }
    if note.rest {
        b4 |= 0x08; // bit 3
    }
    if note.unpitched {
        b4 |= 0x04; // bit 2
    }
    if note.beamed {
        b4 |= 0x02; // bit 1
    }
    if note.other_stem_side {
        b4 |= 0x01; // bit 0
    }
    buf[4] = b4;

    // Byte 5: yqpit (ShortQd / i8)
    buf[5] = note.yqpit as u8;

    // Bytes 6-17: DDIST and timing fields (all big-endian i16)
    buf[6..8].copy_from_slice(&note.xd.to_be_bytes());
    buf[8..10].copy_from_slice(&note.yd.to_be_bytes());
    buf[10..12].copy_from_slice(&note.ystem.to_be_bytes());
    buf[12..14].copy_from_slice(&note.play_time_delta.to_be_bytes());
    buf[14..16].copy_from_slice(&note.play_dur.to_be_bytes());
    buf[16..18].copy_from_slice(&note.p_time.to_be_bytes());

    // Bytes 18-20: noteNum, velocities
    buf[18] = note.note_num;
    buf[19] = note.on_velocity;
    buf[20] = note.off_velocity;

    // Byte 21: tiedL(1)|tiedR(1)|ymovedots(2)|ndots(4)
    let mut b21: u8 = 0;
    if note.tied_l {
        b21 |= 0x80; // bit 7
    }
    if note.tied_r {
        b21 |= 0x40; // bit 6
    }
    b21 |= (note.y_move_dots & 0x03) << 4; // bits 5-4
    b21 |= note.ndots & 0x0F; // bits 3-0
    buf[21] = b21;

    // Byte 22: voice (i8)
    buf[22] = note.voice as u8;

    // Byte 23: rspIgnore(1)|accident(3)|accSoft(1)|playAsCue(1)|micropitch(2)
    let mut b23: u8 = 0;
    b23 |= (note.rsp_ignore & 0x01) << 7; // bit 7
    b23 |= (note.accident & 0x07) << 4; // bits 6-4
    if note.acc_soft {
        b23 |= 0x08; // bit 3
    }
    if note.play_as_cue {
        b23 |= 0x04; // bit 2
    }
    b23 |= note.micropitch & 0x03; // bits 1-0
    buf[23] = b23;

    // Byte 24: xmoveAcc(5)|merged(1)|courtesyAcc(1)|doubleDur(1)
    let mut b24: u8 = 0;
    b24 |= (note.xmove_acc & 0x1F) << 3; // bits 7-3
    if note.merged != 0 {
        b24 |= 0x04; // bit 2
    }
    if note.courtesy_acc != 0 {
        b24 |= 0x02; // bit 1
    }
    if note.double_dur != 0 {
        b24 |= 0x01; // bit 0
    }
    buf[24] = b24;

    // Byte 25: headShape(5)|xmovedots(3)
    let mut b25: u8 = 0;
    b25 |= (note.head_shape & 0x1F) << 3; // bits 7-3
    b25 |= note.x_move_dots & 0x07; // bits 2-0
    buf[25] = b25;

    // Bytes 26-27: firstMod (LINK, big-endian)
    buf[26..28].copy_from_slice(&note.first_mod.to_be_bytes());

    // Byte 28: slurredL(2)|slurredR(2)|inTuplet(1)|inOttava(1)|small(1)|tempFlag(1)
    let mut b28: u8 = 0;
    if note.slurred_l {
        b28 |= 0x80; // bit 7 (2-bit field, using only bit 7 for now)
    }
    if note.slurred_r {
        b28 |= 0x20; // bit 5 (2-bit field at bits 5-4, using only bit 5)
    }
    if note.in_tuplet {
        b28 |= 0x08; // bit 3
    }
    if note.in_ottava {
        b28 |= 0x04; // bit 2
    }
    if note.small {
        b28 |= 0x02; // bit 1
    }
    b28 |= note.temp_flag & 0x01; // bit 0
    buf[28] = b28;

    // Byte 29: fillerN (padding, leave as 0)

    buf
}

/// Pack N105 ACLEF_5 to raw bytes (10 bytes total).
///
/// Layout:
/// ```text
/// 0-4   SUBOBJHEADER_5 (next, staffn, sub_type, flags)
/// 4     Bitfield: selected|visible|soft|filler1(3)|small(2)
/// 5     filler2
/// 6-7   xd (DDIST, big-endian)
/// 8-9   yd (DDIST, big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 227-236
pub fn pack_aclef_n105(clef: &AClef, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 10];

    // Offset 0-4: SUBOBJHEADER_5
    pack_subobj_header_n105(&clef.header, link_map, &mut buf);

    // Byte 4: Overlay filler1 and small onto header flags
    let mut b4 = buf[4]; // Keep selected|visible|soft from header (bits 7-5)
    b4 |= (clef.filler1 & 0x07) << 2; // bits 4-2
    b4 |= clef.small & 0x03; // bits 1-0
    buf[4] = b4;

    // Byte 5: filler2
    buf[5] = clef.filler2;

    // Bytes 6-9: xd, yd (DDIST, big-endian)
    buf[6..8].copy_from_slice(&clef.xd.to_be_bytes());
    buf[8..10].copy_from_slice(&clef.yd.to_be_bytes());

    buf
}

/// Pack N105 AKEYSIG_5 to raw bytes (24 bytes total).
///
/// Layout:
/// ```text
/// 0-4   SUBOBJHEADER_5 (next, staffn, sub_type, flags)
/// 4     Bitfield: selected|visible|soft|nonstandard|filler1(2)|small(2)
/// 5     filler2 (SignedByte)
/// 6-7   xd (DDIST, big-endian)
/// 8-22  WHOLE_KSINFO_5 (15 bytes)
/// 23    filler3
/// ```
///
/// Source: NObjTypesN105.h lines 248-272
pub fn pack_akeysig_n105(keysig: &AKeySig, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 24];

    // Offset 0-4: SUBOBJHEADER_5
    pack_subobj_header_n105(&keysig.header, link_map, &mut buf);

    // Byte 4: Overlay nonstandard, filler1, small onto header flags
    let mut b4 = buf[4]; // Keep selected|visible|soft from header (bits 7-5)
    b4 |= (keysig.nonstandard & 0x01) << 4; // bit 4
    b4 |= (keysig.filler1 & 0x03) << 2; // bits 3-2
    b4 |= keysig.small & 0x03; // bits 1-0
    buf[4] = b4;

    // Byte 5: filler2 (i8)
    buf[5] = keysig.filler2 as u8;

    // Bytes 6-7: xd (DDIST, big-endian)
    buf[6..8].copy_from_slice(&keysig.xd.to_be_bytes());

    // Bytes 8-22: WHOLE_KSINFO_5 (15 bytes)
    pack_ksinfo_n105(&keysig.ks_info, &mut buf, 8);

    // Byte 23: padding (leave as 0)

    buf
}

/// Pack N105 ATIMESIG_5 to raw bytes (12 bytes total).
///
/// Layout:
/// ```text
/// 0-4   SUBOBJHEADER_5 (next, staffn, sub_type, flags)
/// 4     Bitfield: selected|visible|soft|filler(3)|small(2)
/// 5     connStaff (SignedByte)
/// 6-7   xd (DDIST, big-endian)
/// 8-9   yd (DDIST, big-endian)
/// 10    numerator (SignedByte)
/// 11    denominator (SignedByte)
/// ```
///
/// Source: NObjTypesN105.h lines 283-293
pub fn pack_atimesig_n105(timesig: &ATimeSig, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 12];

    // Offset 0-4: SUBOBJHEADER_5
    pack_subobj_header_n105(&timesig.header, link_map, &mut buf);

    // Byte 4: Overlay filler and small onto header flags
    let mut b4 = buf[4]; // Keep selected|visible|soft from header (bits 7-5)
    b4 |= (timesig.filler & 0x07) << 2; // bits 4-2
    b4 |= timesig.small & 0x03; // bits 1-0
    buf[4] = b4;

    // Byte 5: connStaff (i8)
    buf[5] = timesig.conn_staff as u8;

    // Bytes 6-9: xd, yd (DDIST, big-endian)
    buf[6..8].copy_from_slice(&timesig.xd.to_be_bytes());
    buf[8..10].copy_from_slice(&timesig.yd.to_be_bytes());

    // Bytes 10-11: numerator, denominator (i8)
    buf[10] = timesig.numerator as u8;
    buf[11] = timesig.denominator as u8;

    buf
}

/// Pack N105 ASTAFF_5 to raw bytes (50 bytes total).
///
/// On-disk layout with mac68k alignment (50 bytes):
/// ```text
/// Offset  Size  Field
/// 0       2     next (LINK)
/// 2       1     staffn
/// 3       1     selected:1+visible:1+fillerStf:6
/// 4       2     staffTop (DDIST)
/// 6       2     staffLeft (DDIST)
/// 8       2     staffRight (DDIST)
/// 10      2     staffHeight (DDIST)
/// 12      1     staffLines
/// 13      1     [PADDING — align fontSize]
/// 14      2     fontSize (short)
/// 16      2     flagLeading (DDIST)
/// 18      2     minStemFree (DDIST)
/// 20      2     ledgerWidth (DDIST)
/// 22      2     noteHeadWidth (DDIST)
/// 24      2     fracBeamWidth (DDIST)
/// 26      2     spaceBelow (DDIST)
/// 28      1     clefType
/// 29      1     dynamicType
/// 30      14    KSItem[0..6] (7 x 2 bytes each)
/// 44      1     nKSItems
/// 45      1     timeSigType
/// 46      1     numerator
/// 47      1     denominator
/// 48      1     filler:3+showLedgers:1+showLines:4
/// 49      1     [PADDING — struct aligned to 2-byte boundary]
/// ```
///
/// Source: NObjTypesN105.h lines 152-180
pub fn pack_astaff_n105(staff: &AStaff, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 50];

    // Offset 0-1: next (LINK, big-endian)
    // Convert in-memory LINK to sequential file index before packing
    let file_index = link_map.convert(staff.next);
    buf[0..2].copy_from_slice(&file_index.to_be_bytes());

    // Offset 2: staffn
    buf[2] = staff.staffn as u8;

    // Offset 3: Bitfield selected:1 | visible:1 | fillerStf:6
    let mut b3: u8 = 0;
    if staff.selected {
        b3 |= 0x80;
    }
    if staff.visible {
        b3 |= 0x40;
    }
    if staff.filler_stf {
        b3 |= 0x20;
    }
    buf[3] = b3;

    // Offsets 4-11: DDIST coordinates (big-endian i16 pairs)
    buf[4..6].copy_from_slice(&staff.staff_top.to_be_bytes());
    buf[6..8].copy_from_slice(&staff.staff_left.to_be_bytes());
    buf[8..10].copy_from_slice(&staff.staff_right.to_be_bytes());
    buf[10..12].copy_from_slice(&staff.staff_height.to_be_bytes());

    // Offset 12: staffLines
    buf[12] = staff.staff_lines as u8;
    // Offset 13: padding (already 0)

    // Offset 14-15: fontSize (big-endian i16)
    buf[14..16].copy_from_slice(&staff.font_size.to_be_bytes());

    // Offset 16-27: More DDIST pairs
    buf[16..18].copy_from_slice(&staff.flag_leading.to_be_bytes());
    buf[18..20].copy_from_slice(&staff.min_stem_free.to_be_bytes());
    buf[20..22].copy_from_slice(&staff.ledger_width.to_be_bytes());
    buf[22..24].copy_from_slice(&staff.note_head_width.to_be_bytes());
    buf[24..26].copy_from_slice(&staff.frac_beam_width.to_be_bytes());
    buf[26..28].copy_from_slice(&staff.space_below.to_be_bytes());

    // Offset 28-29: clefType, dynamicType
    buf[28] = staff.clef_type as u8;
    buf[29] = staff.dynamic_type as u8;

    // Offset 30-44: WHOLE_KSINFO_5 (KsInfo packing)
    pack_ksinfo_n105(&staff.ks_info, &mut buf, 30);

    // Offset 45-47: timeSigType, numerator, denominator
    buf[45] = staff.time_sig_type as u8;
    buf[46] = staff.numerator as u8;
    buf[47] = staff.denominator as u8;

    // Offset 48: filler:3 (bits 7-5) | showLedgers:1 (bit 4) | showLines:4 (bits 3-0)
    let mut b48: u8 = 0;
    b48 |= (staff.filler & 0x07) << 5;
    b48 |= (staff.show_ledgers & 0x01) << 4;
    b48 |= staff.show_lines & 0x0F;
    buf[48] = b48;

    // Offset 49: trailing struct padding (already 0)

    buf
}

/// Pack N105 AMEASURE_5 to raw bytes (40 bytes total).
///
/// On-disk layout (mac68k alignment):
/// ```text
/// 0-3   SUBOBJHEADER_5 (next, staffn, sub_type, flags byte)
/// 4     measureVisible:1 | connAbove:1 | filler1:3 | unused:3
/// 5     filler2 (SignedByte)
/// 6-7   reserved_m (short, includes oldFakeMeas in high bit)
/// 8-15  measSizeRect (DRect = 4 x DDIST)
/// 16    connStaff (SignedByte)
/// 17    clefType (SignedByte)
/// 18    dynamicType (SignedByte)
/// 19    [PADDING — align KSITEM_5 struct to 2-byte boundary]
/// 20-34 WHOLE_KSINFO_5 (15 bytes)
/// 35    timeSigType
/// 36    numerator
/// 37    denominator
/// 38    xMNStdOffset (SHORTSTD)
/// 39    yMNStdOffset (SHORTSTD)
/// ```
///
/// Source: NObjTypesN105.h lines 192-210
pub fn pack_ameasure_n105(measure: &AMeasure, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 40];

    // Offset 0-3: SUBOBJHEADER_5
    pack_subobj_header_n105(&measure.header, link_map, &mut buf);

    // Offset 4: measureVisible:1 | connAbove:1 | filler1:3
    // (high 3 bits of this are unused, low 5 bits carry subobj flags from header byte 4)
    let mut b4 = buf[4]; // Keep the header flags from pack_subobj_header
    if measure.measure_visible {
        b4 |= 0x10; // bit 4
    }
    if measure.conn_above {
        b4 |= 0x08; // bit 3
    }
    b4 |= measure.filler1 & 0x07; // bits 2-0
    buf[4] = b4;

    // Offset 5: filler2
    buf[5] = measure.filler2 as u8;

    // Offset 6-7: reserved_m (short, big-endian)
    buf[6..8].copy_from_slice(&measure.reserved_m.to_be_bytes());

    // Offset 8-15: measSizeRect (DRect: 4 x DDIST, big-endian)
    buf[8..10].copy_from_slice(&measure.meas_size_rect.top.to_be_bytes());
    buf[10..12].copy_from_slice(&measure.meas_size_rect.left.to_be_bytes());
    buf[12..14].copy_from_slice(&measure.meas_size_rect.bottom.to_be_bytes());
    buf[14..16].copy_from_slice(&measure.meas_size_rect.right.to_be_bytes());

    // Offset 16-18: connStaff, clefType, dynamicType
    buf[16] = measure.conn_staff as u8;
    buf[17] = measure.clef_type as u8;
    buf[18] = measure.dynamic_type as u8;

    // Offset 19: padding (already 0)

    // Offset 20-34: WHOLE_KSINFO_5 (15 bytes)
    pack_ksinfo_n105(&measure.ks_info, &mut buf, 20);

    // Offset 35-37: timeSigType, numerator, denominator
    buf[35] = measure.time_sig_type as u8;
    buf[36] = measure.numerator as u8;
    buf[37] = measure.denominator as u8;

    // Offset 38-39: xMNStdOffset, yMNStdOffset (SHORTSTD = SignedByte)
    buf[38] = measure.x_mn_std_offset as u8;
    buf[39] = measure.y_mn_std_offset as u8;

    buf
}

/// Pack SUBOBJHEADER_5 to raw bytes at buffer offset 0-3 (4 bytes total).
///
/// Bitfield for byte 4 (flags):
/// ```text
/// Bit 7 (MSB): selected
/// Bit 6:       visible
/// Bit 5:       soft
/// Bits 4-0:    (vary by subobject type)
/// ```
///
/// Source: NObjTypesN105.h lines 29-35
fn pack_subobj_header_n105(header: &SubObjHeader, link_map: &SubobjLinkMap, buf: &mut [u8]) {
    // Offset 0-1: next (LINK, big-endian)
    // Convert in-memory LINK to sequential file index before packing
    let file_index = link_map.convert(header.next);
    buf[0..2].copy_from_slice(&file_index.to_be_bytes());

    // Offset 2: staffn
    buf[2] = header.staffn as u8;

    // Offset 3: sub_type
    buf[3] = header.sub_type as u8;

    // Offset 4: Bitfield selected:1 | visible:1 | soft:1 | (rest varies)
    let mut b4: u8 = 0;
    if header.selected {
        b4 |= 0x80; // bit 7
    }
    if header.visible {
        b4 |= 0x40; // bit 6
    }
    if header.soft {
        b4 |= 0x20; // bit 5
    }
    buf[4] = b4;
}

/// Pack WHOLE_KSINFO_5 to raw bytes at specified buffer offset (15 bytes total).
///
/// Layout:
/// - Offsets +0..+14: KSItem[7] array, each item 2 bytes (data byte + mac68k padding)
/// - Offset +14: nKSItems
///
/// KSITEM_5 bitfield on 68k/PPC (MSB-first):
/// ```text
/// bit 7-1: letcode (7 bits)
/// bit 0:   sharp (1 bit)
/// byte 1:  padding (mac68k alignment)
/// ```
///
/// Source: NObjTypesN105.h lines 37-41
fn pack_ksinfo_n105(ks_info: &KsInfo, buf: &mut [u8], offset: usize) {
    // Pack KSItem array (7 items x 2 bytes each = 14 bytes)
    for i in 0..crate::basic_types::MAX_KSITEMS {
        let item_offset = offset + i * 2;
        let item = &ks_info.ks_item[i];

        // Byte 0: letcode (7 bits, MSB) | sharp (1 bit, LSB)
        let letcode_bits = ((item.letcode as u8) & 0x7F) << 1;
        let sharp_bit = item.sharp & 1;
        buf[item_offset] = letcode_bits | sharp_bit;

        // Byte 1: padding (already 0)
    }

    // nKSItems at offset+14
    buf[offset + 14] = ks_info.n_ks_items as u8;
}

// =============================================================================
// Additional N105 Subobject Packers
// =============================================================================

/// Pack N105 ANOTEBEAM_5 to raw bytes (6 bytes total).
///
/// Layout:
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2-3   bpSync (LINK, big-endian)
/// 4     startend (SignedByte)
/// 5     Bitfield: fracs(3)|fracGoLeft(1)|filler(4)
/// ```
///
/// Source: NObjTypesN105.h lines 297-305
pub fn pack_anotebeam_n105(notebeam: &ANoteBeam) -> Vec<u8> {
    let mut buf = vec![0u8; 6];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&notebeam.next.to_be_bytes());

    // Bytes 2-3: bpSync (LINK, big-endian)
    buf[2..4].copy_from_slice(&notebeam.bp_sync.to_be_bytes());

    // Byte 4: startend (SignedByte)
    buf[4] = notebeam.startend as u8;

    // Byte 5: fracs(3)|fracGoLeft(1)|filler(4)
    let mut b5: u8 = 0;
    b5 |= (notebeam.fracs & 0x07) << 5; // bits 7-5
    b5 |= (notebeam.frac_go_left & 0x01) << 4; // bit 4
    b5 |= notebeam.filler & 0x0F; // bits 3-0
    buf[5] = b5;

    buf
}

/// Pack N105 ANOTETUPLE_5 to raw bytes (4 bytes total).
///
/// Layout:
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2-3   tpSync (LINK, big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 547-551
pub fn pack_anotetuple_n105(tuplet: &ANoteTuple) -> Vec<u8> {
    let mut buf = vec![0u8; 4];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&tuplet.next.to_be_bytes());

    // Bytes 2-3: tpSync (LINK, big-endian)
    buf[2..4].copy_from_slice(&tuplet.tp_sync.to_be_bytes());

    buf
}

/// Pack N105 ADYNAMIC_5 to raw bytes (14 bytes total).
///
/// Layout:
/// ```text
/// 0-4   SUBOBJHEADER_5 (5 bytes)
/// 4     Bitfield (overlaps SUBOBJ byte 4): selected:1|visible:1|soft:1|mouthWidth:5
/// 5     small:2|otherWidth:6
/// 6-7   xd (DDIST, big-endian)
/// 8-9   yd (DDIST, big-endian)
/// 10-11 endxd (DDIST, big-endian)
/// 12-13 endyd (DDIST, big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 357-369
pub fn pack_adynamic_n105(dynamic: &ADynamic, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 14];

    // Bytes 0-3: SUBOBJHEADER_5 prefix (next, staffn, subType)
    // Convert in-memory LINK to sequential file index before packing
    let file_index = link_map.convert(dynamic.header.next);
    buf[0..2].copy_from_slice(&file_index.to_be_bytes());
    buf[2] = dynamic.header.staffn as u8;
    buf[3] = dynamic.header.sub_type as u8;

    // Byte 4: selected:1|visible:1|soft:1|mouthWidth:5
    let mut b4: u8 = 0;
    if dynamic.header.selected {
        b4 |= 0x80; // bit 7
    }
    if dynamic.header.visible {
        b4 |= 0x40; // bit 6
    }
    if dynamic.header.soft {
        b4 |= 0x20; // bit 5
    }
    b4 |= dynamic.mouth_width & 0x1F; // bits 4-0
    buf[4] = b4;

    // Byte 5: small:2|otherWidth:6
    let b5 = ((dynamic.small & 0x03) << 6) | (dynamic.other_width & 0x3F);
    buf[5] = b5;

    // Bytes 6-13: xd, yd, endxd, endyd (DDIST, big-endian)
    buf[6..8].copy_from_slice(&dynamic.xd.to_be_bytes());
    buf[8..10].copy_from_slice(&dynamic.yd.to_be_bytes());
    buf[10..12].copy_from_slice(&dynamic.endxd.to_be_bytes());
    buf[12..14].copy_from_slice(&dynamic.endyd.to_be_bytes());

    buf
}

/// Pack N105 ACONNECT_5 to raw bytes (12 bytes total).
///
/// Layout (matching unpack_aconnect_n105):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2     selected:1|filler:1|connLevel:3|connectType:2
/// 3     staffAbove (SignedByte)
/// 4     staffBelow (SignedByte)
/// 5     [PADDING]
/// 6-7   xd (DDIST, big-endian)
/// 8-9   firstPart (LINK, big-endian)
/// 10-11 lastPart (LINK, big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 338-349, unpack_stubs.rs:111-151
pub fn pack_aconnect_n105(connect: &AConnect) -> Vec<u8> {
    let mut buf = vec![0u8; 12];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&connect.next.to_be_bytes());

    // Byte 2: selected:1|filler:1|connLevel:3|connectType:2
    let mut b2: u8 = 0;
    if connect.selected {
        b2 |= 0x80; // bit 7
    }
    b2 |= (connect.filler & 0x01) << 6; // bit 6
    b2 |= (connect.conn_level & 0x07) << 3; // bits 5-3
    b2 |= (connect.connect_type & 0x03) << 1; // bits 2-1
    buf[2] = b2;

    // Bytes 3-4: staffAbove, staffBelow
    buf[3] = connect.staff_above as u8;
    buf[4] = connect.staff_below as u8;

    // Byte 5: padding (already 0)

    // Bytes 6-7: xd (DDIST, big-endian)
    buf[6..8].copy_from_slice(&connect.xd.to_be_bytes());

    // Bytes 8-9: firstPart (LINK, big-endian)
    buf[8..10].copy_from_slice(&connect.first_part.to_be_bytes());

    // Bytes 10-11: lastPart (LINK, big-endian)
    buf[10..12].copy_from_slice(&connect.last_part.to_be_bytes());

    buf
}

/// Pack N105 AMODNR_5 to raw bytes (6 bytes total).
///
/// Layout (matching unpack_amodnr_n105):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2     selected:1|visible:1|soft:1|xstd:5
/// 3     modCode (Byte)
/// 4     data (SignedByte)
/// 5     ystdpit (SHORTSTD = SignedByte)
/// ```
///
/// Source: NObjTypesN105.h lines 382-391, unpack_stubs.rs:245-276
pub fn pack_amodnr_n105(modnr: &AModNr) -> Vec<u8> {
    let mut buf = vec![0u8; 6];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&modnr.next.to_be_bytes());

    // Byte 2: selected:1|visible:1|soft:1|xstd:5
    let mut b2: u8 = 0;
    if modnr.selected {
        b2 |= 0x80; // bit 7
    }
    if modnr.visible {
        b2 |= 0x40; // bit 6
    }
    if modnr.soft {
        b2 |= 0x20; // bit 5
    }
    b2 |= modnr.xstd & 0x1F; // bits 4-0
    buf[2] = b2;

    // Byte 3: modCode
    buf[3] = modnr.mod_code;

    // Byte 4: data
    buf[4] = modnr.data as u8;

    // Byte 5: ystdpit (SHORTSTD = SignedByte)
    buf[5] = modnr.ystdpit as u8;

    buf
}

/// Pack N105 AGRAPHIC_5 to raw bytes (6 bytes total).
///
/// Layout (matching unpack_agraphic_n105):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2-5   strOffset (STRINGOFFSET = long, i32 big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 396-399, unpack_stubs.rs:285-295
pub fn pack_agraphic_n105(graphic: &AGraphic) -> Vec<u8> {
    let mut buf = vec![0u8; 6];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&graphic.next.to_be_bytes());

    // Bytes 2-5: strOffset (i32, big-endian)
    buf[2..6].copy_from_slice(&graphic.str_offset.to_be_bytes());

    buf
}

/// Pack N105 ANOTEOTTAVA_5 to raw bytes (4 bytes total).
///
/// Layout:
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2-3   opSync (LINK, big-endian)
/// ```
///
/// Source: NObjTypesN105.h lines 461-465
pub fn pack_anoteottava_n105(ottava: &ANoteOttava) -> Vec<u8> {
    let mut buf = vec![0u8; 4];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&ottava.next.to_be_bytes());

    // Bytes 2-3: opSync (LINK, big-endian)
    buf[2..4].copy_from_slice(&ottava.op_sync.to_be_bytes());

    buf
}

/// Pack N105 ASLUR_5 to raw bytes (42 bytes total).
///
/// Layout (matching unpack_aslur_n105):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2     selected:1|visible:1|soft:1|dashed:2|filler:3
/// 3     filler (SignedByte, mac68k padding)
/// 4-11  bounds (Rect: 4 x i16, big-endian)
/// 12    firstInd (SignedByte)
/// 13    lastInd (SignedByte)
/// 14-17 reserved (long, i32 big-endian)
/// 18-29 seg (SplineSeg: 3 x DPoint, 12 bytes)
/// 30-33 startPt (Point: 2 x i16, big-endian)
/// 34-37 endPt (Point: 2 x i16, big-endian)
/// 38-41 endKnot (DPoint: 2 x i16, big-endian)
/// ```
///
/// Source: NObjTypesN105.h, unpack_slur.rs:41-128
pub fn pack_aslur_n105(slur: &ASlur) -> Vec<u8> {
    let mut buf = vec![0u8; 42];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&slur.next.to_be_bytes());

    // Byte 2: selected:1|visible:1|soft:1|dashed:2|filler:3
    let mut b2: u8 = 0;
    if slur.selected {
        b2 |= 0x80; // bit 7
    }
    if slur.visible {
        b2 |= 0x40; // bit 6
    }
    if slur.soft {
        b2 |= 0x20; // bit 5
    }
    if slur.dashed {
        b2 |= 0x18; // bits 4-3 (2-bit field)
    }
    // filler occupies bits 2-0, but stored as bool (unpack treats any non-zero as true)
    // Leave bits 2-0 as 0 for now (filler is just padding)
    buf[2] = b2;

    // Byte 3: filler (i8, padding)
    buf[3] = 0;

    // Bytes 4-11: bounds (Rect: 4 x i16, big-endian)
    buf[4..6].copy_from_slice(&slur.bounds.top.to_be_bytes());
    buf[6..8].copy_from_slice(&slur.bounds.left.to_be_bytes());
    buf[8..10].copy_from_slice(&slur.bounds.bottom.to_be_bytes());
    buf[10..12].copy_from_slice(&slur.bounds.right.to_be_bytes());

    // Bytes 12-13: firstInd, lastInd (i8)
    buf[12] = slur.first_ind as u8;
    buf[13] = slur.last_ind as u8;

    // Bytes 14-17: reserved (i32, big-endian)
    buf[14..18].copy_from_slice(&slur.reserved.to_be_bytes());

    // Bytes 18-29: seg (SplineSeg: 3 x DPoint)
    // seg.knot (DPoint: v, h)
    buf[18..20].copy_from_slice(&slur.seg.knot.v.to_be_bytes());
    buf[20..22].copy_from_slice(&slur.seg.knot.h.to_be_bytes());
    // seg.c0 (DPoint: v, h)
    buf[22..24].copy_from_slice(&slur.seg.c0.v.to_be_bytes());
    buf[24..26].copy_from_slice(&slur.seg.c0.h.to_be_bytes());
    // seg.c1 (DPoint: v, h)
    buf[26..28].copy_from_slice(&slur.seg.c1.v.to_be_bytes());
    buf[28..30].copy_from_slice(&slur.seg.c1.h.to_be_bytes());

    // Bytes 30-33: startPt (Point: v, h)
    buf[30..32].copy_from_slice(&slur.start_pt.v.to_be_bytes());
    buf[32..34].copy_from_slice(&slur.start_pt.h.to_be_bytes());

    // Bytes 34-37: endPt (Point: v, h)
    buf[34..36].copy_from_slice(&slur.end_pt.v.to_be_bytes());
    buf[36..38].copy_from_slice(&slur.end_pt.h.to_be_bytes());

    // Bytes 38-41: endKnot (DPoint: v, h)
    buf[38..40].copy_from_slice(&slur.end_knot.v.to_be_bytes());
    buf[40..42].copy_from_slice(&slur.end_knot.h.to_be_bytes());

    buf
}

/// Pack N105 PARTINFO to raw bytes (64 bytes minimum).
///
/// Layout (matching unpack_partinfo):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2     partVelocity (SignedByte)
/// 3     firstStaff (SignedByte)
/// 4     patchNum (Byte)
/// 5     lastStaff (SignedByte)
/// 6     channel (Byte)
/// 7     transpose (SignedByte)
/// 8-9   loKeyNum (short, big-endian)
/// 10-11 hiKeyNum (short, big-endian)
/// 12-43 name[32] (C string)
/// 44-55 shortName[12] (C string)
/// 56    hiKeyName
/// 57    hiKeyAcc
/// 58    tranName
/// 59    tranAcc
/// 60    loKeyName
/// 61    loKeyAcc
/// 62    bankNumber0
/// 63    bankNumber32
/// 64-65 fmsOutputDevice (short, big-endian)
/// 66+   fmsOutputDestination[280] (obsolete, write zeros)
/// ```
///
/// Source: NBasicTypes.h:171-201, unpack_stubs.rs:33-93
pub fn pack_partinfo(part_info: &PartInfo) -> Vec<u8> {
    // Minimum size is 346 bytes (66 + 280), but we'll write the full struct
    let mut buf = vec![0u8; 346];

    // Bytes 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&part_info.next.to_be_bytes());

    // Bytes 2-7: velocity, staffs, patch, channel, transpose
    buf[2] = part_info.part_velocity as u8;
    buf[3] = part_info.first_staff as u8;
    buf[4] = part_info.patch_num;
    buf[5] = part_info.last_staff as u8;
    buf[6] = part_info.channel;
    buf[7] = part_info.transpose as u8;

    // Bytes 8-11: key range (i16, big-endian)
    buf[8..10].copy_from_slice(&part_info.lo_key_num.to_be_bytes());
    buf[10..12].copy_from_slice(&part_info.hi_key_num.to_be_bytes());

    // Bytes 12-43: name (32-byte C string)
    buf[12..44].copy_from_slice(&part_info.name);

    // Bytes 44-55: shortName (12-byte C string)
    buf[44..56].copy_from_slice(&part_info.short_name);

    // Bytes 56-61: key names and accidentals
    buf[56] = part_info.hi_key_name;
    buf[57] = part_info.hi_key_acc;
    buf[58] = part_info.tran_name;
    buf[59] = part_info.tran_acc;
    buf[60] = part_info.lo_key_name;
    buf[61] = part_info.lo_key_acc;

    // Bytes 62-63: bank numbers
    buf[62] = part_info.bank_number0;
    buf[63] = part_info.bank_number32;

    // Bytes 64-65: fmsOutputDevice (u16, big-endian)
    buf[64..66].copy_from_slice(&part_info.fms_output_device.to_be_bytes());

    // Bytes 66-345: fmsOutputDestination[280] (obsolete, already zeroed)
    // The struct has this field but we write zeros

    buf
}

/// Pack N105 ARPTEND_5 to raw bytes (8 bytes total).
///
/// Layout (matching unpack_arptend_n105):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2     staffn (SignedByte)
/// 3     subType (SignedByte)
/// 4     selected:1|visible:1|soft:1|spare:5
/// 5     connAbove (Byte)
/// 6     filler (Byte)
/// 7     connStaff (SignedByte)
/// ```
///
/// Source: NObjTypes.h:142-147, unpack_stubs.rs:337-373
pub fn pack_arptend_n105(rptend: &ARptEnd, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 8];

    // Bytes 0-1: next (LINK, big-endian)
    // Convert in-memory LINK to sequential file index before packing
    let file_index = link_map.convert(rptend.header.next);
    buf[0..2].copy_from_slice(&file_index.to_be_bytes());

    // Byte 2: staffn
    buf[2] = rptend.header.staffn as u8;

    // Byte 3: subType
    buf[3] = rptend.header.sub_type as u8;

    // Byte 4: selected:1|visible:1|soft:1|spare:5
    let mut b4: u8 = 0;
    if rptend.header.selected {
        b4 |= 0x80; // bit 7
    }
    if rptend.header.visible {
        b4 |= 0x40; // bit 6
    }
    if rptend.header.soft {
        b4 |= 0x20; // bit 5
    }
    // bits 4-0 are spare (leave as 0)
    buf[4] = b4;

    // Byte 5: connAbove
    buf[5] = rptend.conn_above;

    // Byte 6: filler
    buf[6] = rptend.filler;

    // Byte 7: connStaff
    buf[7] = rptend.conn_staff as u8;

    buf
}

/// Pack N105 APSMEAS_5 to raw bytes (8 bytes total).
///
/// Layout (matching unpack_apsmeas_n105):
/// ```text
/// 0-1   next (LINK, big-endian)
/// 2     staffn (SignedByte)
/// 3     subType (SignedByte) — barline type
/// 4     selected:1|visible:1|soft:1|spare:5
/// 5     connAbove (Boolean)
/// 6     filler1 (char)
/// 7     connStaff (SignedByte)
/// ```
///
/// Source: NObjTypesN105.h, unpack_stubs.rs:384-420
pub fn pack_apsmeas_n105(psmeas: &APsMeas, link_map: &SubobjLinkMap) -> Vec<u8> {
    let mut buf = vec![0u8; 8];

    // Bytes 0-1: next (LINK, big-endian)
    // Convert in-memory LINK to sequential file index before packing
    let file_index = link_map.convert(psmeas.header.next);
    buf[0..2].copy_from_slice(&file_index.to_be_bytes());

    // Byte 2: staffn
    buf[2] = psmeas.header.staffn as u8;

    // Byte 3: subType (barline type: PSM_DOTTED=8, PSM_DOUBLE=9, PSM_FINALDBL=10)
    buf[3] = psmeas.header.sub_type as u8;

    // Byte 4: selected:1|visible:1|soft:1|spare:5
    let mut b4: u8 = 0;
    if psmeas.header.selected {
        b4 |= 0x80; // bit 7
    }
    if psmeas.header.visible {
        b4 |= 0x40; // bit 6
    }
    if psmeas.header.soft {
        b4 |= 0x20; // bit 5
    }
    // bits 4-0 are spare (leave as 0)
    buf[4] = b4;

    // Byte 5: connAbove (Boolean: 0 or 1)
    buf[5] = if psmeas.conn_above { 1 } else { 0 };

    // Byte 6: filler1
    buf[6] = psmeas.filler1;

    // Byte 7: connStaff
    buf[7] = psmeas.conn_staff as u8;

    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_types::{DRect, KsItem};
    use crate::obj_types::SubObjHeader;

    #[test]
    fn test_pack_astaff_roundtrip() {
        // Create a test AStaff with known values
        let original = AStaff {
            next: 42,
            staffn: 1,
            selected: true,
            visible: true,
            filler_stf: false,
            staff_top: 100,
            staff_left: 0,
            staff_right: 2000,
            staff_height: 100,
            staff_lines: 5,
            font_size: 14,
            flag_leading: 10,
            min_stem_free: 20,
            ledger_width: 25,
            note_head_width: 30,
            frac_beam_width: 8,
            space_below: 50,
            clef_type: 3, // Treble
            dynamic_type: 0,
            ks_info: KsInfo::default(),
            time_sig_type: 1,
            numerator: 4,
            denominator: 4,
            filler: 0,
            show_ledgers: 1,
            show_lines: 15,
        };

        // Pack and verify size
        let mut link_map = SubobjLinkMap::new();
        link_map.register(original.next);
        let packed = pack_astaff_n105(&original, &link_map);
        assert_eq!(packed.len(), 50, "ASTAFF must be 50 bytes");

        // Verify key fields are present (file index should be 1, not 42)
        assert_eq!(u16::from_be_bytes([packed[0], packed[1]]), 1);
        assert_eq!(packed[2] as i8, 1);
        assert_eq!(packed[3] & 0xC0, 0xC0); // selected and visible set
        assert_eq!(i16::from_be_bytes([packed[4], packed[5]]), 100);
    }

    #[test]
    fn test_pack_ameasure_roundtrip() {
        let header = SubObjHeader {
            next: 10,
            staffn: 1,
            sub_type: 1,
            selected: false,
            visible: true,
            soft: false,
        };

        let original = AMeasure {
            header,
            measure_visible: true,
            conn_above: false,
            filler1: 0,
            filler2: 0,
            reserved_m: 0,
            measure_num: 1,
            meas_size_rect: DRect {
                top: 0,
                left: 0,
                bottom: 100,
                right: 500,
            },
            conn_staff: 0,
            clef_type: 3,
            dynamic_type: 0,
            ks_info: KsInfo::default(),
            time_sig_type: 1,
            numerator: 4,
            denominator: 4,
            x_mn_std_offset: 0,
            y_mn_std_offset: 0,
        };

        // Pack and verify size
        let mut link_map = SubobjLinkMap::new();
        link_map.register(original.header.next);
        let packed = pack_ameasure_n105(&original, &link_map);
        assert_eq!(packed.len(), 40, "AMEASURE must be 40 bytes");

        // Verify header fields (file index should be 1, not 10)
        assert_eq!(u16::from_be_bytes([packed[0], packed[1]]), 1);
        assert_eq!(packed[2] as i8, 1);
        assert_eq!(packed[3] as i8, 1);
    }

    #[test]
    fn test_pack_ksinfo_default() {
        let ks_info = KsInfo::default();
        let mut buf = vec![0u8; 20];

        pack_ksinfo_n105(&ks_info, &mut buf, 0);

        // nKSItems should be 0 at offset 14
        assert_eq!(buf[14], 0);

        // All KSItem entries should be 0 (default)
        for item in buf.iter().take(14) {
            assert_eq!(*item, 0);
        }
    }

    #[test]
    fn test_pack_ksinfo_with_sharps() {
        let mut ks_info = KsInfo::default();
        ks_info.ks_item[0] = KsItem {
            letcode: 6, // G
            sharp: 1,
        };
        ks_info.n_ks_items = 1;

        let mut buf = vec![0u8; 20];
        pack_ksinfo_n105(&ks_info, &mut buf, 0);

        // First KSItem at offset 0: letcode=6 (bits 7-1), sharp=1 (bit 0)
        // Expected: (6 << 1) | 1 = 0b1100_0011 = 0xC
        let expected_byte = ((6u8 & 0x7F) << 1) | (1u8 & 1);
        assert_eq!(buf[0], expected_byte);

        // nKSItems at offset 14
        assert_eq!(buf[14], 1);
    }
}
