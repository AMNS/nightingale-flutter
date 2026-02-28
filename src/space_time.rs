//! Spacing and time utilities — port of SpaceTime.cp + SpaceHighLevel.cp.
//!
//! Duration-proportional spacing tables and functions used for measure layout.
//! Includes the full Gourlay spacing pipeline: ideal spacing from duration,
//! collision-avoidance width computation (SymWidthLeft/Right), and Respace1Bar.
//!
//! Reference: Nightingale/src/CFilesBoth/SpaceTime.cp, SpaceHighLevel.cp

use crate::basic_types::Ddist;
use crate::defs::{
    grace_size, BAR_DOUBLE, BAR_FINALDBL, BAR_RPT_L, BAR_RPT_LR, BAR_RPT_R, BAR_SINGLE, RESFACTOR,
};
use crate::utility::{nflags, DFLT_XMOVEACC, STD_ACCWIDTH};

/// Nightingale's ideal spacing table (dfltSpaceMap from SpaceTime.cp line 701).
///
/// Fibonacci/sqrt(2) progression. Index: 0=128th .. 8=breve.
/// Values are multiplied by STD_LINEHT to get STDIST.
pub const IDEAL_SPACE_MAP: [f32; 9] = [0.625, 1.0, 1.625, 2.50, 3.75, 5.50, 8.00, 11.5, 16.25];

/// STD_LINEHT as float for spacing calculations.
const STD_LINEHT_F: f32 = 8.0;

/// Convert l_dur to ideal space in STDIST (port of IdealSpace).
///
/// Port of IdealSpace from SpaceTime.cp (line 956-964).
///
/// l_dur: BREVE=1, WHOLE=2, HALF=3, QTR=4, 8TH=5, 16TH=6, 32ND=7, 64TH=8
pub fn ideal_space_stdist(l_dur: i8) -> f32 {
    // Map l_dur to spaceMap index (reverse order)
    let idx: usize = match l_dur {
        1 => 8,
        2 => 7,
        3 => 6,
        4 => 5,
        5 => 4,
        6 => 3,
        7 => 2,
        8 => 1,
        _ => 0,
    };
    IDEAL_SPACE_MAP[idx] * STD_LINEHT_F
}

/// Fine ideal space from physical duration in PDUR ticks.
///
/// Port of FIdealSpace from SpaceTime.cp:739-774.
/// Takes duration in PDUR ticks (e.g., 480 for quarter note) and returns
/// fine STDIST (10x resolution) for proportional spacing.
///
/// The function interpolates between table entries for non-power-of-2 durations
/// (e.g., dotted notes, tuplets). Uses the same Fibonacci/sqrt(2) progression
/// as `ideal_space_stdist` but works from actual tick counts rather than l_dur codes.
pub fn f_ideal_space(pdur_ticks: i32) -> f32 {
    if pdur_ticks <= 0 {
        return 0.0;
    }

    // Convert to "128th-note units" (divide by PDURUNIT=15)
    let dur = pdur_ticks / crate::duration::PDURUNIT;
    if dur <= 0 {
        return 0.0;
    }

    // Walk powers of 2 to find which table bracket the duration falls in.
    // Table index 0 = shortest (128th), index 8 = longest (breve).
    // two2i tracks 1, 2, 4, 8, ... = 128th counts for each table entry.
    let mut last_two2i: i32 = 0;
    let mut two2i: i32 = 1;
    for i in 0..9 {
        if dur < two2i {
            // Interpolate between table[i-1] and table[i]
            if i > 0 && last_two2i < two2i {
                let y0 = IDEAL_SPACE_MAP[i - 1] * STD_LINEHT_F;
                let y1 = IDEAL_SPACE_MAP[i] * STD_LINEHT_F;
                let t = (dur - last_two2i) as f32 / (two2i - last_two2i) as f32;
                return FIDEAL_RESOLVE * (y0 + t * (y1 - y0));
            }
            return FIDEAL_RESOLVE * IDEAL_SPACE_MAP[i] * STD_LINEHT_F;
        } else if dur == two2i {
            return FIDEAL_RESOLVE * IDEAL_SPACE_MAP[i] * STD_LINEHT_F;
        }
        last_two2i = two2i;
        two2i *= 2;
    }

    // Duration longer than breve — use breve space
    FIDEAL_RESOLVE * IDEAL_SPACE_MAP[8] * STD_LINEHT_F
}

