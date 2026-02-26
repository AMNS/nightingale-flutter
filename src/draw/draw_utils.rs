//! Glyph mapping and layout utilities — port of DrawUtils.cp.
//!
//! Maps duration codes, accidentals, clef types, etc. to SMuFL glyph codepoints.
//! Also contains key signature Y-offset tables and rest positioning.
//!
//! Reference: Nightingale/src/Utilities/DrawUtils.cp

use crate::defs::*;

/// Map l_dur (logical duration) to notehead glyph for normal-appearance notes.
///
/// Reference: DrawNRGR.cp, MusCharXLoc() and GetMusicAscDesc()
/// SMuFL noteheads:
/// - BREVE_L_DUR (1): 0xE0A0 (noteheadDoubleWhole)
/// - WHOLE_L_DUR (2): 0xE0A2 (noteheadWhole)
/// - HALF_L_DUR (3): 0xE0A3 (noteheadHalf)
/// - QTR_L_DUR+ (4+): 0xE0A4 (noteheadBlack)
pub fn notehead_glyph_for_duration(l_dur: i8) -> u32 {
    match l_dur {
        x if x == BREVE_L_DUR => 0xE0A0, // noteheadDoubleWhole
        x if x == WHOLE_L_DUR => 0xE0A2, // noteheadWhole
        x if x == HALF_L_DUR => 0xE0A3,  // noteheadHalf
        _ => 0xE0A4,                     // noteheadBlack (quarter and shorter)
    }
}

/// Map head_shape and l_dur to the appropriate notehead glyph.
///
/// Reference: Utilities/DrawUtils.cp, NoteGlyph() (line 1159) and GetNoteheadInfo() (line 1190)
/// HeadShape values (from NObjTypes.h lines 125-137):
/// - 0 = NO_VIS (invisible)
/// - 1 = NORMAL_VIS (normal appearance)
/// - 2 = X_SHAPE (X-shaped head)
/// - 3 = HARMONIC_SHAPE (hollow harmonic head)
/// - 4 = SQUAREH_SHAPE (square hollow)
/// - 5 = SQUAREF_SHAPE (square filled)
/// - 6 = DIAMONDH_SHAPE (diamond hollow)
/// - 7 = DIAMONDF_SHAPE (diamond filled)
/// - 8 = HALFNOTE_SHAPE (always half-note head)
/// - 9 = SLASH_SHAPE (chord slash - drawn as line, not glyph)
/// - 10 = NOTHING_VIS (everything invisible)
///
/// SMuFL alternate notehead glyphs:
/// - X-shape: 0xE0A9 (noteheadXBlack), 0xE0A8 (noteheadXHalf), 0xE0A7 (noteheadXWhole)
/// - Harmonic: 0xE0D3 (noteheadDiamondHalf) - OG uses 'O' (0x4F) in Sonata font
/// - Square hollow: 0xE0B9 (noteheadSquareWhite)
/// - Square filled: 0xE0B3 (noteheadSquareBlack)
/// - Diamond hollow: 0xE0D3 (noteheadDiamondHalf), 0xE0D2 (noteheadDiamondWhole)
/// - Diamond filled: 0xE0DB (noteheadDiamondBlack)
/// - Slash: return 0 (must be drawn as custom line)
pub fn notehead_glyph(head_shape: u8, l_dur: i8) -> u32 {
    use crate::obj_types::HeadShape;

    match head_shape {
        x if x == HeadShape::XShape as u8 => {
            // X-shaped noteheads (for percussion, ghost notes)
            // Reference: DrawUtils.cp line 1163, MCH_xShapeHead = 0xC0
            match l_dur {
                x if x == WHOLE_L_DUR => 0xE0A7, // noteheadXWhole
                x if x == HALF_L_DUR => 0xE0A8,  // noteheadXHalf
                _ => 0xE0A9,                     // noteheadXBlack (quarter and shorter)
            }
        }
        x if x == HeadShape::HarmonicShape as u8 => {
            // Harmonic (hollow diamond-like) noteheads
            // Reference: DrawUtils.cp line 1164, MCH_harmonicHead = 'O'
            // Use diamond hollow shapes from SMuFL
            match l_dur {
                x if x == WHOLE_L_DUR || x == BREVE_L_DUR => 0xE0D2, // noteheadDiamondWhole
                _ => 0xE0D3, // noteheadDiamondHalf (used for all other durations)
            }
        }
        x if x == HeadShape::SquareHShape as u8 => {
            // Square hollow notehead
            // Reference: DrawUtils.cp line 1166, MCH_squareHHead = 0xAD
            0xE0B9 // noteheadSquareWhite (always hollow)
        }
        x if x == HeadShape::SquareFShape as u8 => {
            // Square filled notehead
            // Reference: DrawUtils.cp line 1167, MCH_squareFHead = 0xD0
            0xE0B3 // noteheadSquareBlack (always filled)
        }
        x if x == HeadShape::DiamondHShape as u8 => {
            // Diamond hollow notehead
            // Reference: DrawUtils.cp line 1168, MCH_diamondHHead = 0xE1
            match l_dur {
                x if x == WHOLE_L_DUR || x == BREVE_L_DUR => 0xE0D2, // noteheadDiamondWhole
                _ => 0xE0D3,                                         // noteheadDiamondHalf
            }
        }
        x if x == HeadShape::DiamondFShape as u8 => {
            // Diamond filled notehead
            // Reference: DrawUtils.cp line 1169, MCH_diamondFHead = 0xE2
            0xE0DB // noteheadDiamondBlack (always filled)
        }
        x if x == HeadShape::HalfnoteShape as u8 => {
            // Always use half-note head regardless of duration
            // Reference: DrawUtils.cp line 1170, MCH_halfNoteHead = 0xFA
            0xE0A3 // noteheadHalf
        }
        x if x == HeadShape::SlashShape as u8 => {
            // Slash notation (chord slash)
            // Reference: DrawUtils.cp line 1165 - returns '\0', must be drawn as line
            // TODO: implement slash drawing in renderer
            0 // Return 0 to indicate no glyph (must draw custom slash)
        }
        _ => {
            // NORMAL_VIS (1), NO_VIS (0), or NOTHING_VIS (10): use normal duration-based glyph
            notehead_glyph_for_duration(l_dur)
        }
    }
}

