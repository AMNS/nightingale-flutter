//! Pitch utilities — port of PitchUtils.cp.
//!
//! Shared pitch-to-staff-position conversion used by both the Notelist and
//! NGL pipelines.
//!
//! Reference: Nightingale/src/Utilities/PitchUtils.cp

use crate::basic_types::Ddist;

// ===========================================================================
// Accidental codes (from NObjTypes.h)
// ===========================================================================

pub const AC_DBLFLAT: u8 = 1;
pub const AC_FLAT: u8 = 2;
pub const AC_NATURAL: u8 = 3;
pub const AC_SHARP: u8 = 4;
pub const AC_DBLSHARP: u8 = 5;

// ===========================================================================
// Pitch conversion
// ===========================================================================

/// Marker for invalid table entries.
const XX: i8 = -99;

/// NLMIDI2HalfLn — brute-force pitch-to-staff-position conversion.
///
/// Given a MIDI note number, effective accidental code, and the half-line
/// position of middle C for the current clef, returns the half-line number
/// relative to the top staff line (0 = top line, positive = downward).
///
/// This is a direct port of NLMIDI2HalfLn from NotelistOpen.cp (line 579).
///
/// Returns `None` if the combination is invalid (e.g., C-natural can't be Db).
pub fn nl_midi_to_half_ln(note_num: u8, e_acc: u8, mid_c_half_ln: i16) -> Option<i16> {
    // Brute-force lookup table: [accidental_index][pitch_class] → half-line offset
    // Index: eAcc - AC_DBLFLAT (so 0=dblflat, 1=flat, 2=natural, 3=sharp, 4=dblsharp)
    //
    // Reference: NotelistOpen.cp lines 583-597
    #[rustfmt::skip]
    static HL_TABLE: [[i8; 12]; 5] = [
        // AC_DBLFLAT:   Dbb C#  Ebb Fbb E   Gbb Gb  Abb Ab  Bbb Cbb B
        [  1,  XX,  2,   3,  XX,  4,  XX,  5,  XX,  6,   7, XX ],
        // AC_FLAT:      C   Db  D   Eb  Fb  F   Gb  G   Ab  A   Bb  Cb
        [ XX,   1, XX,   2,   3, XX,   4, XX,   5, XX,   6,  7 ],
        // AC_NATURAL:   C   C#  D   D#  E   F   F#  G   G#  A   A#  B
        [  0,  XX,  1,  XX,   2,  3,  XX,  4,  XX,  5,  XX,  6 ],
        // AC_SHARP:     B#  C#  D   D#  E   E#  F#  G   G#  A   A#  B
        [ -1,   0, XX,   1,  XX,  2,   3, XX,   4, XX,   5, XX ],
        // AC_DBLSHARP:  C   Bx  Cx  D#  Dx  F   Ex  Fx  G#  Gx  A#  Ax
        [ XX,  -1,  0,  XX,   1, XX,   2,  3,  XX,  4,  XX,  5 ],
    ];

    if !(AC_DBLFLAT..=AC_DBLSHARP).contains(&e_acc) {
        return None;
    }

    let pitch_class = (note_num % 12) as usize;
    let acc_idx = (e_acc - AC_DBLFLAT) as usize;
    let half_steps = HL_TABLE[acc_idx][pitch_class];

    if half_steps == XX {
        return None;
    }

    let octave = (note_num as i16 / 12) - 5;
    let half_lines = octave * 7 + half_steps as i16;

    Some(-half_lines + mid_c_half_ln)
}

/// ClefMiddleCHalfLn — get half-line position of middle C for a given clef.
///
/// Direct port from PitchUtils.cp (line 118).
/// Returns the staff half-line number where middle C sits.
/// Top line of staff = 0, each half-line step goes down by 1.
///
/// Clef type codes (from NObjTypes.h):
///   1=TREBLE8, 2=FRVIOLIN(unused), 3=TREBLE, 4=SOPRANO, 5=MZSOPRANO,
///   6=ALTO, 7=TRTENOR, 8=TENOR, 9=BARITONE, 10=BASS, 11=BASS8B, 12=PERC
pub fn clef_middle_c_half_ln(clef_type: u8) -> i16 {
    match clef_type {
        1 => 17,  // TREBLE8_CLEF
        3 => 10,  // TREBLE_CLEF
        4 => 8,   // SOPRANO_CLEF
        5 => 6,   // MZSOPRANO_CLEF
        6 => 4,   // ALTO_CLEF
        7 => 3,   // TRTENOR_CLEF
        8 => 2,   // TENOR_CLEF
        9 => 0,   // BARITONE_CLEF
        10 => -2, // BASS_CLEF
        11 => -9, // BASS8B_CLEF
        12 => 10, // PERC_CLEF (same as treble)
        _ => 10,  // Default to treble
    }
}

/// Half-line position to DDIST Y offset from staff top.
///
/// Each half-line = half the inter-line distance.
/// In a standard 5-line staff with staff_height, inter-line = staff_height / 4.
/// Half-line 0 = top line, half-line 8 = bottom line.
pub fn half_ln_to_yd(half_ln: i16, staff_height: Ddist) -> Ddist {
    // Inter-line distance in DDIST
    let d_interline = staff_height / 4; // For 5-line staff
                                        // Each half-line = half of d_interline
    let d_half_line = d_interline / 2;
    half_ln * d_half_line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clef_middle_c_half_ln_values() {
        assert_eq!(clef_middle_c_half_ln(3), 10); // treble
        assert_eq!(clef_middle_c_half_ln(10), -2); // bass
        assert_eq!(clef_middle_c_half_ln(6), 4); // alto
        assert_eq!(clef_middle_c_half_ln(8), 2); // tenor
    }

    #[test]
    fn test_nl_midi_to_half_ln_middle_c_treble() {
        // Middle C (MIDI 60) natural in treble clef → half-line 10
        let result = nl_midi_to_half_ln(60, AC_NATURAL, 10);
        assert_eq!(result, Some(10));
    }

    #[test]
    fn test_nl_midi_to_half_ln_e4_treble() {
        // E4 (MIDI 64) natural in treble clef → half-line 8
        let result = nl_midi_to_half_ln(64, AC_NATURAL, 10);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn test_nl_midi_to_half_ln_bass_clef() {
        // Middle C (MIDI 60) natural in bass clef → half-line -2
        let result = nl_midi_to_half_ln(60, AC_NATURAL, -2);
        assert_eq!(result, Some(-2));
    }

    #[test]
    fn test_nl_midi_to_half_ln_invalid() {
        // C natural can't be Db → None
        assert!(nl_midi_to_half_ln(60, AC_FLAT, 10).is_none());
    }

    #[test]
    fn test_half_ln_to_yd() {
        // staff_height = 64 DDIST (standard), interline = 16, half-line = 8
        assert_eq!(half_ln_to_yd(0, 64), 0);
        assert_eq!(half_ln_to_yd(8, 64), 64); // bottom line of 5-line staff
        assert_eq!(half_ln_to_yd(4, 64), 32); // middle of staff
    }
}
