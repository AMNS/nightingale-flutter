//! Stub unpackers for N105 subobject types not yet fully implemented.
//!
//! These will be filled in as we port additional rendering features.

use crate::obj_types::{AConnect, ADynamic, AGraphic, AModNr, ANoteOttava, APsMeas, ARptEnd};

pub fn unpack_aconnect_n105(_data: &[u8]) -> Result<AConnect, String> {
    // TODO: Implement full ACONNECT_5 unpacking (12 bytes, bitfields in byte 2)
    Err("ACONNECT unpacking not yet implemented".to_string())
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

pub fn unpack_agraphic_n105(_data: &[u8]) -> Result<AGraphic, String> {
    // TODO: Implement full AGRAPHIC_5 unpacking (6 bytes)
    Err("AGRAPHIC unpacking not yet implemented".to_string())
}

pub fn unpack_anoteottava_n105(_data: &[u8]) -> Result<ANoteOttava, String> {
    // TODO: Implement full ANOTEOTTAVA_5 unpacking (4 bytes)
    Err("ANOTEOTTAVA unpacking not yet implemented".to_string())
}

pub fn unpack_arptend_n105(_data: &[u8]) -> Result<ARptEnd, String> {
    // TODO: Implement full ARPTEND_5 unpacking (6 bytes, bitfields in byte 4)
    Err("ARPTEND unpacking not yet implemented".to_string())
}

pub fn unpack_apsmeas_n105(_data: &[u8]) -> Result<APsMeas, String> {
    // TODO: Implement full APSMEAS_5 unpacking (6 bytes, bitfields in byte 4)
    Err("APSMEAS unpacking not yet implemented".to_string())
}