/// Resolve the effective drawing l_dur for a rest, handling whole-measure rests.
///
/// In OG Nightingale, whole-measure rests (l_dur <= -1) are stored with negative
/// subType values but drawn as whole rests (or breve rests in some time signatures).
///
/// Reference: DrawUtils.cp, GetRestDrawInfo(), line 1281
/// - subType <= WHOLEMR_L_DUR (-1): draw as WHOLE_L_DUR (or BREVE if time sig warrants)
/// - subType == UNKNOWN_L_DUR (0): draw as BREVE_L_DUR (shouldn't happen)
/// - subType > 0: draw as-is (normal CMN duration)
pub fn resolve_rest_l_dur(l_dur: i8) -> i8 {
    if l_dur <= WHOLEMR_L_DUR {
        // Whole-measure rest (or multi-measure rest): draw as whole rest.
        // TODO: check WholeMeasRestIsBreve() for time sigs like 4/2, 2/1
        WHOLE_L_DUR
    } else if l_dur == UNKNOWN_L_DUR {
        BREVE_L_DUR // Should never happen
    } else {
        l_dur
    }
}

/// Map l_dur (logical duration) to rest glyph.
///
/// Reference: DrawNRGR.cp, DrawRest() (line 1402)
/// SMuFL rests:
/// - BREVE_L_DUR (1): 0xE4E2 (restDoubleWhole)
/// - WHOLE_L_DUR (2): 0xE4E3 (restWhole)
/// - HALF_L_DUR (3): 0xE4E4 (restHalf)
/// - QTR_L_DUR (4): 0xE4E5 (restQuarter)
/// - EIGHTH_L_DUR (5): 0xE4E6 (restEighth)
/// - SIXTEENTH_L_DUR (6): 0xE4E7 (rest16th)
pub fn rest_glyph_for_duration(l_dur: i8) -> u32 {
    match l_dur {
        x if x == BREVE_L_DUR => 0xE4E2,
        x if x == WHOLE_L_DUR => 0xE4E3,
        x if x == HALF_L_DUR => 0xE4E4,
        x if x == QTR_L_DUR => 0xE4E5,
        x if x == EIGHTH_L_DUR => 0xE4E6,
        x if x == SIXTEENTH_L_DUR => 0xE4E7,
        _ => 0xE4E5, // Default to quarter rest
    }
}

