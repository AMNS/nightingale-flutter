//! Stub unpackers for N105 subobject types not yet fully implemented.
//!
//! These will be filled in as we port additional rendering features.

use crate::obj_types::{
    AConnect, ADynamic, AGraphic, AModNr, ANoteOttava, APsMeas, ARptEnd, PartInfo,
};

/// Unpack PARTINFO subobject from N105/N103 binary data.
///
/// On-disk layout (NBasicTypes.h:171-201, no mac68k padding issues — all fields align):
/// ```text
/// Offset  Size  Field
/// ------  ----  ---------
/// 0       2     next (LINK)
/// 2       1     partVelocity
/// 3       1     firstStaff
/// 4       1     patchNum
/// 5       1     lastStaff
/// 6       1     channel
/// 7       1     transpose
/// 8       2     loKeyNum
/// 10      2     hiKeyNum
/// 12      32    name[32] (C string)
/// 44      12    shortName[12] (C string)
/// 56      6     hiKeyName..loKeyAcc
/// 62+     2     bankNumber0, bankNumber32 (N103+, may not be present)
/// 64+     282   FreeMIDI fields (obsolete, variable size)
/// ```
///
/// We read the essential fields (up to offset 56) and handle
/// optional trailing fields gracefully.
pub fn unpack_partinfo(data: &[u8]) -> Result<PartInfo, String> {
    if data.len() < 56 {
        return Err(format!(
            "PARTINFO data too short: {} bytes (need >=56)",
            data.len()
        ));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);
    let part_velocity = data[2] as i8;
    let first_staff = data[3] as i8;
    let patch_num = data[4];
    let last_staff = data[5] as i8;
    let channel = data[6];
    let transpose = data[7] as i8;
    let lo_key_num = i16::from_be_bytes([data[8], data[9]]);
    let hi_key_num = i16::from_be_bytes([data[10], data[11]]);

    let mut name = [0u8; 32];
    name.copy_from_slice(&data[12..44]);
    let mut short_name = [0u8; 12];
    short_name.copy_from_slice(&data[44..56]);

    let hi_key_name = if data.len() > 56 { data[56] } else { 0 };
    let hi_key_acc = if data.len() > 57 { data[57] } else { 0 };
    let tran_name = if data.len() > 58 { data[58] } else { 0 };
    let tran_acc = if data.len() > 59 { data[59] } else { 0 };
    let lo_key_name = if data.len() > 60 { data[60] } else { 0 };
    let lo_key_acc = if data.len() > 61 { data[61] } else { 0 };
    let bank_number0 = if data.len() > 62 { data[62] } else { 0 };
    let bank_number32 = if data.len() > 63 { data[63] } else { 0 };
    let fms_output_device = if data.len() > 65 {
        u16::from_be_bytes([data[64], data[65]])
    } else {
        0
    };

    Ok(PartInfo {
        next,
        part_velocity,
        first_staff,
        last_staff,
        patch_num,
        channel,
        transpose,
        lo_key_num,
        hi_key_num,
        name,
        short_name,
        hi_key_name,
        hi_key_acc,
        tran_name,
        tran_acc,
        lo_key_name,
        lo_key_acc,
        bank_number0,
        bank_number32,
        fms_output_device,
        fms_output_destination: [0u8; 280], // Obsolete, don't bother reading
    })
}

/// Unpack ACONNECT_5 from N105 binary data.
///
/// On-disk layout (NObjTypesN105.h:338-349, mac68k alignment):
/// ```text
/// Offset  Size  Field
/// ------  ----  ---------
/// 0       2     next (LINK)
/// 2       1     bitfields: selected:1 | filler:1 | connLevel:3 | connectType:2
/// 3       1     staffAbove (SignedByte)
/// 4       1     staffBelow (SignedByte)
/// 5       1     [PADDING — align xd to 2-byte boundary]
/// 6       2     xd (DDIST)
/// 8       2     firstPart (LINK, unused)
/// 10      2     lastPart (LINK, unused)
///               TOTAL: 12 bytes
/// ```
pub fn unpack_aconnect_n105(data: &[u8]) -> Result<AConnect, String> {
    if data.len() < 12 {
        return Err(format!(
            "ACONNECT_5 data too short: {} bytes (need >=12)",
            data.len()
        ));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);

    // Byte 2 bitfields (PowerPC MSB-first):
    //   bit 7: selected
    //   bit 6: filler
    //   bits 5-3: connLevel (0=system, 1=group, 7=part)
    //   bits 2-1: connectType (1=line, 2=bracket, 3=curly/brace)
    let byte2 = data[2];
    let selected = (byte2 & 0x80) != 0;
    let filler = (byte2 & 0x40) >> 6;
    let conn_level = (byte2 & 0x38) >> 3;
    let connect_type = (byte2 & 0x06) >> 1;

    let staff_above = data[3] as i8;
    let staff_below = data[4] as i8;
    // byte 5 is padding
    let xd = i16::from_be_bytes([data[6], data[7]]);
    let first_part = u16::from_be_bytes([data[8], data[9]]);
    let last_part = u16::from_be_bytes([data[10], data[11]]);

    Ok(AConnect {
        next,
        selected,
        filler,
        conn_level,
        connect_type,
        staff_above,
        staff_below,
        xd,
        first_part,
        last_part,
    })
}

