//! Spacing and time utilities — port of SpaceTime.cp.
//!
//! Duration-proportional spacing tables and functions used for measure layout.
//!
//! Reference: Nightingale/src/CFilesBoth/SpaceTime.cp

use crate::basic_types::Ddist;

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

/// Convert STDIST (as float) to DDIST for standard 5-line staff.
///
/// Port of std2d for spacing context:
///   std2d(s, staffHeight, staffLines) = s * staffHeight / (STD_LINEHT * (staffLines-1))
///
/// For staffHeight=384, staffLines=5: 1 STDIST = 384 / 32 = 12 DDIST
pub fn stdist_to_ddist(stdist: f32, staff_height: Ddist) -> Ddist {
    (stdist * staff_height as f32 / (STD_LINEHT_F * 4.0)) as Ddist
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
}
