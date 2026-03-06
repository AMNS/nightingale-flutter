//! Duration and rhythm mathematics for music notation.
//!
//! Ported from:
//! - `Nightingale/src/CFilesBoth/SpaceTime.cp` (Code2LDur, SimpleLDur, TimeSigDur)
//! - `Nightingale/src/CFilesBoth/RhythmDur.cp` (CalcBeatDur, other timing functions)
//! - `Nightingale/src/CFilesBoth/Objects.cp` (SimplePlayDur)
//!
//! Core concepts:
//! - **PDUR ticks**: All durations measured in "play duration" units
//! - **PDURUNIT**: 15 ticks = duration of shortest note (128th note)
//! - **Whole note**: 128 * PDURUNIT = 1920 ticks
//! - **L_DUR codes**: Integer codes for note durations (BREVE=1, WHOLE=2, HALF=3, etc.)

use crate::defs::*;
use crate::limits::MAX_L_DUR;

/// Play duration unit: the duration of the shortest note (128th note) in PDUR ticks.
/// A whole note = 128 * PDURUNIT = 1920 ticks.
///
/// Source: `Nightingale/src/Precomps/defs.h:259` and `SpaceTime.cp` comments (lines 39-42)
pub const PDURUNIT: i32 = 15;

/// Duration of a whole note in PDUR ticks (128 * PDURUNIT = 1920).
///
/// Source: Calculated from `SpaceTime.cp:222-224` (l2p_durs table initialization)
pub const WHOLE_NOTE_PDUR: i32 = 1920;

// Internal lookup table for logical-to-physical duration conversion.
// Initialized at module load time using the same algorithm as C++.
// Index corresponds to l_dur codes: l2p_durs[MAX_L_DUR] = PDURUNIT = 15
// and each earlier entry is double the next: [8]=15, [7]=30, [6]=60, etc.
//
// Source: `Nightingale/src/CFilesBoth/InitNightingale.cp:222-224`
static L2P_DURS: [i32; (MAX_L_DUR + 1) as usize] = [
    0,    // Index 0: unused (WHOLEMR_L_DUR=-1, UNKNOWN_L_DUR=0)
    1920, // Index 1: BREVE_L_DUR (2 * whole = 3840, but index is 1, so 1920*2)
    1920, // Index 2: WHOLE_L_DUR (128 * 15 = 1920 ticks)
    960,  // Index 3: HALF_L_DUR
    480,  // Index 4: QTR_L_DUR (quarter note)
    240,  // Index 5: EIGHTH_L_DUR
    120,  // Index 6: SIXTEENTH_L_DUR
    60,   // Index 7: THIRTY2ND_L_DUR
    30,   // Index 8: SIXTY4TH_L_DUR
    15,   // Index 9: ONE28TH_L_DUR (= PDURUNIT)
];

/// Convert a note l_dur code and number of augmentation dots to logical duration in PDUR ticks.
///
/// The basic formula is: base_duration + (base_duration/2) + (base_duration/4) + ...
/// for each dot.
///
/// # Arguments
/// * `dur_code` - Duration code (BREVE_L_DUR through ONE28TH_L_DUR)
/// * `n_dots` - Number of augmentation dots (0, 1, 2, etc.)
///
/// # Returns
/// Duration in PDUR ticks
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
/// use nightingale_core::defs::*;
///
/// // Whole note = 1920 ticks
/// assert_eq!(code_to_l_dur(WHOLE_L_DUR, 0), 1920);
///
/// // Quarter note = 480 ticks
/// assert_eq!(code_to_l_dur(QTR_L_DUR, 0), 480);
///
/// // Dotted quarter = 480 + 240 = 720 ticks
/// assert_eq!(code_to_l_dur(QTR_L_DUR, 1), 720);
///
/// // Double-dotted quarter = 480 + 240 + 120 = 840 ticks
/// assert_eq!(code_to_l_dur(QTR_L_DUR, 2), 840);
/// ```
///
/// Source: `Nightingale/src/CFilesBoth/SpaceTime.cp:956-964`
pub fn code_to_l_dur(dur_code: i8, n_dots: u8) -> i32 {
    let idx = (dur_code as usize).min(L2P_DURS.len() - 1);
    let mut note_dur = L2P_DURS[idx];

    // Add duration for each augmentation dot
    for j in 1..=n_dots {
        let dot_idx = (dur_code + j as i8) as usize;
        if dot_idx >= L2P_DURS.len() {
            break; // Can't subdivide beyond 128th notes
        }
        note_dur += L2P_DURS[dot_idx];
    }

    note_dur
}