/// Unpack ADYNAMIC_5 from N105 binary data.
///
/// On-disk layout (NObjTypesN105.h:359-368, mac68k alignment):
/// ```text
/// Offset  Size  Field
/// ------  ----  ---------
/// 0       2     next (LINK)
/// 2       1     staffn
/// 3       1     subType (unused for dynamics)
/// 4       1     selected:1 + visible:1 + soft:1 + mouthWidth_hi:5
/// 5       1     small:2 + otherWidth:6
///                (note: byte 4 is shared between SUBOBJHEADER_5 bitfields
///                 and mouthWidth; byte 5 has small + otherWidth)
/// 6       2     xd (DDIST, unused)
/// 8       2     yd (DDIST)
/// 10      2     endxd (DDIST)
/// 12      2     endyd (DDIST)
/// Total:  14 bytes (no padding needed — already even)
/// ```
///
/// If heap obj_size differs, the extra bytes are padding.
pub fn unpack_adynamic_n105(data: &[u8]) -> Result<ADynamic, String> {
    if data.len() < 14 {
        return Err(format!(
            "ADYNAMIC_5 data too short: {} bytes (need >=14)",
            data.len()
        ));
    }
    use crate::obj_types::SubObjHeader;

    let next = u16::from_be_bytes([data[0], data[1]]);
    let staffn = data[2] as i8;
    let sub_type = data[3] as i8;

    // Byte 4: SUBOBJHEADER_5 bitfields in top 3 bits, then mouthWidth in lower 5
    let byte4 = data[4];
    let selected = (byte4 & 0x80) != 0;
    let visible = (byte4 & 0x40) != 0;
    let soft = (byte4 & 0x20) != 0;
    let mouth_width = byte4 & 0x1F; // lower 5 bits

    // Byte 5: small in top 2 bits, otherWidth in lower 6
    let byte5 = data[5];
    let small = (byte5 >> 6) & 0x03;
    let other_width = byte5 & 0x3F;

    // DDISTs at offsets 6, 8, 10, 12
    let xd = i16::from_be_bytes([data[6], data[7]]);
    let yd = i16::from_be_bytes([data[8], data[9]]);
    let endxd = i16::from_be_bytes([data[10], data[11]]);
    let endyd = i16::from_be_bytes([data[12], data[13]]);

    Ok(ADynamic {
        header: SubObjHeader {
            next,
            staffn,
            sub_type,
            selected,
            visible,
            soft,
        },
        mouth_width,
        small,
        other_width,
        xd,
        yd,
        endxd,
        endyd,
        d_mod_code: 0,  // Not in N105
        cross_staff: 0, // Not in N105
    })
}

/// Unpack AMODNR_5 from N105 binary data.
///
/// On-disk layout (NObjTypesN105.h:382-391):
/// ```text
/// Offset  Size  Field
/// ------  ----  ---------
/// 0       2     next (LINK)
/// 2       1     selected:1 + visible:1 + soft:1 + xstd:5
/// 3       1     modCode (Byte)
/// 4       1     data (SignedByte)
/// 5       1     ystdpit (SHORTSTD = SignedByte, 1 byte)
/// Total:  6 bytes (no mac68k padding — already even)
/// ```
///
/// Note: xstd is a 5-bit unsigned field encoding signed values via XSTD_OFFSET (16).
/// We store the raw value; caller subtracts XSTD_OFFSET when computing positions.
/// SHORTSTD is SignedByte (1 byte), not short (2 bytes).
///
/// Source: NObjTypesN105.h lines 382-391, NObjTypes.h lines 512-523
pub fn unpack_amodnr_n105(data: &[u8]) -> Result<AModNr, String> {
    if data.len() < 6 {
        return Err(format!(
            "AMODNR_5 data too short: {} bytes (need >=6)",
            data.len()
        ));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);

    // Byte 2: selected:1 | visible:1 | soft:1 | xstd:5
    let byte2 = data[2];
    let selected = (byte2 & 0x80) != 0;
    let visible = (byte2 & 0x40) != 0;
    let soft = (byte2 & 0x20) != 0;
    let xstd = byte2 & 0x1F; // Lower 5 bits (biased by XSTD_OFFSET=16)

    let mod_code = data[3];
    let mod_data = data[4] as i8;
    let ystdpit = data[5] as i8; // SHORTSTD = SignedByte (1 byte)

    Ok(AModNr {
        next,
        selected,
        visible,
        soft,
        xstd,
        mod_code,
        data: mod_data,
        ystdpit,
    })
}