/// Vertical Y offset for rest glyphs, in half-spaces.
///
/// Indexed by l_dur value (1=breve through 9=128th).
/// Reference: vars.h line 342
pub const REST_Y_OFFSET: [i16; 10] = [0, 0, 0, 0, 0, -1, 1, 1, 3, 3];

/// Map accidental code to SMuFL glyph.
///
/// Accidental codes (from NObjTypes.h, ANote.accident field):
/// - 0: none
/// - 1: double flat (0xE264)
/// - 2: flat (0xE260)
/// - 3: natural (0xE261)
/// - 4: sharp (0xE262)
/// - 5: double sharp (0xE263)
pub fn accidental_glyph(accident_code: u8) -> Option<u32> {
    match accident_code {
        1 => Some(0xE264), // accidentalDoubleFlat
        2 => Some(0xE260), // accidentalFlat
        3 => Some(0xE261), // accidentalNatural
        4 => Some(0xE262), // accidentalSharp
        5 => Some(0xE263), // accidentalDoubleSharp
        _ => None,
    }
}

/// Map clef type to SMuFL glyph.
///
/// Reference: DrawUtils.cp, GetClefDrawInfo() (line 285-308)
///
/// Clef types (NObjTypes.h:298-311):
///   1=TREBLE8_CLEF, 2=FRVIOLIN_CLEF, 3=TREBLE_CLEF, 4=SOPRANO_CLEF,
///   5=MZSOPRANO_CLEF, 6=ALTO_CLEF, 7=TRTENOR_CLEF, 8=TENOR_CLEF,
///   9=BARITONE_CLEF, 10=BASS_CLEF, 11=BASS8B_CLEF, 12=PERC_CLEF
pub fn clef_glyph(clef_type: i8) -> u32 {
    match clef_type {
        1 => 0xE052,                 // TREBLE8_CLEF  -> gClef8vb
        3 | 7 => 0xE050,             // TREBLE_CLEF / TRTENOR_CLEF -> gClef
        4 | 5 | 6 | 8 | 9 => 0xE05C, // SOPRANO..BARITONE -> cClef
        10 | 11 => 0xE062,           // BASS_CLEF / BASS8B_CLEF -> fClef
        12 => 0xE069,                // PERC_CLEF -> unpitchedPercussionClef1
        _ => 0xE050,                 // Default to treble
    }
}

/// Get the Y position (in half-lines from staff top) for a clef glyph origin.
///
/// Reference: DrawUtils.cp, GetClefDrawInfo() (line 337-384)
///
/// Half-lines are counted from staff top: 0 = top line, 8 = bottom line (5-line staff).
/// The glyph origin for each family is:
///   gClef: origin at the G line (the curl's intersection)
///   cClef: origin at the C line (the middle indentation)
///   fClef: origin at the F line (where the dots go)
///   percClef: origin centered on the staff
///
/// For standard 5-line staff (lines at halflines 0, 2, 4, 6, 8):
///   G line = halfline 6, F line = halfline 2, middle = halfline 4
pub fn clef_halfline_position(clef_type: i8) -> i16 {
    match clef_type {
        1 | 3 | 7 => 6, // TREBLE8/TREBLE/TRTENOR: G line (2nd from bottom)
        4 => 8,         // SOPRANO: C on bottom line
        5 => 6,         // MZSOPRANO: C on 2nd line from bottom
        6 => 4,         // ALTO: C on middle line
        8 => 2,         // TENOR: C on 2nd line from top
        9 => 0,         // BARITONE: C on top line
        10 | 11 => 2,   // BASS/BASS8B: F line (2nd from top)
        12 => 4,        // PERC: centered
        _ => 4,         // Default to middle
    }
}

/// Get flag glyph for unbeamed eighth/sixteenth notes.
///
/// Reference: DrawNRGR.cp, DrawModNR() (line 1158)
/// SMuFL flags:
/// - 8th up: 0xE240, down: 0xE241
/// - 16th up: 0xE242, down: 0xE243
pub fn flag_glyph(l_dur: i8, stem_up: bool) -> Option<u32> {
    match l_dur {
        x if x == EIGHTH_L_DUR => {
            if stem_up {
                Some(0xE240) // flag8thUp
            } else {
                Some(0xE241) // flag8thDown
            }
        }
        x if x == SIXTEENTH_L_DUR => {
            if stem_up {
                Some(0xE242) // flag16thUp
            } else {
                Some(0xE243) // flag16thDown
            }
        }
        _ => None,
    }
}

