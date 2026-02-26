//! Object manipulation — port of Objects.cp.
//!
//! Shared functions for stem direction, chord processing, and key signature
//! setup. Used by both the Notelist and NGL pipelines.
//!
//! Reference: Nightingale/src/CFilesBoth/Objects.cp

use crate::basic_types::*;
use crate::utility::{calc_ystem, nflags, DFLT_XMOVEACC};

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

// ===========================================================================
// ArrangeChordNotes — compute otherStemSide for seconds in chords
// Port of PitchUtils.cp:1583-1616
// ===========================================================================

/// Compute `other_stem_side` flags for a list of note yd values in one chord.
///
/// Port of PitchUtils.cp ArrangeChordNotes() (line 1583-1616).
///
/// Takes yd values (in DDIST) and the half-line height (staff_height / 8).
/// Sets `other_stem_side` = true for notes that should be placed on the "wrong"
/// side of the stem due to a second interval (adjacent notes that would collide).
///
/// The OG uses yqpit (quarter-tone units) and checks `|delta| == QD_SECOND (2)`.
/// Our equivalent: yd values in DDIST, where one diatonic step = `half_ln` DDIST.
///
/// Algorithm:
/// 1. Sort notes by yd, starting from the extreme note furthest from stem end
///    (lowest yd for stems-down, highest yd for stems-up)
/// 2. Walk through sorted notes; first note always on normal side
/// 3. For each interval of a second (|yd delta| == half_ln): toggle side
/// 4. For intervals >= third: reset to normal side
///
/// Returns a Vec of booleans indexed by the original note order, indicating
/// which notes should have `other_stem_side = true`.
pub fn arrange_chord_notes(yds: &[i16], stem_down: bool, half_ln: i16) -> Vec<bool> {
    let n = yds.len();
    if n < 2 {
        return vec![false; n];
    }

    // Build (yd, original_index) pairs and sort.
    // Stem-down: sort ascending yd (highest note first = closest to stem end last).
    //   OG scans from extreme note (furthest from stem end) which is lowest yd for stem-down.
    // Stem-up: sort descending yd (lowest note first = closest to stem end last).
    //   OG scans from extreme note which is highest yd for stem-up.
    let mut sorted: Vec<(i16, usize)> = yds
        .iter()
        .copied()
        .enumerate()
        .map(|(i, y)| (y, i))
        .collect();
    if stem_down {
        sorted.sort_by_key(|&(y, _)| y); // ascending: top notes first (far from stem)
    } else {
        sorted.sort_by_key(|&(y, _)| std::cmp::Reverse(y)); // descending: bottom notes first
    }

    let mut result = vec![false; n];
    let mut other_side = false;
    let mut prev_yd = sorted[0].0;

    for &(yd, orig_idx) in sorted.iter().skip(1) {
        let delta = (yd - prev_yd).abs();
        // A "second" = exactly one diatonic step = one half-line of staff spacing.
        // OG: |yqpit delta| == QD_SECOND (2 quarter-tone units)
        // Ours: |yd delta| == half_ln DDIST
        if delta > 0 && delta <= half_ln {
            other_side = !other_side; // Toggle for seconds
        } else {
            other_side = false; // Reset for thirds or larger
        }
        result[orig_idx] = other_side;
        prev_yd = yd;
    }

    result
}

/// Default horizontal step for accidental staggering.
/// Port of HACCSTEP_DFLT from Initialize.cp:938.
const HACCSTEP: i16 = 4;