/// Calculate the multiplier for augmentation dots.
///
/// Returns the factor by which a note's duration is multiplied when it has `n_dots` dots.
/// - 0 dots: factor = 1.0
/// - 1 dot: factor = 1.5 (original + half)
/// - 2 dots: factor = 1.75 (original + half + quarter)
/// - 3 dots: factor = 1.875 (original + half + quarter + eighth)
///
/// # Arguments
/// * `n_dots` - Number of augmentation dots
///
/// # Returns
/// Multiplier as a floating-point value
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
///
/// assert_eq!(calc_play_dur_factor(0), 1.0);
/// assert_eq!(calc_play_dur_factor(1), 1.5);
/// assert_eq!(calc_play_dur_factor(2), 1.75);
/// ```
pub fn calc_play_dur_factor(n_dots: u8) -> f64 {
    let mut factor = 1.0;
    let mut dot_value = 0.5;

    for _ in 0..n_dots {
        factor += dot_value;
        dot_value /= 2.0;
    }

    factor
}

/// Calculate the total duration in PDUR ticks for a note with the given l_dur and dots.
///
/// This is a wrapper around `code_to_l_dur` with a more descriptive name for external callers.
///
/// # Arguments
/// * `l_dur` - Duration code (BREVE_L_DUR through ONE28TH_L_DUR)
/// * `n_dots` - Number of augmentation dots (0, 1, 2, etc.)
///
/// # Returns
/// Duration in PDUR ticks
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
/// use nightingale_core::defs::*;
///
/// // Whole note = 1920
/// assert_eq!(simple_l_dur(WHOLE_L_DUR, 0), 1920);
///
/// // Half note = 960
/// assert_eq!(simple_l_dur(HALF_L_DUR, 0), 960);
///
/// // Dotted half = 960 + 480 = 1440
/// assert_eq!(simple_l_dur(HALF_L_DUR, 1), 1440);
/// ```
///
/// Source: `Nightingale/src/CFilesBoth/SpaceTime.cp:971-983` (SimpleLDur function)
pub fn simple_l_dur(l_dur: i8, n_dots: u8) -> i32 {
    code_to_l_dur(l_dur, n_dots)
}

/// Calculate the l_dur code (duration type) for one beat given a time signature denominator.
///
/// For example:
/// - denominator 4 (quarter note gets beat) → returns QTR_L_DUR
/// - denominator 2 (half note gets beat) → returns HALF_L_DUR
/// - denominator 8 (eighth note gets beat) → returns EIGHTH_L_DUR
///
/// # Arguments
/// * `denominator` - Time signature denominator (2, 4, 8, 16, etc.)
///
/// # Returns
/// The l_dur code for one beat
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
/// use nightingale_core::defs::*;
///
/// // 4/4 time: quarter note gets the beat
/// assert_eq!(beat_l_dur(4), QTR_L_DUR);
///
/// // 6/8 time: eighth note gets the beat
/// assert_eq!(beat_l_dur(8), EIGHTH_L_DUR);
///
/// // 3/2 time: half note gets the beat
/// assert_eq!(beat_l_dur(2), HALF_L_DUR);
/// ```
///
/// Source: Derived from `Nightingale/src/CFilesBoth/RhythmDur.cp:406-414` (CalcBeatDur)
pub fn beat_l_dur(denominator: i8) -> i8 {
    // A whole note is represented by denominator=1 (though rarely used in time sigs)
    // denominator=2 → half note (WHOLE_L_DUR + 1)
    // denominator=4 → quarter note (WHOLE_L_DUR + 2)
    // denominator=8 → eighth note (WHOLE_L_DUR + 3)
    // etc.

    // Calculate how many times we need to halve a whole note
    let mut halving_count = 0;
    let mut denom = denominator;
    while denom > 1 {
        denom /= 2;
        halving_count += 1;
    }

    WHOLE_L_DUR + halving_count
}