/// Fine STDIST resolution factor (10 parts per STDIST).
/// Source: defs.h:255
const FIDEAL_RESOLVE: f32 = 10.0;

/// Convert fine STDIST (10x resolution) to normal STDIST.
///
/// Port of IdealSpace from SpaceTime.cp:778-785.
pub fn ideal_space_pdur(pdur_ticks: i32) -> f32 {
    f_ideal_space(pdur_ticks) / FIDEAL_RESOLVE
}

/// Convert STDIST (as float) to DDIST for standard 5-line staff.
///
/// Port of std2d for spacing context:
///   std2d(s, staffHeight, staffLines) = s * staffHeight / (STD_LINEHT * (staffLines-1))
///
/// For staffHeight=384, staffLines=5: 1 STDIST = 384 / 32 = 12 DDIST
pub fn stdist_to_ddist(stdist: f32, staff_height: Ddist) -> Ddist {
    (stdist * staff_height as f32 / (STD_LINEHT_F * 4.0)) as Ddist
}

// ============================================================================
// OG Nightingale spacing pipeline — port of SpaceTime.cp + SpaceHighLevel.cp
// ============================================================================
//
// The Gourlay spacing algorithm works in these stages:
//   1. Build SpaceTimeInfo array for each measure (start times, just types)
//   2. GetSpaceInfo: compute controlling duration + fraction for each event
//   3. Initialize fSpBefore[] with ideal duration-based spacing
//   4. ConsiderITWidths: collision avoidance using SymWidthLeft/Right
//   5. Convert to STDIST positions
//   6. Scale to fit system width (done at caller level)

/// STD_LINEHT as i16 (duplicated from basic_types for local use).
const STD_LINEHT: i16 = 8;

// --- Spacing config constants (OG Initialize.cp:933-937) ---
// Values in STDIST = eighth-spaces of an interline space.

/// Space after barline before first Sync (STDIST).
/// Port of config.spAfterBar from Initialize.cp:933.
pub const CONFIG_SP_AFTER_BAR: i16 = 6;

/// Minimum space after barline for non-Sync first object (STDIST).
/// Port of config.minSpAfterBar.
const CONFIG_MIN_SP_AFTER_BAR: i16 = 5;

/// Minimum space before barline (STDIST).
/// Port of config.minSpBeforeBar.
const CONFIG_MIN_SP_BEFORE_BAR: i16 = 5;

/// Minimum space between symbols (STDIST).
/// Port of config.minRSpace.
const CONFIG_MIN_R_SPACE: i16 = 2;

/// Empty measure width (STDIST).
/// Port of EMPTYMEAS_WIDTH (SpaceHighLevel.cp).
const EMPTY_MEAS_WIDTH: i16 = 40;

// --- Fine STDIST conversion (SpaceHighLevel.cp:290-291) ---

/// FIDEAL_RESOLVE as i32 for integer operations.
const FIDEAL_RESOLVE_I: i32 = 10;

/// Convert STDIST to fine STDIST (×10).
/// Port of STD2F macro.
#[inline]
fn std2f(sd: i16) -> i32 {
    sd as i32 * FIDEAL_RESOLVE_I
}

/// Convert fine STDIST to STDIST (÷10).
/// Port of F2STD macro.
#[inline]
fn f2std(fd: i32) -> i16 {
    (fd / FIDEAL_RESOLVE_I) as i16
}

// ============================================================================
// SpaceTimeInfo — per-object spacing data within a measure
// ============================================================================

/// Justification type constants.
/// Port of NMiscTypes.h:16-21.
pub const J_IT: u8 = 1; // Independent, totally ordered (Syncs)
pub const J_IP: u8 = 2; // Independent, partially ordered (GrSyncs, mid-measure clefs)
pub const J_STRUC: u8 = 4; // Structural (measure terminator)

