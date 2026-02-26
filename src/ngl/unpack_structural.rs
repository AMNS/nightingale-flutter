//! N105 structural subobject unpackers: ASTAFF, AMEASURE.

use crate::basic_types::{DRect, ShortStd};
use crate::obj_types::{AMeasure, AStaff};

use super::unpack_headers::{unpack_ksinfo_n105, unpack_subobj_header_n105};

/// Unpack N105 ASTAFF_5 from raw bytes (50 bytes).
///
/// On-disk layout with mac68k alignment (50 bytes total):
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
/// 30      14    KSItem[0..6] (7 x 2 bytes each, mac68k padded)
/// 44      1     nKSItems
/// 45      1     timeSigType
/// 46      1     numerator
/// 47      1     denominator
/// 48      1     filler:3+showLedgers:1+showLines:4
/// 49      1     [PADDING — struct aligned to 2-byte boundary]
/// ```
///
/// Source: NObjTypesN105.h lines 152-180
/// See CLAUDE.md "N105 Struct Alignment" for derivation.
pub fn unpack_astaff_n105(data: &[u8]) -> Result<AStaff, String> {
    if data.len() < 50 {
        return Err(format!("ASTAFF too short: {} bytes", data.len()));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);
    let staffn = data[2] as i8;

    // Byte 3: selected:1 | visible:1 | fillerStf:6
    let b3 = data[3];
    let selected = (b3 & 0x80) != 0;
    let visible = (b3 & 0x40) != 0;
    let filler_stf = (b3 & 0x20) != 0;

    let staff_top = i16::from_be_bytes([data[4], data[5]]);
    let staff_left = i16::from_be_bytes([data[6], data[7]]);
    let staff_right = i16::from_be_bytes([data[8], data[9]]);
    let staff_height = i16::from_be_bytes([data[10], data[11]]);
    let staff_lines = data[12] as i8;
    // Byte 13 is padding (align fontSize to 2-byte boundary)
    let font_size = i16::from_be_bytes([data[14], data[15]]);
    let flag_leading = i16::from_be_bytes([data[16], data[17]]);
    let min_stem_free = i16::from_be_bytes([data[18], data[19]]);
    let ledger_width = i16::from_be_bytes([data[20], data[21]]);
    let note_head_width = i16::from_be_bytes([data[22], data[23]]);
    let frac_beam_width = i16::from_be_bytes([data[24], data[25]]);
    let space_below = i16::from_be_bytes([data[26], data[27]]);
    let clef_type = data[28] as i8;
    let dynamic_type = data[29] as i8;

    // WHOLE_KSINFO_5: 15 bytes starting at offset 30
    // (7 x KSITEM_5 @ 2 bytes each = 14 bytes, then nKSItems = 1 byte)
    let ks_info = unpack_ksinfo_n105(data, 30);

    // Offsets 45-47: timeSigType, numerator, denominator (no padding after nKSItems)
    let time_sig_type = data[45] as i8;
    let numerator = data[46] as i8;
    let denominator = data[47] as i8;

    // Byte 48: filler:3 (bits 7-5) | showLedgers:1 (bit 4) | showLines:4 (bits 3-0)
    // Byte 49 is trailing struct padding (mac68k aligns to 2-byte boundary: 49->50)
    // Reference: NObjTypesN105.h line 176, SHOW_ALL_LINES=15
    let b48 = data[48];
    let show_ledgers = (b48 >> 4) & 1;
    let show_lines = b48 & 0x0F;
    let filler = (b48 >> 5) & 0x07;

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
        show_ledgers,
        show_lines,
    })
}

/// Unpack N105 AMEASURE_5 from raw bytes (40 bytes).
///
/// Source: NObjTypesN105.h lines 222-253
/// On-disk layout (mac68k alignment):
///   0-3   SUBOBJHEADER_5
///   4     measureVisible:1 | connAbove:1 | filler1:3 | unused:3
///   5     filler2 (SignedByte)
///   6-7   oldFakeMeas:1 | measureNum:15 (short)
///   8-15  measSizeRect (DRect = 4 x DDIST)
///   16    connStaff (SignedByte)
///   17    clefType (SignedByte)
///   18    dynamicType (SignedByte)
///   19    [PADDING — align KSITEM_5 struct to 2-byte boundary]
///   20-33 KSITEM_5[7] (7 x 2 bytes = 14)
///   34    nKSItems
///   35    timeSigType
///   36    numerator
///   37    denominator
///   38    xMNStdOffset (SHORTSTD = SignedByte)
///   39    yMNStdOffset (SHORTSTD = SignedByte)
///         TOTAL: 40 bytes
///
/// Source: NObjTypesN105.h lines 192-210
pub fn unpack_ameasure_n105(data: &[u8]) -> Result<AMeasure, String> {
    if data.len() < 40 {
        return Err(format!("AMEASURE too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4 layout (shared with SUBOBJHEADER_5):
    //   bit 7: selected (header)
    //   bit 6: visible (header)
    //   bit 5: soft (header)
    //   bit 4: measureVisible
    //   bit 3: connAbove
    //   bits 2-0: filler1
    let b4 = data[4];
    let measure_visible = (b4 >> 4) & 1 != 0;
    let conn_above = (b4 >> 3) & 1 != 0;
    let filler1 = b4 & 0x07;

    // Byte 5: filler2
    let filler2 = data[5] as i8;

    // Bytes 6-7: oldFakeMeas:1 | measureNum:15 (single short)
    let meas_short = i16::from_be_bytes([data[6], data[7]]);
    let reserved_m = meas_short; // keep raw value for compat
    let measure_num = meas_short & 0x7FFF;

    // Bytes 8-15: DRect (4 x DDIST)
    let meas_size_rect = DRect {
        top: i16::from_be_bytes([data[8], data[9]]),
        left: i16::from_be_bytes([data[10], data[11]]),
        bottom: i16::from_be_bytes([data[12], data[13]]),
        right: i16::from_be_bytes([data[14], data[15]]),
    };

    // Bytes 16-18: connStaff, clefType, dynamicType
    let conn_staff = data[16] as i8;
    let clef_type = data[17] as i8;
    let dynamic_type = data[18] as i8;

    // Byte 19: [PADDING — align KSITEM_5 struct to 2-byte boundary]
    // Bytes 20-34: WHOLE_KSINFO_5 (15 bytes)
    let ks_info = unpack_ksinfo_n105(data, 20);

    // Bytes 35-37: timeSigType, numerator, denominator
    let time_sig_type = data[35] as i8;
    let numerator = data[36] as i8;
    let denominator = data[37] as i8;

    // Bytes 38-39: xMNStdOffset, yMNStdOffset (SHORTSTD = SignedByte)
    let x_mn_std_offset = data[38] as i8 as ShortStd;
    let y_mn_std_offset = data[39] as i8 as ShortStd;

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
