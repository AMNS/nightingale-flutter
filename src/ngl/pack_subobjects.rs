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

use crate::basic_types::KsInfo;
use crate::obj_types::{AMeasure, AStaff, SubObjHeader};

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
pub fn pack_astaff_n105(staff: &AStaff) -> Vec<u8> {
    let mut buf = vec![0u8; 50];

    // Offset 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&staff.next.to_be_bytes());

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
pub fn pack_ameasure_n105(measure: &AMeasure) -> Vec<u8> {
    let mut buf = vec![0u8; 40];

    // Offset 0-3: SUBOBJHEADER_5
    pack_subobj_header_n105(&measure.header, &mut buf);

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
fn pack_subobj_header_n105(header: &SubObjHeader, buf: &mut [u8]) {
    // Offset 0-1: next (LINK, big-endian)
    buf[0..2].copy_from_slice(&header.next.to_be_bytes());

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
        let packed = pack_astaff_n105(&original);
        assert_eq!(packed.len(), 50, "ASTAFF must be 50 bytes");

        // Verify key fields are present
        assert_eq!(u16::from_be_bytes([packed[0], packed[1]]), 42);
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
        let packed = pack_ameasure_n105(&original);
        assert_eq!(packed.len(), 40, "AMEASURE must be 40 bytes");

        // Verify header fields
        assert_eq!(u16::from_be_bytes([packed[0], packed[1]]), 10);
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