/// Calculate the duration of one beat in PDUR ticks, given a time signature.
///
/// For simple meters (2/4, 3/4, 4/4), one beat = one denominator unit.
/// For compound meters (6/8, 9/8, 12/8), one beat = three denominator units.
///
/// # Arguments
/// * `denominator` - Time signature denominator
/// * `compound` - True if compound meter (6/8, 9/8, etc.), false for simple meters
///
/// # Returns
/// Duration of one beat in PDUR ticks
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
///
/// // 4/4: quarter note beat = 480 ticks
/// assert_eq!(beats_dur(4, false), 480);
///
/// // 6/8: dotted quarter beat = 3 * 240 = 720 ticks
/// assert_eq!(beats_dur(8, true), 720);
/// ```
///
/// Source: `Nightingale/src/CFilesBoth/RhythmDur.cp:406-414` (CalcBeatDur)
pub fn beats_dur(denominator: i8, compound: bool) -> i32 {
    let whole_dur = L2P_DURS[WHOLE_L_DUR as usize];
    let mut beat_dur = whole_dur / (denominator as i32);

    if compound {
        beat_dur *= 3;
    }

    beat_dur
}

/// Calculate the total duration of a measure in PDUR ticks.
///
/// The formula is: `(numerator * whole_note_duration) / denominator`
///
/// # Arguments
/// * `numerator` - Time signature numerator (beats per measure)
/// * `denominator` - Time signature denominator (note value that gets one beat)
///
/// # Returns
/// Measure duration in PDUR ticks
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
///
/// // 4/4: 4 quarter notes = 4 * 480 = 1920 ticks (same as whole note)
/// assert_eq!(measure_dur(4, 4), 1920);
///
/// // 3/4: 3 quarter notes = 3 * 480 = 1440 ticks
/// assert_eq!(measure_dur(3, 4), 1440);
///
/// // 6/8: 6 eighth notes = 6 * 240 = 1440 ticks (same as 3/4)
/// assert_eq!(measure_dur(6, 8), 1440);
///
/// // 2/2: 2 half notes = 2 * 960 = 1920 ticks
/// assert_eq!(measure_dur(2, 2), 1920);
/// ```
///
/// Source: `Nightingale/src/CFilesBoth/SpaceTime.cp:1115-1120` (TimeSigDur)
pub fn measure_dur(numerator: i8, denominator: i8) -> i32 {
    let whole_dur = L2P_DURS[WHOLE_L_DUR as usize];
    (numerator as i32 * whole_dur) / (denominator as i32)
}