/// Compute `xmove_acc` values for each note in a chord.
///
/// Port of ArrangeNCAccs from PitchUtils.cp:1517-1572.
///
/// The algorithm creates a "pyramid" stagger pattern: the middle accidental
/// is pushed furthest left, with accidentals above and below it progressively
/// closer to the noteheads. This works well for small chords (2-4 accidentals).
///
/// Arguments:
/// - `notes`: slice of (yd, accident) pairs in the chord's display order
///   (sorted from extreme note toward stem end)
/// - `stem_down`: true if stem goes down
///
/// Returns a Vec of `xmove_acc` values (0-31) indexed by position in `notes`.
pub fn arrange_nc_accs(notes: &[(i16, u8)], stem_down: bool) -> Vec<u8> {
    let n = notes.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![DFLT_XMOVEACC as u8];
    }

    // Count accidentals and find the closest interval between two accidentals.
    // OG uses yqpit (quarter-pitch units); we use yd (DDIST).
    // QD_SECOND in the OG = 2 quarter-pitch units = one diatonic step.
    // The threshold "6 * QD_SECOND" = 12 qd = "less than a 7th apart".
    // In our coordinate system: 6 diatonic steps * half_ln DDIST per step.
    // But since we're comparing yd values directly, and yd = half_ln * halflines,
    // we need the half_ln to compute the threshold. Instead, we can count
    // the diatonic interval directly from yd differences.
    //
    // However, the OG code compares yqpit (which is in quarter-spaces, 2 per step).
    // For simplicity, we replicate the logic using sorted yd positions and
    // compute the threshold in terms of the actual yd spacing.

    let mut acc_count = 0i16;
    let mut closest_yd: i32 = i32::MAX;
    let mut prev_acc_yd: i32 = 0;
    let mut has_prev_acc = false;

    for &(yd, accident) in notes {
        if accident != 0 {
            if has_prev_acc {
                let diff = (yd as i32 - prev_acc_yd).abs();
                if diff < closest_yd {
                    closest_yd = diff;
                }
            }
            acc_count += 1;
            prev_acc_yd = yd as i32;
            has_prev_acc = true;
        }
    }

    if acc_count == 0 {
        return vec![DFLT_XMOVEACC as u8; n];
    }

    let max_step = acc_count / 2;
    let mid_acc = if acc_count % 2 == 0 && !stem_down {
        max_step - 1
    } else {
        max_step
    };

    // Threshold: accidentals more than ~6 diatonic steps apart don't need staggering.
    // We can't know half_ln here, so we use the yd values directly.
    // The notes are already sorted, so we check if any consecutive pair of
    // accidentals is "close". We'll check against 6 half-lines, approximated
    // from the yd spacing between consecutive notes in the sorted list.
    //
    // Actually, the simplest approach: if we have the minimum yd gap between
    // accidentals, we compare it against a threshold. The OG threshold is
    // 6 * QD_SECOND = 12 in yqpit terms. In yd terms for NGL (half_ln=8):
    // 6*8=48. For Notelist (half_ln=48): 6*48=288.
    // Rather than requiring half_ln, we can check: if all accidental pairs
    // are >= 6 * (first detected step size), use defaults.
    //
    // Simpler: compute half_ln from the actual yd data. The minimum non-zero
    // interval between any two notes gives us half_ln.
    let mut min_step: i32 = i32::MAX;
    if notes.len() >= 2 {
        for i in 1..notes.len() {
            let diff = (notes[i].0 as i32 - notes[i - 1].0 as i32).abs();
            if diff > 0 && diff < min_step {
                min_step = diff;
            }
        }
    }
    // If no useful step found, default everything
    if min_step == i32::MAX {
        return vec![DFLT_XMOVEACC as u8; n];
    }

    // The threshold is 6 * half_ln (6 diatonic steps = a 7th)
    let threshold = 6 * min_step;
    let needs_stagger = closest_yd < threshold;

    let mut result = vec![DFLT_XMOVEACC as u8; n];
    let mut acc_so_far = 0i16;

    for (i, &(_yd, accident)) in notes.iter().enumerate() {
        if needs_stagger {
            let diff = max_step - (mid_acc - acc_so_far).abs();
            let xmove = (DFLT_XMOVEACC + HACCSTEP * diff).min(31);
            result[i] = xmove as u8;
        }
        // else: result[i] already = DFLT_XMOVEACC
        if accident != 0 {
            acc_so_far += 1;
        }
    }

    result
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

    #[test]
    fn test_arrange_chord_notes_no_seconds() {
        // C-E-G: thirds only, no seconds → all false
        // yd values: C=32, E=16, G=0 (8 DDIST per half-line)
        let yds = vec![32, 16, 0];
        let result = arrange_chord_notes(&yds, true, 8); // stem down, half_ln=8
        assert_eq!(result, vec![false, false, false]);
    }

    #[test]
    fn test_arrange_chord_notes_with_second() {
        // C-D: second → second note on other side
        // yd: C=32, D=24 (delta=8 = one half-line)
        let yds = vec![32, 24];
        let result = arrange_chord_notes(&yds, false, 8); // stem up, half_ln=8
                                                          // Stem up: sort descending (start from bottom=32, then 24).
                                                          // 32 → normal, 24 → second from 32 → toggle
        assert_eq!(result, vec![false, true]);
    }

    #[test]
    fn test_arrange_chord_notes_cluster() {
        // C-D-E cluster (all seconds): alternating
        // yd: C=32, D=24, E=16 (stem down)
        let yds = vec![32, 24, 16];
        let result = arrange_chord_notes(&yds, true, 8); // half_ln=8
                                                         // Stem down: sort ascending (start from top=16, then 24, then 32).
                                                         // 16 → normal, 24 → second from 16 → toggle, 32 → second from 24 → toggle back
        assert_eq!(result, vec![false, true, false]);
    }

    #[test]
    fn test_arrange_chord_notes_single() {
        let yds = vec![32];
        let result = arrange_chord_notes(&yds, true, 8);
        assert_eq!(result, vec![false]);
    }

    #[test]
    fn test_arrange_chord_notes_large_staff() {
        // Same C-D second but with staff_height=384 (notelist default)
        // half_ln = 384/8 = 48. C yd=432, D yd=384 (delta=48 = one half-line)
        let yds = vec![432, 384];
        let result = arrange_chord_notes(&yds, false, 48); // stem up, half_ln=48
        assert_eq!(result, vec![false, true]);
    }
}
