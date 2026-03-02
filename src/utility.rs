//! General utility functions — port of Utility.cp.
//!
//! Shared stem calculation and note-related utilities used by both the
//! Notelist and NGL pipelines, and by the drawing modules.
//!
//! Reference: Nightingale/src/Utilities/Utility.cp

use crate::basic_types::Ddist;

// ===========================================================================
// Stem calculation — faithful port from Utility.cp
// ===========================================================================

/// CalcYStem — calculate optimum stem endpoint for a note.
///
/// Direct port of CalcYStem from Utility.cp (lines 49-89).
///
/// # Arguments
/// * `yhead` - DDIST position of note head (relative to staff top)
/// * `nflags` - number of flags or beams (0 for quarter, 1 for eighth, etc.)
/// * `stem_down` - true if stem points downward
/// * `staff_height` - total staff height in DDIST
/// * `staff_lines` - number of staff lines (typically 5)
/// * `qtr_sp` - desired stem length in quarter-spaces (from config)
/// * `no_extend` - if true, don't extend stem to midline
///
/// # Returns
/// The DDIST Y position of the stem endpoint.
pub fn calc_ystem(
    yhead: Ddist,
    nflags: i16,
    stem_down: bool,
    staff_height: Ddist,
    staff_lines: i16,
    qtr_sp: i16,
    no_extend: bool,
) -> Ddist {
    let mut qtr_sp = qtr_sp;

    // Extend stem for flags beyond the first two (Bravura has 16th flag chars).
    // Reference: Utility.cp:69-74
    // MusFontHas16thFlag is true for Bravura (it has dedicated 16th flag glyphs).
    if nflags > 2 {
        qtr_sp += 4 * (nflags - 2); // Every flag after 1st 2 adds a space
    }

    // Convert quarter-spaces to DDIST
    // Reference: Utility.cp:78
    let d_len = (qtr_sp as i32 * staff_height as i32 / (4 * (staff_lines as i32 - 1))) as Ddist;

    // Initially, set stem end to requested length from notehead
    let mut ystem = if stem_down {
        yhead + d_len
    } else {
        yhead - d_len
    };

    // Extend to midline if beneficial
    // Reference: Utility.cp:82-88
    if !no_extend {
        let midline = staff_height / 2;
        // Would ending at midline lengthen the stem without changing direction?
        if (yhead - midline).abs() > d_len && (ystem - midline).abs() < (yhead - midline).abs() {
            ystem = midline;
        }
    }

    ystem
}

/// ShortenStem — should this note get a shorter-than-normal stem?
///
/// Returns true for notes entirely outside the staff with stems pointing away
/// from the staff. In OG, these get `stemLenOutside` (12 qtr-sp) instead of
/// `stemLenNormal` (14 qtr-sp).
///
/// Port of ShortenStem from Utility.cp:135-150.
///
/// * `half_ln` - half-line position (0 = top staff line, 2 = next line, etc.)
/// * `stem_down` - true if stem goes down
/// * `staff_lines` - number of staff lines (typically 5)
pub fn shorten_stem(half_ln: i16, stem_down: bool, staff_lines: i16) -> bool {
    // STRICT_SHORTSTEM = 0 (style.h:61) — no strictness adjustment
    const STRICT_SHORTSTEM: i16 = 0;

    // Above staff (halfLn < 0) with stem up → shorten
    if half_ln < STRICT_SHORTSTEM && !stem_down {
        return true;
    }

    // Below staff (halfLn > bottom line) with stem down → shorten
    let bottom_half_ln = 2 * (staff_lines - 1);
    if half_ln > (bottom_half_ln - STRICT_SHORTSTEM) && stem_down {
        return true;
    }

    false
}

/// NFLAGS — number of flags for a given duration code.
///
/// Port of NFLAGS macro from defs.h.
/// l_dur: BREVE=1, WHOLE=2, HALF=3, QTR=4, 8TH=5, 16TH=6, 32ND=7, 64TH=8
pub fn nflags(l_dur: i8) -> i16 {
    match l_dur {
        x if x >= 5 => (x - 4) as i16, // 8th=1, 16th=2, 32nd=3, 64th=4
        _ => 0,                        // quarter and longer have no flags
    }
}

/// GetLineAugDotPos — get Y offset for augmentation dot to avoid staff lines.
///
/// Port of GetLineAugDotPos from Utility.cp (line 262).
/// If the notehead sits exactly on a staff line, the dot must be nudged up
/// by a half-space so it sits in the space above, not on the line.
///
/// Returns DDIST Y adjustment for the augmentation dot.
pub fn get_line_aug_dot_pos(half_ln: i16, staff_height: Ddist, staff_lines: i16) -> Ddist {
    // Check if the note is on a line (even half-line positions = lines)
    let on_line = half_ln % 2 == 0 && half_ln >= 0 && half_ln <= (staff_lines - 1) * 2;
    if on_line {
        // Nudge dot up by one half-space (quarter interline)
        let d_interline = staff_height / (staff_lines - 1) as Ddist;
        -(d_interline / 2)
    } else {
        0
    }
}