/// Spacing information for one object in a measure.
/// Port of SPACETIMEINFO from SpaceHighLevel.cp.
#[derive(Clone, Debug)]
pub struct SpaceTimeInfo {
    /// Index into the measure's object list.
    pub index: usize,
    /// Logical time in PDUR ticks from measure start.
    pub start_time: i32,
    /// Controlling duration (PDUR ticks) — set by get_space_info.
    pub dur: i32,
    /// Fraction of dur before the next event — set by get_space_info.
    pub frac: f32,
    /// Is this a Sync (note/rest)?
    pub is_sync: bool,
    /// Justification type (J_IT, J_IP, J_STRUC).
    pub just_type: u8,
    /// STDIST width to the left of origin (accidentals, etc.).
    pub width_left: i16,
    /// STDIST width to the right of origin (notehead + flags + dots).
    pub width_right: i16,
}

// ============================================================================
// NoteWidthInfo — per-note data for width calculations
// ============================================================================

/// Information about a single note needed for spacing width calculations.
/// Populated from ANote data during layout.
#[derive(Clone, Debug)]
pub struct NoteWidthInfo {
    /// Duration code (BREVE=1..128TH=8).
    pub l_dur: i8,
    /// Number of augmentation dots.
    pub ndots: u8,
    /// Is this a rest?
    pub rest: bool,
    /// Is this note beamed?
    pub beamed: bool,
    /// Stem goes up? (yd > ystem in Nightingale's downward-Y convention).
    pub stem_up: bool,
    /// xMoveDots offset (quarter-spaces from origin; default = 3).
    pub x_move_dots: u8,
    /// Accidental code (0=none, 1=dbl-flat..5=dbl-sharp).
    pub acc: u8,
    /// xMoveAcc offset (quarter-spaces; default = DFLT_XMOVEACC=5).
    pub xmove_acc: u8,
    /// Has a courtesy accidental?
    pub courtesy_acc: bool,
    /// Note is to the left of stem (downstem chord with seconds).
    pub note_to_left: bool,
    /// Note is to the right of stem (upstem chord with seconds).
    pub note_to_right: bool,
}

// ============================================================================
// SymWidthRight — right-side extent of objects
// Port of SpaceTime.cp:215-467
// ============================================================================

/// Compute right-side width of a Sync (note/rest) in STDIST.
///
/// Port of SymWidthRight SYNCtype case from SpaceTime.cp:245-330.
/// Considers notehead width, flag space, augmentation dots, and notes
/// displaced to the right side of the stem in chords.
pub fn sync_width_right(notes: &[NoteWidthInfo], to_head: bool) -> i16 {
    let mut tot_width: i16 = 0;

    for note in notes {
        let mut ns_width: i16;

        // Base notehead width: (STD_LINEHT*4)/3 = 10 STDIST
        // Port of GetNoteWidth from DrawUtils.cp:1220-1236.
        ns_width = (STD_LINEHT * 4) / 3;

        // Flag space: if stem-up, unbeamed, flagged duration, and not head-only
        // "STD_LINEHT is too little, STD_LINEHT*4/3 too much" => (STD_LINEHT*7)/6
        if !note.rest && !note.beamed && nflags(note.l_dur) > 0 && note.stem_up && !to_head {
            ns_width += (STD_LINEHT * 7) / 6;
        }

        // Augmentation dots determine width when present
        // Port of SpaceTime.cp:271-300 (non-graph-mode path).
        if note.ndots > 0 && !to_head {
            // First dot at default position: (STD_LINEHT*2)+2 = 18 STDIST
            ns_width = (STD_LINEHT * 2) + 2;
            // Adjust for desired dot position (xMoveDots default=3)
            ns_width += (STD_LINEHT * (note.x_move_dots as i16 - 3)) / 4;
            // Additional dots: STD_LINEHT per extra dot
            if note.ndots > 1 {
                ns_width += STD_LINEHT * (note.ndots as i16 - 1);
            }
        }

        tot_width = tot_width.max(ns_width);
    }

    // Upstem chord with notes to right of stem extends further
    // Port of SpaceTime.cp:307-325.
    if notes.iter().any(|n| n.note_to_right) {
        tot_width += STD_LINEHT;
    }

    tot_width
}

