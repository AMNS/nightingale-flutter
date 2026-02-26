//! N105 slur subobject unpacker: ASLUR.

use crate::basic_types::{DPoint, Point, Rect};
use crate::obj_types::ASlur;

/// Unpack N105 ASLUR_5 from raw bytes (42 bytes).
///
/// Source: NObjTypesN105.h lines 456-470
///
/// On-disk layout (mac68k alignment, 42 bytes total):
/// ```text
/// Offset  Size  Field
/// 0-1     2     next (LINK)
/// 2       1     selected:1|visible:1|soft:1|dashed:2|unused:3  (bitfield byte)
/// 3       1     filler (SignedByte)
/// 4-11    8     bounds (Rect: top,left,bottom,right x i16)
/// 12      1     firstInd (SignedByte — starting note index in chord)
/// 13      1     lastInd (SignedByte — ending note index in chord)
/// 14-17   4     reserved (long)
/// 18-29   12    seg (SplineSeg: knot + c0 + c1, each DPoint = 2xi16)
/// 30-33   4     startPt (Point: v,h x i16) — paper-relative base position
/// 34-37   4     endPt (Point: v,h x i16) — paper-relative base position
/// 38-41   4     endKnot (DPoint: v,h x i16) — relative to endPt
///               TOTAL: 42 bytes
/// ```
///
/// **Bitfield byte (offset 2):**
/// ```text
/// Bit 7:       selected
/// Bit 6:       visible
/// Bit 5:       soft
/// Bits 4-3:    dashed (2 bits)
/// Bits 2-0:    unused
/// ```
///
/// Note: Byte 3 (filler) was previously missing from offset calculations,
/// causing all subsequent fields to be read from wrong offsets.
/// Verified empirically: startPt at offset 30 yields valid paper coordinates
/// (e.g., h=82..250, v=225 for capital_regiment_march.ngl), while offset 29
/// yields garbage values like -19456.
pub fn unpack_aslur_n105(data: &[u8]) -> Result<ASlur, String> {
    if data.len() < 42 {
        return Err(format!("ASLUR too short: {} bytes", data.len()));
    }

    let next = u16::from_be_bytes([data[0], data[1]]);

    // Byte 2: bitfield (selected, visible, soft, dashed)
    let b2 = data[2];
    let selected = (b2 & 0x80) != 0;
    let visible = (b2 & 0x40) != 0;
    let soft = (b2 & 0x20) != 0;
    let dashed = (b2 & 0x18) != 0; // 2-bit field at bits 4-3
    let filler = (b2 & 0x07) != 0;

    // Byte 3: filler (SignedByte) — mac68k padding/unused field

    // Bytes 4-11: bounds (Rect = 4 x i16)
    let bounds = Rect {
        top: i16::from_be_bytes([data[4], data[5]]),
        left: i16::from_be_bytes([data[6], data[7]]),
        bottom: i16::from_be_bytes([data[8], data[9]]),
        right: i16::from_be_bytes([data[10], data[11]]),
    };

    // Bytes 12-13: firstInd, lastInd
    let first_ind = data[12] as i8;
    let last_ind = data[13] as i8;

    // Bytes 14-17: reserved (long, 4 bytes)
    let reserved = i32::from_be_bytes([data[14], data[15], data[16], data[17]]);

    // Bytes 18-29: SplineSeg (3 x DPoint, each 4 bytes = 12 bytes)
    // seg.knot: offset from startPt to start knot (DDIST)
    let seg_knot = DPoint {
        v: i16::from_be_bytes([data[18], data[19]]),
        h: i16::from_be_bytes([data[20], data[21]]),
    };
    // seg.c0: offset from start knot to first control point (DDIST)
    let seg_c0 = DPoint {
        v: i16::from_be_bytes([data[22], data[23]]),
        h: i16::from_be_bytes([data[24], data[25]]),
    };
    // seg.c1: offset from end knot to second control point (DDIST)
    let seg_c1 = DPoint {
        v: i16::from_be_bytes([data[26], data[27]]),
        h: i16::from_be_bytes([data[28], data[29]]),
    };

    // Bytes 30-33: startPt (Point: v,h — paper-relative, in screen points)
    let start_pt = Point {
        v: i16::from_be_bytes([data[30], data[31]]),
        h: i16::from_be_bytes([data[32], data[33]]),
    };

    // Bytes 34-37: endPt (Point: v,h — paper-relative, in screen points)
    let end_pt = Point {
        v: i16::from_be_bytes([data[34], data[35]]),
        h: i16::from_be_bytes([data[36], data[37]]),
    };

    // Bytes 38-41: endKnot (DPoint: v,h — relative to endPt, in DDIST)
    let end_knot = DPoint {
        v: i16::from_be_bytes([data[38], data[39]]),
        h: i16::from_be_bytes([data[40], data[41]]),
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
