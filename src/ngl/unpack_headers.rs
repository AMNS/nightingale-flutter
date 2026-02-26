//! N105 header unpacking: ObjectHeader, SubObjHeader, and KSINFO helper.
//!
//! These foundational unpackers are used by all other subobject unpackers.

use crate::basic_types::{KsInfo, KsItem, Rect};
use crate::obj_types::{ObjectHeader, SubObjHeader};

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

/// Unpack WHOLE_KSINFO_5 from N105 format.
///
/// Source: NBasicTypesN105.h lines 31-48
///
/// N105 KSITEM_5 is a 1-byte bitfield struct:
///   char letcode:7;    // bits 6-0: F=0, E=1, D=2, C=3, B=4, A=5, G=6
///   Boolean sharp:1;   // bit 7: True=sharp, False=flat
///
/// On-disk layout (mac68k alignment, 16 bytes):
///   offset+0..+13: KSITEM_5[7] (7 items x 2 bytes: 1 data + 1 pad)
///   offset+14: nKSItems (SignedByte)
///   offset+15: padding
pub fn unpack_ksinfo_n105(data: &[u8], offset: usize) -> KsInfo {
    let mut ks_info = KsInfo {
        ks_item: [KsItem::default(); crate::basic_types::MAX_KSITEMS],
        n_ks_items: 0,
    };

    // Need at least 16 bytes for mac68k padded KSINFO
    if data.len() < offset + 16 {
        return ks_info;
    }

    // KSItem array: 7 items x 2 bytes each (1 data byte + 1 padding byte).
    // KSITEM_5 bitfield on 68k/PPC (MSB-first bitfield ordering):
    //   char letcode:7;     // bits 7-1 (MSB)
    //   Boolean sharp:1;    // bit 0 (LSB)
    for i in 0..crate::basic_types::MAX_KSITEMS {
        let item_offset = offset + i * 2;
        let b = data[item_offset];
        ks_info.ks_item[i] = KsItem {
            letcode: ((b >> 1) & 0x7F) as i8,
            sharp: b & 1,
        };
    }

    // nKSItems at offset+14
    ks_info.n_ks_items = data[offset + 14] as i8;

    ks_info
}
