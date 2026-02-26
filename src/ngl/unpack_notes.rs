//! N105 note-related subobject unpackers: ANOTE, ANOTEBEAM, ANOTETUPLE.

use crate::basic_types::ShortQd;
use crate::obj_types::{ANote, ANoteBeam, ANoteTuple};

use super::unpack_headers::unpack_subobj_header_n105;

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

/// Unpack ANOTETUPLE_5 from N105 format (4 bytes).
/// Layout:
///   0-1  next (LINK, u16 big-endian)
///   2-3  tpSync (LINK, u16 big-endian)
pub fn unpack_anotetuple_n105(data: &[u8]) -> Result<ANoteTuple, String> {
    if data.len() < 4 {
        return Err(format!("ANOTETUPLE_5 data too short: {} bytes", data.len()));
    }
    let next = u16::from_be_bytes([data[0], data[1]]);
    let tp_sync = u16::from_be_bytes([data[2], data[3]]);
    Ok(ANoteTuple { next, tp_sync })
}
