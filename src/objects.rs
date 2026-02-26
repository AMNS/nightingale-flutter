//! Object manipulation — port of Objects.cp.
//!
//! Shared functions for stem direction, chord processing, and key signature
//! setup. Used by both the Notelist and NGL pipelines.
//!
//! Reference: Nightingale/src/CFilesBoth/Objects.cp

use crate::basic_types::*;
use crate::utility::{calc_ystem, nflags};

/// Voice role constants from Multivoice.h.
/// Determines stem direction and stem length for each voice.
///
/// This enum is re-exported from `notelist::to_score::VoiceRole` for
/// backward compatibility; the canonical definition remains there for now
/// since it's tightly coupled with the notelist pipeline. When the NGL
/// pipeline also needs voice roles, this should become the canonical location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceRole {
    /// Single voice on staff (VCROLE_SINGLE = 6 in OG).
    Single,
    /// Upper voice in multi-voice notation (VCROLE_UPPER = 3 in OG).
    Upper,
    /// Lower voice in multi-voice notation (VCROLE_LOWER = 4 in OG).
    Lower,
}

// ===========================================================================
// NormalStemUpDown — stem direction for single notes
// Port of Objects.cp:1457-1497
// ===========================================================================

/// Determine stem direction for a single note based on voice role and position.
///
/// Port of NormalStemUpDown from Objects.cp (lines 1457-1497).
///
/// - VCROLE_SINGLE: position-based (above midline = stem down, below = stem up)
/// - VCROLE_UPPER: always stem up
/// - VCROLE_LOWER: always stem down
///
/// Returns `true` if stem should point down.
pub fn normal_stem_up_down_single(half_ln: i16, staff_lines: i16, role: VoiceRole) -> bool {
    match role {
        VoiceRole::Upper => false,
        VoiceRole::Lower => true,
        VoiceRole::Single => {
            // Notes on or above the middle line get stems down.
            // Middle line of 5-line staff = half-line 4 (= staffLines - 1).
            half_ln < staff_lines
        }
    }
}

/// Determine stem direction for a chord based on voice role and extreme notes.
///
/// Port of NormalStemUpDown for chords from Objects.cp (lines 1594-1633).
///
/// - VCROLE_UPPER: always stem up
/// - VCROLE_LOWER: always stem down
/// - VCROLE_SINGLE: compare distance of extreme notes from midline
///
/// `min_yd` = highest note (smallest Y), `max_yd` = lowest note (largest Y).
/// Returns `true` if stem should point down.
pub fn normal_stem_up_down_chord(
    min_yd: Ddist,
    max_yd: Ddist,
    staff_height: Ddist,
    role: VoiceRole,
) -> bool {
    match role {
        VoiceRole::Upper => false,
        VoiceRole::Lower => true,
        VoiceRole::Single => {
            let mid_line = staff_height / 2;
            // Compare: how far is the lowest note below midline vs
            // how far is the highest note above midline.
            // If the lowest is closer to (or above) midline, stem goes down.
            (max_yd as i32 - mid_line as i32) <= (mid_line as i32 - min_yd as i32)
        }
    }
}

// ===========================================================================
// Chord stem processing — port of FixChordForYStem / GetNCYStem
// Port of Objects.cp:1674-1744
// ===========================================================================

