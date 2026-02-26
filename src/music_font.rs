//! Music font metrics — port of MusicFont.cp.
//!
//! Font-specific metrics for the current music font (Bravura/SMuFL).
//! These values are used by both layout (notelist/to_score, ngl) and
//! drawing code to compute stem positions, notehead widths, etc.
//!
//! Reference: Nightingale/src/CFilesBoth/MusicFont.cp

use crate::basic_types::Ddist;

/// MusFontStemSpaceWidthDDIST — stem-space width (distance from left edge of
/// notehead to where the stem attaches on the right side for stems-up).
///
/// Port of MusFontStemSpaceWidthDDIST from MusicFont.cp.
/// For Bravura, we use the notehead width from metadata (1.18 staff spaces),
/// which maps to stemSpaceWidth = 118 (percent of interline space).
/// Sonata's stemSpaceWidth was typically ~80.
///
/// Formula: (stemSpaceWidth * lnSpace) / 100
pub fn stem_space_width_ddist(ln_space: Ddist) -> Ddist {
    // Bravura noteheadBlack bbox NE.x = 1.18 spaces → 118% of lnSpace
    // This is the distance from the left edge of the notehead to the right edge
    // where an upstem attaches.
    (118 * ln_space as i32 / 100) as Ddist
}

/// Stem space width for Bravura font, as a percentage of interline space.
pub const STEM_SPACE_WIDTH_PCT: i32 = 118;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stem_space_width_ddist() {
        // lnSpace=16 → 118*16/100 = 18 DDIST
        assert_eq!(stem_space_width_ddist(16), 18);
    }

    #[test]
    fn test_stem_space_width_ddist_small() {
        // lnSpace=12 → 118*12/100 = 14 DDIST
        assert_eq!(stem_space_width_ddist(12), 14);
    }
}