/// Compute right-side width of a GrSync (grace notes) in STDIST.
///
/// Port of SymWidthRight GRSYNCtype case from SpaceTime.cp:332-368.
pub fn grsync_width_right(notes: &[NoteWidthInfo], to_head: bool) -> i16 {
    let mut tot_width: i16 = 0;

    for note in notes {
        // Base grace notehead: (STD_LINEHT*4)/3 = 10 STDIST
        let mut ns_width: i16 = (STD_LINEHT * 4) / 3;

        // Grace note flags: smaller than normal: (STD_LINEHT*2)/3 ≈ 5 STDIST
        if !note.beamed && nflags(note.l_dur) > 0 && note.stem_up && !to_head {
            ns_width += (STD_LINEHT * 2) / 3;
        }

        tot_width = tot_width.max(ns_width);
    }

    // GRACESIZE(totWidth) + STD_LINEHT/3
    grace_size(tot_width) + STD_LINEHT / 3
}

/// Compute right-side width of a barline (Measure object) in STDIST.
///
/// Port of SymWidthRight MEASUREtype case from SpaceTime.cp:370-392.
pub fn measure_width_right(subtype: u8) -> i16 {
    match subtype {
        BAR_SINGLE => 0,
        BAR_DOUBLE => STD_LINEHT / 2,
        BAR_FINALDBL => STD_LINEHT, // (2*STD_LINEHT)/2
        BAR_RPT_L => (11 * STD_LINEHT) / 8,
        BAR_RPT_R => STD_LINEHT, // (8*STD_LINEHT)/8
        BAR_RPT_LR => (11 * STD_LINEHT) / 8,
        _ => 0,
    }
}

/// Compute right-side width of a clef in STDIST.
///
/// Port of SymWidthRight CLEFtype case from SpaceTime.cp:402-417.
pub fn clef_width_right(small: bool) -> i16 {
    // normWidth = .85*STD_LINEHT*4 = 0.85 * 32 ≈ 27 STDIST
    let norm_width: i16 = (85 * STD_LINEHT * 4) / 100;
    if small {
        3 * norm_width / 4 // SMALLSIZE
    } else {
        norm_width
    }
}

/// Compute right-side width of a key signature in STDIST.
///
/// Port of SymWidthRight KEYSIGtype case from SpaceTime.cp:419-436.
pub fn keysig_width_right(n_items: u8) -> i16 {
    if n_items == 0 {
        return 0;
    }
    let mut width: i16 = STD_ACCWIDTH;
    if n_items > 1 {
        width += (n_items as i16 - 1) * STD_ACCWIDTH;
    }
    width += STD_LINEHT / 2; // trailing space
    width
}

/// Compute right-side width of a time signature in STDIST.
///
/// Port of SymWidthRight TIMESIGtype case from SpaceTime.cp:438-457.
pub fn timesig_width_right(numerator: u8, denominator: u8) -> i16 {
    let n_chars: i16 = if numerator >= 10 || denominator >= 10 {
        2
    } else {
        1
    };
    (n_chars * 3 * STD_LINEHT) / 2
}

// ============================================================================
// SymWidthLeft — left-side extent of objects
// Port of SpaceTime.cp:83-212
// ============================================================================

/// Compute left-side width of a Sync (note/rest) in STDIST.
///
/// Port of SymWidthLeft SYNCtype case from SpaceTime.cp:104-152.
/// The left width is determined by accidentals and notes displaced
/// to the left of the stem.
pub fn sync_width_left(notes: &[NoteWidthInfo]) -> i16 {
    let note_to_left = notes.iter().any(|n| n.note_to_left);
    let mut max_xmove_acc: i16 = -1;

    for note in notes {
        if note.acc != 0 {
            let mut xmove = note.xmove_acc as i16;
            // Double flat needs extra space
            if note.acc == 5 {
                xmove += 2;
            }
            if note.courtesy_acc {
                xmove += 3; // config.courtesyAccLXD default
            }
            max_xmove_acc = max_xmove_acc.max(xmove);
        }
    }

    if max_xmove_acc >= 0 {
        // Downstem chord with notes to left needs more accidental room
        if note_to_left {
            max_xmove_acc += 4;
        }
        let mut tot_width = STD_ACCWIDTH;
        tot_width += STD_LINEHT / 8; // roundoff
        tot_width += (STD_ACCWIDTH * (max_xmove_acc - DFLT_XMOVEACC)) / 4;
        tot_width
    } else if note_to_left {
        STD_LINEHT
    } else {
        0
    }
}