/// Compute the stem endpoint for a chord.
///
/// Port of GetNCYStem from Objects.cp (lines 1674-1680).
///
/// Finds the "far note" (furthest from middle in stem direction) and
/// computes CalcYStem from that position.
///
/// Returns `(far_idx, ystem)` — index of the far note and the stem endpoint.
pub fn get_nc_ystem(
    yd_values: &[Ddist],
    l_durs: &[i8],
    stem_down: bool,
    staff_height: Ddist,
    staff_lines: i16,
    qtr_sp: i16,
) -> (usize, Ddist) {
    if yd_values.is_empty() {
        return (0, 0);
    }

    // Far note = stem_down ? lowest (max yd) : highest (min yd)
    let far_idx = if stem_down {
        yd_values
            .iter()
            .enumerate()
            .max_by_key(|(_, &yd)| yd)
            .map(|(i, _)| i)
            .unwrap_or(0)
    } else {
        yd_values
            .iter()
            .enumerate()
            .min_by_key(|(_, &yd)| yd)
            .map(|(i, _)| i)
            .unwrap_or(0)
    };

    let far_yd = yd_values[far_idx];
    let far_dur = l_durs[far_idx];

    let ystem = if far_dur >= 3 {
        calc_ystem(
            far_yd,
            nflags(far_dur),
            stem_down,
            staff_height,
            staff_lines,
            qtr_sp,
            false,
        )
    } else {
        far_yd
    };

    (far_idx, ystem)
}

// ===========================================================================
// Key signature setup — port of SetupKeySig
// Port of Objects.cp:1083-1144
// ===========================================================================

/// Build a KsInfo from number of accidentals and sharp/flat flag.
///
/// Port of SetupKeySig from Objects.cp (lines 1083-1144).
/// Circle-of-fifths order: sharps = F C G D A E B, flats = B E A D G C F.
pub fn setup_ks_info(n_items: u8, is_sharp: bool) -> KsInfo {
    const SHARP_ORDER: [i8; 7] = [0, 3, 6, 2, 5, 1, 4]; // F C G D A E B
    const FLAT_ORDER: [i8; 7] = [4, 1, 5, 2, 6, 3, 0]; // B E A D G C F
    let mut ks = KsInfo {
        n_ks_items: n_items as i8,
        ..KsInfo::default()
    };
    let order = if is_sharp { &SHARP_ORDER } else { &FLAT_ORDER };
    for (k, &letcode) in order.iter().enumerate().take(n_items.min(7) as usize) {
        ks.ks_item[k] = KsItem {
            letcode,
            sharp: u8::from(is_sharp),
        };
    }
    ks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stem_direction_single_voice() {
        // Above midline (half_ln 2, staffLines 5) → stem down = false (above mid)
        // Actually: half_ln < staffLines means stem down is false? No:
        // half_ln < staff_lines → half_ln=2, staff_lines=5 → 2 < 5 → true → stem NOT down
        // Wait, the function returns true for stem_down.
        // half_ln < staff_lines means the note is ABOVE the midline → stem DOWN
        assert!(normal_stem_up_down_single(2, 5, VoiceRole::Single));
        // Below midline → stem up
        assert!(!normal_stem_up_down_single(6, 5, VoiceRole::Single));
    }

    #[test]
    fn test_stem_direction_upper_lower() {
        assert!(!normal_stem_up_down_single(0, 5, VoiceRole::Upper)); // always up
        assert!(normal_stem_up_down_single(0, 5, VoiceRole::Lower)); // always down
    }

    #[test]
    fn test_chord_stem_direction() {
        // Chord spanning from top to middle: stem down
        assert!(normal_stem_up_down_chord(0, 32, 64, VoiceRole::Single));
        // Chord centered below midline: stem up
        assert!(!normal_stem_up_down_chord(40, 60, 64, VoiceRole::Single));
    }

    #[test]
    fn test_setup_ks_info_sharp() {
        let ks = setup_ks_info(2, true);
        assert_eq!(ks.n_ks_items, 2);
        assert_eq!(ks.ks_item[0].letcode, 0); // F
        assert_eq!(ks.ks_item[0].sharp, 1);
        assert_eq!(ks.ks_item[1].letcode, 3); // C
    }

    #[test]
    fn test_setup_ks_info_flat() {
        let ks = setup_ks_info(3, false);
        assert_eq!(ks.n_ks_items, 3);
        assert_eq!(ks.ks_item[0].letcode, 4); // B
        assert_eq!(ks.ks_item[0].sharp, 0);
        assert_eq!(ks.ks_item[1].letcode, 1); // E
        assert_eq!(ks.ks_item[2].letcode, 5); // A
    }
}