/// Get the vertical half-line position for a key signature accidental.
///
/// Port of GetKSYOffset (DrawUtils.cp:737-806).
/// Returns half-line position (0 = top staff line) based on clef type and letter code.
///
/// Letter codes: F=0, E=1, D=2, C=3, B=4, A=5, G=6
pub fn get_ks_y_offset(clef_type: i8, letcode: i8, is_sharp: bool) -> i8 {
    // Position tables from DrawUtils.cp:741-793
    // Indexed by letcode: [F=0, E=1, D=2, C=3, B=4, A=5, G=6]
    const TREBLE_SHARP: [i8; 7] = [0, 1, 2, 3, 4, 5, -1]; // F♯ top line, G♯ above staff
    const TREBLE_FLAT: [i8; 7] = [7, 1, 2, 3, 4, 5, 6]; // B♭ 4th line, E♭ 1st space
    const ALTO_SHARP: [i8; 7] = [1, 2, 3, 4, 5, 6, 0];
    const ALTO_FLAT: [i8; 7] = [8, 2, 3, 4, 5, 6, 7];
    const BASS_SHARP: [i8; 7] = [2, 3, 4, 5, 6, 7, 1];
    const BASS_FLAT: [i8; 7] = [9, 3, 4, 5, 6, 7, 8];
    const TENOR_SHARP: [i8; 7] = [6, 0, 1, 2, 3, 4, 5];
    const TENOR_FLAT: [i8; 7] = [6, 0, 1, 2, 3, 4, 5];
    const SOPRANO_SHARP: [i8; 7] = [5, 6, 7, 1, 2, 3, 4];
    const SOPRANO_FLAT: [i8; 7] = [5, 6, 7, 1, 2, 3, 4];
    const MZ_SOPR_SHARP: [i8; 7] = [3, 4, 5, 6, 7, 1, 2];
    const MZ_SOPR_FLAT: [i8; 7] = [3, 4, 5, 6, 7, 1, 2];
    const BARITONE_SHARP: [i8; 7] = [4, 5, 6, 7, 1, 2, 3];
    const BARITONE_FLAT: [i8; 7] = [4, 5, 6, 7, 1, 2, 3];

    let idx = (letcode as usize).min(6);

    // Clef type constants from NObjTypes.h:298-314
    // TREBLE8_CLEF=1, FRVIOLIN_CLEF=2, TREBLE_CLEF=3, SOPRANO_CLEF=4,
    // MZSOPRANO_CLEF=5, ALTO_CLEF=6, TRTENOR_CLEF=7, TENOR_CLEF=8,
    // BARITONE_CLEF=9, BASS_CLEF=10, BASS8B_CLEF=11, PERC_CLEF=12
    match clef_type {
        1..=3 | 7 | 12 => {
            // TREBLE8_CLEF=1 | FRVIOLIN_CLEF=2 | TREBLE_CLEF=3 | TRTENOR_CLEF=7 | PERC_CLEF=12
            if is_sharp {
                TREBLE_SHARP[idx]
            } else {
                TREBLE_FLAT[idx]
            }
        }
        4 => {
            // SOPRANO_CLEF
            if is_sharp {
                SOPRANO_SHARP[idx]
            } else {
                SOPRANO_FLAT[idx]
            }
        }
        5 => {
            // MZSOPRANO_CLEF
            if is_sharp {
                MZ_SOPR_SHARP[idx]
            } else {
                MZ_SOPR_FLAT[idx]
            }
        }
        6 => {
            // ALTO_CLEF
            if is_sharp {
                ALTO_SHARP[idx]
            } else {
                ALTO_FLAT[idx]
            }
        }
        8 => {
            // TENOR_CLEF
            if is_sharp {
                TENOR_SHARP[idx]
            } else {
                TENOR_FLAT[idx]
            }
        }
        9 => {
            // BARITONE_CLEF
            if is_sharp {
                BARITONE_SHARP[idx]
            } else {
                BARITONE_FLAT[idx]
            }
        }
        10 | 11 => {
            // BASS_CLEF=10 | BASS8B_CLEF=11
            if is_sharp {
                BASS_SHARP[idx]
            } else {
                BASS_FLAT[idx]
            }
        }
        _ => {
            // Default to treble (includes TRTENOR_CLEF=7)
            if is_sharp {
                TREBLE_SHARP[idx]
            } else {
                TREBLE_FLAT[idx]
            }
        }
    }
}