/// Compute left-side width of a GrSync in STDIST.
///
/// Port of SymWidthLeft GRSYNCtype case from SpaceTime.cp:154-185.
pub fn grsync_width_left(notes: &[NoteWidthInfo]) -> i16 {
    let mut max_xmove_acc: i16 = -1;

    for note in notes {
        if note.acc != 0 {
            let mut xmove = note.xmove_acc as i16;
            if note.acc == 5 {
                xmove += 1; // GrSync uses +1 for dbl flat (vs +2 for Sync)
            }
            if note.courtesy_acc {
                xmove += 3;
            }
            max_xmove_acc = max_xmove_acc.max(xmove);
        }
    }

    if max_xmove_acc >= 0 {
        let mut tot_width = STD_ACCWIDTH;
        tot_width += (STD_ACCWIDTH * (max_xmove_acc - DFLT_XMOVEACC)) / 4;
        grace_size(tot_width)
    } else {
        0
    }
}

/// Compute left-side width of a barline in STDIST.
///
/// Port of SymWidthLeft MEASUREtype case from SpaceTime.cp:187-199.
pub fn measure_width_left(subtype: u8) -> i16 {
    if subtype == BAR_RPT_R || subtype == BAR_RPT_LR {
        (3 * STD_LINEHT) / 4
    } else {
        STD_LINEHT / 8
    }
}

// ============================================================================
// FIdealSpace with spaceProp scaling
// ============================================================================

/// Fine ideal space with spaceProp scaling factor.
///
/// Port of FIdealSpace from SpaceTime.cp:739-774.
/// spaceProp = RESFACTOR * spacePercent (default 5000 for 100%).
/// Returns fine STDIST (integer).
pub fn f_ideal_space_scaled(pdur_ticks: i32, space_prop: i32) -> i32 {
    let h_scale = space_prop as f32 / (RESFACTOR as f32 * 100.0);
    (h_scale * f_ideal_space(pdur_ticks)) as i32
}

// ============================================================================
// PrevITSym — find previous J_IT object in spacing array
// Port of SpaceHighLevel.cp:151-162
// ============================================================================

/// Find index of previous J_IT object, or None.
fn prev_it_sym(index: usize, info: &[SpaceTimeInfo]) -> Option<usize> {
    if index == 0 {
        return None;
    }
    (0..index).rev().find(|&j| info[j].just_type == J_IT)
}

// ============================================================================
// ConsiderITWidths — collision avoidance for J_IT objects
// Port of SpaceHighLevel.cp:432-529
// ============================================================================

/// Adjust spacing to prevent J_IT objects from overlapping.
///
/// Port of ConsiderITWidths from SpaceHighLevel.cp:432-529.
/// Simplified from the OG per-staff version: we compute widths across all
/// staves (ANYONE mode) since the Notelist pipeline doesn't need per-staff
/// collision detection at this stage.
///
/// For each J_IT object, ensures that the previous object's right width +
/// this object's left width + minimum spacing gap doesn't exceed the
/// available space. If it does, increases fSpBefore to prevent overlap.
fn consider_it_widths(info: &[SpaceTimeInfo], f_sp_before: &mut [i32]) {
    let n = info.len();
    let mut prev_need_right: i16 = 0;
    // Accumulates fine-STDIST space available between consecutive J_IT objects.
    // Reset to 0 after each J_IT is processed (OG pattern: SpaceHighLevel.cp:510).
    let mut f_avail_sp: i32 = 0;

    for i in 0..n {
        if info[i].just_type == J_IT || info[i].just_type == J_STRUC {
            f_avail_sp += f_sp_before[i];

            let need_left = info[i].width_left;

            // Space needed to prevent overlap: right extent of previous object
            // + left extent of this object - available space accumulated
            let mut f_sp_needed = std2f(prev_need_right + need_left) - f_avail_sp;

            // Add minimum spacing requirement
            if i == 0 {
                f_sp_needed += std2f(CONFIG_MIN_SP_AFTER_BAR);
            } else if i == n - 1 {
                f_sp_needed += std2f(CONFIG_MIN_SP_BEFORE_BAR);
            } else {
                f_sp_needed += std2f(CONFIG_MIN_R_SPACE);
            }

            if f_sp_needed > 0 {
                // Need more space: move this object right
                f_sp_before[i] += f_sp_needed;
            }

            // Reset for next J_IT object
            prev_need_right = info[i].width_right;
            f_avail_sp = 0;
        }
    }
}