// ===========================================================================
// Coordinate conversion macros
// ===========================================================================

/// STD_LINEHT — STDIST units per interline space.
const STD_LINEHT: i16 = 8;

/// std2d — convert STDIST to DDIST.
///
/// Port of std2d macro from defs.h:521:
///   #define std2d(std, stfHt, lines) ( ((long)(std)*(stfHt)) / (8*((lines)-1)) )
pub fn std2d(stdist: i16, staff_height: Ddist, staff_lines: i16) -> Ddist {
    (stdist as i32 * staff_height as i32 / (STD_LINEHT as i32 * (staff_lines as i32 - 1))) as Ddist
}

/// HeadWidth — width of common (beamable) note heads.
///
/// Port of HeadWidth macro from defs.h:355:
///   #define HeadWidth(lnSp) (9*(lnSp)*4/32)
///
/// Simplifies to: 9*lnSp/8 = 1.125 * lnSpace
pub fn head_width(ln_space: Ddist) -> Ddist {
    (9 * ln_space * 4) / 32
}

/// STD_ACCWIDTH — width of common accidentals in STDIST units.
///
/// From defs.h:318: #define STD_ACCWIDTH (9*STD_LINEHT/8) = 9*8/8 = 9
pub const STD_ACCWIDTH: i16 = 9;

/// DFLT_XMOVEACC — default note xMoveAcc value.
///
/// From defs.h:256.
pub const DFLT_XMOVEACC: i16 = 5;

/// AccXOffset — compute X offset for accidental placement.
///
/// Port of AccXOffset from DrawNRGR.cp:396-406.
/// Returns DDIST offset to place accidental to the left of the notehead.
pub fn acc_x_offset(xmove_acc: i16, staff_height: Ddist, staff_lines: i16) -> Ddist {
    let d_acc_width = std2d(STD_ACCWIDTH, staff_height, staff_lines);
    let mut x_offset = d_acc_width; // Default offset
    x_offset += (d_acc_width * (xmove_acc - DFLT_XMOVEACC)) / 4; // Fine-tune
    x_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_ystem_stem_down() {
        // Standard 5-line staff, height=64, qtr_sp=14 (standard stem length)
        let ystem = calc_ystem(32, 0, true, 64, 5, 14, false);
        assert!(ystem > 32, "stem-down endpoint should be below notehead");
    }

    #[test]
    fn test_calc_ystem_stem_up() {
        let ystem = calc_ystem(32, 0, false, 64, 5, 14, false);
        assert!(ystem < 32, "stem-up endpoint should be above notehead");
    }

    #[test]
    fn test_nflags() {
        assert_eq!(nflags(4), 0); // quarter
        assert_eq!(nflags(5), 1); // eighth
        assert_eq!(nflags(6), 2); // sixteenth
        assert_eq!(nflags(7), 3); // thirty-second
    }

    #[test]
    fn test_head_width() {
        // lnSpace=16 → 9*16*4/32 = 18
        assert_eq!(head_width(16), 18);
    }

    #[test]
    fn test_std2d() {
        // 8 STDIST units = 1 interline space. For staff_height=64, lines=5:
        // std2d(8, 64, 5) = 8*64/(8*4) = 16 DDIST (= 1 interline)
        assert_eq!(std2d(8, 64, 5), 16);
    }

    #[test]
    fn test_acc_x_offset_default() {
        // With default xmove_acc, should return d_acc_width exactly
        let offset = acc_x_offset(DFLT_XMOVEACC, 64, 5);
        let d_acc_width = std2d(STD_ACCWIDTH, 64, 5);
        assert_eq!(offset, d_acc_width);
    }

    #[test]
    fn test_shorten_stem() {
        // 5-line staff: lines at halflines 0, 2, 4, 6, 8
        // Note above staff (halfLn < 0) with stem up → shorten
        assert!(shorten_stem(-1, false, 5));
        assert!(shorten_stem(-2, false, 5));
        // Note above staff with stem DOWN → don't shorten (stem goes toward staff)
        assert!(!shorten_stem(-1, true, 5));

        // Note below staff (halfLn > 8) with stem down → shorten
        assert!(shorten_stem(9, true, 5));
        assert!(shorten_stem(10, true, 5));
        // Note below staff with stem UP → don't shorten
        assert!(!shorten_stem(9, false, 5));

        // Note inside staff → never shorten
        assert!(!shorten_stem(0, false, 5));
        assert!(!shorten_stem(4, true, 5));
        assert!(!shorten_stem(8, true, 5));
        assert!(!shorten_stem(0, true, 5));
    }
}
