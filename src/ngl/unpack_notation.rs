//! N105 notation subobject unpackers: ACLEF, AKEYSIG, ATIMESIG.

use crate::obj_types::{AClef, AKeySig, ATimeSig};

use super::unpack_headers::{unpack_ksinfo_n105, unpack_subobj_header_n105};

/// Unpack N105 ACLEF_5 from raw bytes (10 bytes).
///
/// Source: NObjTypesN105.h lines 255-266
pub fn unpack_aclef_n105(data: &[u8]) -> Result<AClef, String> {
    if data.len() < 10 {
        return Err(format!("ACLEF too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4 layout (shared with SUBOBJHEADER_5):
    //   bits 7-5: selected:1|visible:1|soft:1 (header)
    //   bits 4-2: filler1:3
    //   bits 1-0: small:2
    // Reference: NObjTypesN105.h line 230-231
    let b4 = data[4];
    let filler1 = (b4 >> 2) & 0x07;
    let small = b4 & 0x03;
    // Byte 5: filler2
    let filler2 = data[5];
    // Bytes 6-7: xd (DDIST, 2-byte aligned — no padding needed after filler2)
    let xd = i16::from_be_bytes([data[6], data[7]]);
    // Bytes 8-9: yd
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
/// On-disk layout (mac68k alignment):
///   0-3   SUBOBJHEADER_5
///   4     nonstandard:1 | filler1:2 | small:2 | unused:3
///   5     filler2 (SignedByte)
///   6-7   xd (DDIST)
///   8-22  WHOLE_KSINFO_5 (7x2-byte KSITEM_5 + 1 nKSItems = 15 bytes)
///   23    [trailing mac68k padding]
///
/// Source: NObjTypesN105.h lines 262-270
pub fn unpack_akeysig_n105(data: &[u8]) -> Result<AKeySig, String> {
    if data.len() < 24 {
        return Err(format!("AKEYSIG too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4 layout (shared with SUBOBJHEADER_5):
    //   bits 7-5: selected:1|visible:1|soft:1 (header)
    //   bit 4: nonstandard:1
    //   bits 3-2: filler1:2
    //   bits 1-0: small:2
    // Reference: NObjTypesN105.h line 264-266
    let b4 = data[4];
    let nonstandard = (b4 >> 4) & 1;
    let filler1 = (b4 >> 2) & 0x03;
    let small = b4 & 0x03;

    // Byte 5: filler2
    let filler2 = data[5] as i8;

    // Bytes 6-7: xd (DDIST, 2-byte aligned)
    let xd = i16::from_be_bytes([data[6], data[7]]);

    // Bytes 8-22: WHOLE_KSINFO_5 (15 bytes)
    let ks_info = unpack_ksinfo_n105(data, 8);

    Ok(AKeySig {
        header,
        nonstandard,
        filler1,
        small,
        filler2,
        xd,
        ks_info,
    })
}

/// Unpack N105 ATIMESIG_5 from raw bytes (12 bytes).
///
/// On-disk layout (mac68k alignment):
///   0-3   SUBOBJHEADER_5
///   4     filler:3 | small:2 | unused:3  (bitfields in one Byte)
///   5     connStaff (SignedByte)
///   6-7   xd (DDIST)
///   8-9   yd (DDIST)
///   10    numerator (SignedByte)
///   11    denominator (SignedByte)
///
/// Source: NObjTypesN105.h lines 280-288
pub fn unpack_atimesig_n105(data: &[u8]) -> Result<ATimeSig, String> {
    if data.len() < 12 {
        return Err(format!("ATIMESIG too short: {} bytes", data.len()));
    }

    let header = unpack_subobj_header_n105(data)?;

    // Byte 4 layout (shared with SUBOBJHEADER_5):
    //   bits 7-5: selected:1|visible:1|soft:1 (header)
    //   bits 4-2: filler:3
    //   bits 1-0: small:2
    let b4 = data[4];
    let filler = (b4 >> 2) & 0x07;
    let small = b4 & 0x03;

    // Byte 5: connStaff
    let conn_staff = data[5] as i8;

    // Bytes 6-7: xd, 8-9: yd
    let xd = i16::from_be_bytes([data[6], data[7]]);
    let yd = i16::from_be_bytes([data[8], data[9]]);

    // Bytes 10-11: numerator, denominator
    let numerator = data[10] as i8;
    let denominator = data[11] as i8;

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