// ============================================================================
// Respace1Bar — complete measure spacing
// Port of SpaceHighLevel.cp:846-967
// ============================================================================

/// Respace objects within a single measure using the Gourlay algorithm.
///
/// Port of Respace1Bar from SpaceHighLevel.cp:846-967.
///
/// Takes a SpaceTimeInfo array (with dur/frac already set), the spaceProp
/// scaling factor, and the previous barline's right width. Returns STDIST
/// positions for each object in the measure.
///
/// The algorithm:
///   1. Initialize fSpBefore[] with ideal duration-proportional spacing
///   2. Run ConsiderITWidths for collision avoidance
///   3. Convert accumulated spacing to position table
///   4. Convert fine STDIST to normal STDIST
pub fn respace_1bar(info: &[SpaceTimeInfo], space_prop: i32, prev_bar_width: i16) -> Vec<i16> {
    let n = info.len();
    if n == 0 {
        return vec![];
    }

    // Fine STDIST space-before table
    let mut f_sp_before: Vec<i32> = vec![0; n];

    // --- Initialize with ideal duration-based spacing ---

    // First object: space for barline right width + post-barline gap
    f_sp_before[0] = std2f(prev_bar_width);
    if info[0].is_sync {
        f_sp_before[0] += std2f(CONFIG_SP_AFTER_BAR);
    } else {
        f_sp_before[0] += std2f(CONFIG_MIN_SP_AFTER_BAR);
    }

    // Remaining objects: ideal space from controlling duration
    for i in 1..n {
        if info[i].just_type == J_IT || info[i].just_type == J_STRUC {
            if let Some(pi) = prev_it_sym(i, info) {
                let f_ideal = f_ideal_space_scaled(info[pi].dur, space_prop);
                f_sp_before[i] = (info[pi].frac * f_ideal as f32) as i32;
            }
        }
        // J_IP objects get fSpBefore=0 here (handled separately if needed)
    }

    // --- Collision avoidance ---
    consider_it_widths(info, &mut f_sp_before);

    // --- Convert to position table ---
    let mut position: Vec<i32> = vec![0; n];
    let mut xpos: i32 = 0;
    for i in 0..n {
        xpos += f_sp_before[i];
        position[i] = xpos;
    }

    // Convert fine STDIST to STDIST
    position.iter().map(|&p| f2std(p)).collect()
}

