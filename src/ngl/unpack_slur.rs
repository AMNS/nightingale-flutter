//! N105 slur subobject unpacker: ASLUR.

use crate::basic_types::{DPoint, Point, Rect};
use crate::obj_types::ASlur;

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