/// Calculate the number of beats in a measure.
///
/// For simple meters, this is just the numerator.
/// For compound meters (6/8, 9/8, 12/8), divide by 3 to get the number of beats.
///
/// # Arguments
/// * `numerator` - Time signature numerator
/// * `compound` - True if compound meter
///
/// # Returns
/// Number of beats per measure
///
/// # Examples
/// ```
/// use nightingale_core::duration::*;
///
/// // 4/4: 4 beats per measure
/// assert_eq!(beats_per_measure(4, false), 4);
///
/// // 6/8: 2 beats per measure (6 eighths ÷ 3 = 2 dotted quarters)
/// assert_eq!(beats_per_measure(6, true), 2);
///
/// // 9/8: 3 beats per measure
/// assert_eq!(beats_per_measure(9, true), 3);
/// ```
pub fn beats_per_measure(numerator: i8, compound: bool) -> i8 {
    if compound {
        numerator / 3
    } else {
        numerator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2p_durs_table() {
        // Verify the lookup table initialization matches C++ behavior
        assert_eq!(L2P_DURS[ONE28TH_L_DUR as usize], PDURUNIT);
        assert_eq!(L2P_DURS[WHOLE_L_DUR as usize], WHOLE_NOTE_PDUR);

        // Each entry should be double the next (for undotted durations)
        assert_eq!(
            L2P_DURS[SIXTY4TH_L_DUR as usize] * 2,
            L2P_DURS[THIRTY2ND_L_DUR as usize]
        );
        assert_eq!(
            L2P_DURS[THIRTY2ND_L_DUR as usize] * 2,
            L2P_DURS[SIXTEENTH_L_DUR as usize]
        );
        assert_eq!(
            L2P_DURS[SIXTEENTH_L_DUR as usize] * 2,
            L2P_DURS[EIGHTH_L_DUR as usize]
        );
        assert_eq!(
            L2P_DURS[EIGHTH_L_DUR as usize] * 2,
            L2P_DURS[QTR_L_DUR as usize]
        );
        assert_eq!(
            L2P_DURS[QTR_L_DUR as usize] * 2,
            L2P_DURS[HALF_L_DUR as usize]
        );
        assert_eq!(
            L2P_DURS[HALF_L_DUR as usize] * 2,
            L2P_DURS[WHOLE_L_DUR as usize]
        );
    }

    #[test]
    fn test_code_to_l_dur_undotted() {
        // Test basic undotted durations
        assert_eq!(code_to_l_dur(WHOLE_L_DUR, 0), 1920);
        assert_eq!(code_to_l_dur(HALF_L_DUR, 0), 960);
        assert_eq!(code_to_l_dur(QTR_L_DUR, 0), 480);
        assert_eq!(code_to_l_dur(EIGHTH_L_DUR, 0), 240);
        assert_eq!(code_to_l_dur(SIXTEENTH_L_DUR, 0), 120);
        assert_eq!(code_to_l_dur(THIRTY2ND_L_DUR, 0), 60);
        assert_eq!(code_to_l_dur(SIXTY4TH_L_DUR, 0), 30);
        assert_eq!(code_to_l_dur(ONE28TH_L_DUR, 0), 15);
    }

    #[test]
    fn test_code_to_l_dur_dotted() {
        // Dotted quarter = 480 + 240 = 720
        assert_eq!(code_to_l_dur(QTR_L_DUR, 1), 720);

        // Dotted half = 960 + 480 = 1440
        assert_eq!(code_to_l_dur(HALF_L_DUR, 1), 1440);

        // Dotted eighth = 240 + 120 = 360
        assert_eq!(code_to_l_dur(EIGHTH_L_DUR, 1), 360);

        // Double-dotted quarter = 480 + 240 + 120 = 840
        assert_eq!(code_to_l_dur(QTR_L_DUR, 2), 840);

        // Double-dotted half = 960 + 480 + 240 = 1680
        assert_eq!(code_to_l_dur(HALF_L_DUR, 2), 1680);
    }

    #[test]
    fn test_simple_l_dur() {
        // Wrapper function should give same results
        assert_eq!(simple_l_dur(QTR_L_DUR, 0), 480);
        assert_eq!(simple_l_dur(QTR_L_DUR, 1), 720);
        assert_eq!(simple_l_dur(HALF_L_DUR, 0), 960);
        assert_eq!(simple_l_dur(WHOLE_L_DUR, 0), 1920);
    }

    #[test]
    fn test_calc_play_dur_factor() {
        assert_eq!(calc_play_dur_factor(0), 1.0);
        assert_eq!(calc_play_dur_factor(1), 1.5);
        assert_eq!(calc_play_dur_factor(2), 1.75);
        assert_eq!(calc_play_dur_factor(3), 1.875);
    }

    #[test]
    fn test_beat_l_dur() {
        // denominator 2 = half note
        assert_eq!(beat_l_dur(2), HALF_L_DUR);

        // denominator 4 = quarter note
        assert_eq!(beat_l_dur(4), QTR_L_DUR);

        // denominator 8 = eighth note
        assert_eq!(beat_l_dur(8), EIGHTH_L_DUR);

        // denominator 16 = sixteenth note
        assert_eq!(beat_l_dur(16), SIXTEENTH_L_DUR);
    }

    #[test]
    fn test_beats_dur() {
        // 4/4 time: quarter note beat
        assert_eq!(beats_dur(4, false), 480);

        // 6/8 time: dotted quarter beat (3 eighths)
        assert_eq!(beats_dur(8, true), 720);

        // 3/2 time: half note beat
        assert_eq!(beats_dur(2, false), 960);

        // 9/8 time: dotted quarter beat
        assert_eq!(beats_dur(8, true), 720);
    }

    #[test]
    fn test_measure_dur() {
        // 4/4: 4 quarter notes = 1920 (same as whole note)
        assert_eq!(measure_dur(4, 4), 1920);

        // 3/4: 3 quarter notes = 1440
        assert_eq!(measure_dur(3, 4), 1440);

        // 6/8: 6 eighth notes = 1440 (same pdur as 3/4)
        assert_eq!(measure_dur(6, 8), 1440);

        // 2/2: 2 half notes = 1920
        assert_eq!(measure_dur(2, 2), 1920);

        // 12/8: 12 eighth notes = 2880
        assert_eq!(measure_dur(12, 8), 2880);

        // 5/4: 5 quarter notes = 2400
        assert_eq!(measure_dur(5, 4), 2400);
    }

    #[test]
    fn test_beats_per_measure() {
        // Simple meters: beats = numerator
        assert_eq!(beats_per_measure(4, false), 4);
        assert_eq!(beats_per_measure(3, false), 3);
        assert_eq!(beats_per_measure(2, false), 2);

        // Compound meters: beats = numerator / 3
        assert_eq!(beats_per_measure(6, true), 2);
        assert_eq!(beats_per_measure(9, true), 3);
        assert_eq!(beats_per_measure(12, true), 4);
    }

    #[test]
    fn test_duration_relationships() {
        // Two half notes = one whole note
        assert_eq!(
            code_to_l_dur(HALF_L_DUR, 0) * 2,
            code_to_l_dur(WHOLE_L_DUR, 0)
        );

        // Four quarter notes = one whole note
        assert_eq!(
            code_to_l_dur(QTR_L_DUR, 0) * 4,
            code_to_l_dur(WHOLE_L_DUR, 0)
        );

        // Eight eighth notes = one whole note
        assert_eq!(
            code_to_l_dur(EIGHTH_L_DUR, 0) * 8,
            code_to_l_dur(WHOLE_L_DUR, 0)
        );

        // A dotted note is 1.5x the undotted version
        assert_eq!(
            code_to_l_dur(QTR_L_DUR, 1),
            code_to_l_dur(QTR_L_DUR, 0) * 3 / 2
        );
        assert_eq!(
            code_to_l_dur(HALF_L_DUR, 1),
            code_to_l_dur(HALF_L_DUR, 0) * 3 / 2
        );
    }

    #[test]
    fn test_triplet_math() {
        // Three triplet eighths in the time of one quarter
        // Each triplet eighth = 480 / 3 = 160 ticks
        let quarter_dur = code_to_l_dur(QTR_L_DUR, 0);
        let triplet_eighth_dur = quarter_dur / 3;
        assert_eq!(triplet_eighth_dur, 160);

        // Three triplet eighths = one quarter
        assert_eq!(triplet_eighth_dur * 3, quarter_dur);
    }
}