/// Compute the minimum width for a measure (STDIST).
///
/// Port of the minimum-width calculation from Respace1Bar:
///   minWidth = prevBarWidth + config.spAfterBar + EMPTYMEAS_WIDTH
pub fn min_measure_width_stdist(prev_bar_width: i16) -> i16 {
    prev_bar_width + CONFIG_SP_AFTER_BAR + EMPTY_MEAS_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ideal_space_quarter() {
        // Quarter note (l_dur=4) → index 5 → 5.50 * 8.0 = 44.0
        let space = ideal_space_stdist(4);
        assert!((space - 44.0).abs() < 0.01);
    }

    #[test]
    fn test_ideal_space_eighth() {
        // Eighth note (l_dur=5) → index 4 → 3.75 * 8.0 = 30.0
        let space = ideal_space_stdist(5);
        assert!((space - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_ideal_space_whole() {
        // Whole note (l_dur=2) → index 7 → 11.5 * 8.0 = 92.0
        let space = ideal_space_stdist(2);
        assert!((space - 92.0).abs() < 0.01);
    }

    #[test]
    fn test_stdist_to_ddist() {
        // 8.0 STDIST with staff_height=384 → 384/32 * 8.0 = 96.0 → 96 DDIST
        assert_eq!(stdist_to_ddist(8.0, 384), 96);
    }

    #[test]
    fn test_spacing_progression() {
        // Longer notes should get more space
        let s8 = ideal_space_stdist(5); // eighth
        let s4 = ideal_space_stdist(4); // quarter
        let s2 = ideal_space_stdist(3); // half
        let s1 = ideal_space_stdist(2); // whole
        assert!(s8 < s4);
        assert!(s4 < s2);
        assert!(s2 < s1);
    }

    // === Tests for OG spacing pipeline ===

    /// Helper to create a simple NoteWidthInfo for a plain note.
    fn plain_note(l_dur: i8) -> NoteWidthInfo {
        NoteWidthInfo {
            l_dur,
            ndots: 0,
            rest: false,
            beamed: false,
            stem_up: true,
            x_move_dots: 3,
            acc: 0,
            xmove_acc: 5, // DFLT_XMOVEACC
            courtesy_acc: false,
            note_to_left: false,
            note_to_right: false,
        }
    }

    #[test]
    fn test_sync_width_right_plain_quarter() {
        // Quarter note (no flags, no dots): just notehead width = 10 STDIST
        let notes = [plain_note(4)]; // quarter
        assert_eq!(sync_width_right(&notes, false), 10);
    }

    #[test]
    fn test_sync_width_right_eighth_stem_up() {
        // Eighth note stem up: notehead + flag space = 10 + 9 = 19 STDIST
        let notes = [plain_note(5)]; // eighth
        assert_eq!(sync_width_right(&notes, false), 10 + (STD_LINEHT * 7) / 6);
    }

    #[test]
    fn test_sync_width_right_eighth_beamed() {
        // Beamed eighth: no flag space, just notehead = 10 STDIST
        let mut n = plain_note(5);
        n.beamed = true;
        assert_eq!(sync_width_right(&[n], false), 10);
    }

    #[test]
    fn test_sync_width_right_dotted_quarter() {
        // Dotted quarter: dot space = (16+2) + (8*(3-3))/4 = 18 STDIST
        let mut n = plain_note(4);
        n.ndots = 1;
        assert_eq!(sync_width_right(&[n], false), 18);
    }

    #[test]
    fn test_sync_width_right_double_dotted() {
        // Double-dotted: 18 + 8*(2-1) = 26 STDIST
        let mut n = plain_note(4);
        n.ndots = 2;
        assert_eq!(sync_width_right(&[n], false), 26);
    }

    #[test]
    fn test_sync_width_right_to_head() {
        // to_head=true: ignore flags and dots, just notehead
        let mut n = plain_note(5);
        n.ndots = 1;
        assert_eq!(sync_width_right(&[n], true), 10);
    }

    #[test]
    fn test_sync_width_left_no_acc() {
        // No accidentals: width left = 0
        let notes = [plain_note(4)];
        assert_eq!(sync_width_left(&notes), 0);
    }

    #[test]
    fn test_sync_width_left_with_acc() {
        // Default accidental: STD_ACCWIDTH + roundoff + 0 = 9 + 1 + 0 = 10
        let mut n = plain_note(4);
        n.acc = 2; // flat
        assert_eq!(sync_width_left(&[n]), 10);
    }

    #[test]
    fn test_grsync_width_right() {
        // Grace note: GRACESIZE(10) + STD_LINEHT/3 = 7 + 2 = 9 STDIST
        let notes = [plain_note(6)]; // 16th (beamed in practice, but test unbeamed)
        let w = grsync_width_right(&notes, false);
        // 10 base + 5 flags = 15, GRACESIZE(15) = 10, + 2 = 12
        assert_eq!(w, grace_size(15) + STD_LINEHT / 3);
    }

    #[test]
    fn test_measure_width_right_single() {
        assert_eq!(measure_width_right(BAR_SINGLE), 0);
    }

    #[test]
    fn test_measure_width_right_double() {
        assert_eq!(measure_width_right(BAR_DOUBLE), 4);
    }

    #[test]
    fn test_clef_width_right_normal() {
        // 0.85 * 32 = 27.2, integer = 27
        assert_eq!(clef_width_right(false), 27);
    }

    #[test]
    fn test_clef_width_right_small() {
        // SMALLSIZE(27) = 3*27/4 = 20
        assert_eq!(clef_width_right(true), 20);
    }

    #[test]
    fn test_keysig_width_right_2_sharps() {
        // 2 accidentals: 9 + 9 + 4 = 22
        assert_eq!(keysig_width_right(2), 22);
    }

    #[test]
    fn test_timesig_width_right_4_4() {
        // Single digits: (1*3*8)/2 = 12
        assert_eq!(timesig_width_right(4, 4), 12);
    }

    #[test]
    fn test_f_ideal_space_scaled_default() {
        // Default spacing (100%): spaceProp = 5000
        let base = f_ideal_space(480); // quarter note = 480 ticks
        let scaled = f_ideal_space_scaled(480, 5000);
        assert!((scaled as f32 - base).abs() < 1.0);
    }

    #[test]
    fn test_f_ideal_space_scaled_half() {
        // 50% spacing: spaceProp = 2500
        let base = f_ideal_space(480);
        let scaled = f_ideal_space_scaled(480, 2500);
        assert!((scaled as f32 - base * 0.5).abs() < 1.0);
    }

    #[test]
    fn test_respace_1bar_single_event() {
        // Single quarter note in a measure
        let info = vec![SpaceTimeInfo {
            index: 0,
            start_time: 0,
            dur: 480,
            frac: 1.0,
            is_sync: true,
            just_type: J_IT,
            width_left: 0,
            width_right: 10,
        }];
        let positions = respace_1bar(&info, 5000, 0);
        assert_eq!(positions.len(), 1);
        // First object: prevBarWidth(0) + spAfterBar(6) = 6 STDIST
        assert_eq!(positions[0], 6);
    }

    #[test]
    fn test_respace_1bar_two_quarters() {
        // Two quarter notes
        let info = vec![
            SpaceTimeInfo {
                index: 0,
                start_time: 0,
                dur: 480,
                frac: 1.0,
                is_sync: true,
                just_type: J_IT,
                width_left: 0,
                width_right: 10,
            },
            SpaceTimeInfo {
                index: 1,
                start_time: 480,
                dur: 480,
                frac: 1.0,
                is_sync: true,
                just_type: J_IT,
                width_left: 0,
                width_right: 10,
            },
        ];
        let positions = respace_1bar(&info, 5000, 0);
        assert_eq!(positions.len(), 2);
        // First at 6, second at 6 + ideal_space(quarter)*frac
        assert!(positions[1] > positions[0]);
        // Second should be spaced proportionally
        let delta = positions[1] - positions[0];
        // Quarter note ideal space ≈ 44 STDIST, so delta should be around that
        assert!((40..=50).contains(&delta), "delta={}", delta);
    }

    #[test]
    fn test_respace_1bar_collision_avoidance() {
        // Two events very close together but with wide objects
        // should be pushed apart by ConsiderITWidths
        let info = vec![
            SpaceTimeInfo {
                index: 0,
                start_time: 0,
                dur: 120, // 32nd note
                frac: 1.0,
                is_sync: true,
                just_type: J_IT,
                width_left: 0,
                width_right: 18, // dotted note: wide
            },
            SpaceTimeInfo {
                index: 1,
                start_time: 120,
                dur: 120,
                frac: 1.0,
                is_sync: true,
                just_type: J_IT,
                width_left: 10, // has accidental
                width_right: 10,
            },
        ];
        let positions = respace_1bar(&info, 5000, 0);
        let delta = positions[1] - positions[0];
        // Must be at least width_right(18) + width_left(10) + minRSpace(2) = 30
        assert!(
            delta >= 30,
            "delta={} should be >= 30 for collision avoidance",
            delta
        );
    }

    #[test]
    fn test_std2f_f2std_roundtrip() {
        assert_eq!(f2std(std2f(42)), 42);
        assert_eq!(std2f(10), 100);
        assert_eq!(f2std(100), 10);
    }

    #[test]
    fn test_min_measure_width() {
        // prevBarWidth=0: 0 + 6 + 40 = 46
        assert_eq!(min_measure_width_stdist(0), 46);
    }
}