/// Unpack AGRAPHIC_5 subobject (6 bytes on disk).
///
/// Layout:
///   Offset 0-1: next (LINK, u16 big-endian)
///   Offset 2-5: strOffset (STRINGOFFSET = long, i32 big-endian)
///
/// Source: NObjTypesN105.h lines 396-399
pub fn unpack_agraphic_n105(data: &[u8]) -> Result<AGraphic, String> {
    if data.len() < 6 {
        return Err(format!(
            "AGRAPHIC_5 data too short: {} bytes (need >=6)",
            data.len()
        ));
    }
    let next = u16::from_be_bytes([data[0], data[1]]);
    let str_offset = i32::from_be_bytes([data[2], data[3], data[4], data[5]]);
    Ok(AGraphic { next, str_offset })
}

/// Unpack ANOTEOTTAVA_5 subobject from N105 binary data.
///
/// On-disk layout (4 bytes, ANOTEOTTAVA_5):
/// ```text
/// Offset  Size  Field
/// ------  ----  ----------
/// 0       2     next (LINK)
/// 2       2     opSync (LINK) — Sync containing note/chord under this ottava
/// ```
///
/// Source: NObjTypesN105.h lines 431-434
pub fn unpack_anoteottava_n105(data: &[u8]) -> Result<ANoteOttava, String> {
    if data.len() < 4 {
        return Err(format!(
            "ANOTEOTTAVA_5 data too short: {} bytes (need 4)",
            data.len()
        ));
    }
    let next = u16::from_be_bytes([data[0], data[1]]);
    let op_sync = u16::from_be_bytes([data[2], data[3]]);
    Ok(ANoteOttava { next, op_sync })
}

/// Unpack ARPTEND_5 from N105 binary data.
///
/// On-disk layout (NObjTypes.h:142-147, SUBOBJHEADER + 3 bytes, mac68k alignment):
/// ```text
/// Offset  Size  Field
/// ------  ----  ---------
/// 0       2     next (LINK)
/// 2       1     staffn (SignedByte)
/// 3       1     subType (SignedByte, unused for ARPTEND)
/// 4       1     selected:1 | visible:1 | soft:1 | spare:5
/// 5       1     connAbove (Byte)
/// 6       1     filler (Byte, unused)
/// 7       1     connStaff (SignedByte)
///               TOTAL: 8 bytes
/// ```
///
/// Source: NObjTypes.h lines 142-147, RPTEND_AND_VOLTA_ANALYSIS.md
pub fn unpack_arptend_n105(data: &[u8]) -> Result<ARptEnd, String> {
    if data.len() < 8 {
        return Err(format!(
            "ARPTEND_5 data too short: {} bytes (need >=8)",
            data.len()
        ));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);
    let staffn = data[2] as i8;
    let sub_type = data[3] as i8;

    // Byte 4: SUBOBJHEADER bitfields (selected:1 | visible:1 | soft:1 | spare:5)
    let byte4 = data[4];
    let selected = (byte4 & 0x80) != 0;
    let visible = (byte4 & 0x40) != 0;
    let soft = (byte4 & 0x20) != 0;

    // Bytes 5-7: ARPTEND-specific fields
    let conn_above = data[5];
    let filler = data[6];
    let conn_staff = data[7] as i8;

    Ok(ARptEnd {
        header: crate::obj_types::SubObjHeader {
            next,
            staffn,
            sub_type,
            selected,
            visible,
            soft,
        },
        conn_above,
        filler,
        conn_staff,
    })
}

pub fn unpack_apsmeas_n105(_data: &[u8]) -> Result<APsMeas, String> {
    // TODO: Implement full APSMEAS_5 unpacking (6 bytes, bitfields in byte 4)
    Err("APSMEAS unpacking not yet implemented".to_string())
}
