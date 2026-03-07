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
/// OG Sonata glyphs: MCH_rests[] in vars.h:337
/// SMuFL rests (U+E4E2..U+E4EA):
/// - BREVE_L_DUR (1): U+E4E2 (restDoubleWhole)
/// - WHOLE_L_DUR (2): U+E4E3 (restWhole)
/// - HALF_L_DUR (3): U+E4E4 (restHalf)
/// - QTR_L_DUR (4): U+E4E5 (restQuarter)
/// - EIGHTH_L_DUR (5): U+E4E6 (rest8th)
/// - SIXTEENTH_L_DUR (6): U+E4E7 (rest16th)
/// - THIRTY2ND_L_DUR (7): U+E4E8 (rest32nd)
/// - SIXTY4TH_L_DUR (8): U+E4E9 (rest64th)
/// - ONE28TH_L_DUR (9): U+E4EA (rest128th)
pub fn rest_glyph_for_duration(l_dur: i8) -> u32 {
    match l_dur {
        x if x == BREVE_L_DUR => 0xE4E2,
        x if x == WHOLE_L_DUR => 0xE4E3,
        x if x == HALF_L_DUR => 0xE4E4,
        x if x == QTR_L_DUR => 0xE4E5,
        x if x == EIGHTH_L_DUR => 0xE4E6,
        x if x == SIXTEENTH_L_DUR => 0xE4E7,
        x if x == THIRTY2ND_L_DUR => 0xE4E8,
        x if x == SIXTY4TH_L_DUR => 0xE4E9,
        x if x == ONE28TH_L_DUR => 0xE4EA,
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
    // SMuFL G-clef variants: U+E050=gClef, U+E052=gClef8vb, U+E053=gClef8va
    // SMuFL F-clef variants: U+E062=fClef, U+E064=fClef8vb
    match clef_type {
        1 => 0xE053,                 // TREBLE8_CLEF  -> gClef8va (8 above)
        3 => 0xE050,                 // TREBLE_CLEF   -> gClef
        7 => 0xE052,                 // TRTENOR_CLEF  -> gClef8vb (8 below, guitar/vocal)
        4 | 5 | 6 | 8 | 9 => 0xE05C, // SOPRANO..BARITONE -> cClef
        10 => 0xE062,                // BASS_CLEF     -> fClef
        11 => 0xE064,                // BASS8B_CLEF   -> fClef8vb (8 below)
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

/// Check whether a font name refers to a Sonata-compatible music font.
///
/// Several music fonts share the Sonata character encoding (same byte → same glyph):
/// Sonata, Briard, Petrucci, Opus, Maestro, etc. If a GRAPHIC object uses any of
/// these fonts, its text characters are music symbol codes (not literal text) and
/// must be mapped through sonata_char_to_smufl().
///
/// Reference: MusicFont.cp MapMusChar(), Elbsound music font comparison
pub fn is_music_font_name(name: &str) -> bool {
    matches!(
        name,
        "Sonata"
            | "Briard"
            | "Petrucci"
            | "Opus"
            | "Opus Std"
            | "Maestro"
            | "Engraver"
            | "November"
            | "Bravura" // SMuFL, but Sonata-compat encoding possible
    )
}

/// Convert a Unicode code point (from UTF-8 text) back to the original Mac Roman
/// byte value. This is needed because NGL string pool data is stored in Mac Roman
/// encoding and gets converted to UTF-8 by `mac_roman_to_string()` during parsing.
/// For music font text, we need the original byte values to look up in
/// `sonata_char_to_smufl()`.
///
/// For ASCII (< 0x80), the byte value is unchanged. For high bytes (0x80-0xFF),
/// the Mac Roman → Unicode conversion must be reversed.
pub fn utf8_char_to_mac_roman(ch: char) -> Option<u8> {
    let cp = ch as u32;
    if cp < 0x80 {
        return Some(cp as u8);
    }
    // Mac Roman 0x80-0xFF → Unicode mapping (reverse lookup)
    // Reference: https://en.wikipedia.org/wiki/Mac_OS_Roman
    match cp {
        0x00C4 => Some(0x80), // Ä
        0x00C5 => Some(0x81), // Å
        0x00C7 => Some(0x82), // Ç
        0x00C9 => Some(0x83), // É
        0x00D1 => Some(0x84), // Ñ
        0x00D6 => Some(0x85), // Ö
        0x00DC => Some(0x86), // Ü
        0x00E1 => Some(0x87), // á
        0x00E0 => Some(0x88), // à
        0x00E2 => Some(0x89), // â
        0x00E4 => Some(0x8A), // ä
        0x00E3 => Some(0x8B), // ã
        0x00E5 => Some(0x8C), // å
        0x00E7 => Some(0x8D), // ç
        0x00E9 => Some(0x8E), // é
        0x00E8 => Some(0x8F), // è
        0x00EA => Some(0x90), // ê
        0x00EB => Some(0x91), // ë
        0x00ED => Some(0x92), // í
        0x00EC => Some(0x93), // ì
        0x00EE => Some(0x94), // î
        0x00EF => Some(0x95), // ï
        0x00F1 => Some(0x96), // ñ
        0x00F3 => Some(0x97), // ó
        0x00F2 => Some(0x98), // ò
        0x00F4 => Some(0x99), // ô
        0x00F6 => Some(0x9A), // ö
        0x00F5 => Some(0x9B), // õ
        0x00FA => Some(0x9C), // ú
        0x00F9 => Some(0x9D), // ù
        0x00FB => Some(0x9E), // û → Mac Roman 0x9E (= Sonata coda!)
        0x00FC => Some(0x9F), // ü
        0x2020 => Some(0xA0), // †
        0x00B0 => Some(0xA1), // °
        0x00A2 => Some(0xA2), // ¢
        0x00A3 => Some(0xA3), // £
        0x00A7 => Some(0xA4), // §
        0x2022 => Some(0xA5), // •
        0x00B6 => Some(0xA6), // ¶
        0x00DF => Some(0xA7), // ß
        0x00AE => Some(0xA8), // ®
        0x00A9 => Some(0xA9), // ©
        0x2122 => Some(0xAA), // ™
        0x00B4 => Some(0xAB), // ´
        0x00A8 => Some(0xAC), // ¨
        0x2260 => Some(0xAD), // ≠
        0x00C6 => Some(0xAE), // Æ
        0x00D8 => Some(0xAF), // Ø
        0x221E => Some(0xB0), // ∞
        0x00B1 => Some(0xB1), // ±
        0x2264 => Some(0xB2), // ≤
        0x2265 => Some(0xB3), // ≥
        0x00A5 => Some(0xB4), // ¥
        0x00B5 => Some(0xB5), // µ
        0x2202 => Some(0xB6), // ∂
        0x2211 => Some(0xB7), // ∑
        0x220F => Some(0xB8), // ∏
        0x03C0 => Some(0xB9), // π
        0x222B => Some(0xBA), // ∫
        0x00AA => Some(0xBB), // ª
        0x00BA => Some(0xBC), // º
        0x2126 => Some(0xBD), // Ω
        0x00E6 => Some(0xBE), // æ
        0x00F8 => Some(0xBF), // ø
        0x00BF => Some(0xC0), // ¿
        0x00A1 => Some(0xC1), // ¡
        0x00AC => Some(0xC2), // ¬
        0x221A => Some(0xC3), // √
        0x0192 => Some(0xC4), // ƒ
        0x2248 => Some(0xC5), // ≈
        0x2206 => Some(0xC6), // ∆
        0x00AB => Some(0xC7), // «
        0x00BB => Some(0xC8), // »
        0x2026 => Some(0xC9), // …
        0x00A0 => Some(0xCA), // non-breaking space
        0x00C0 => Some(0xCB), // À
        0x00C3 => Some(0xCC), // Ã
        0x00D5 => Some(0xCD), // Õ
        0x0152 => Some(0xCE), // Œ
        0x0153 => Some(0xCF), // œ
        0x2013 => Some(0xD0), // –
        0x2014 => Some(0xD1), // —
        0x201C => Some(0xD2), // "
        0x201D => Some(0xD3), // "
        0x2018 => Some(0xD4), // '
        0x2019 => Some(0xD5), // '
        0x00F7 => Some(0xD6), // ÷
        0x25CA => Some(0xD7), // ◊
        0x00FF => Some(0xD8), // ÿ
        0x0178 => Some(0xD9), // Ÿ
        0x2044 => Some(0xDA), // ⁄
        0x20AC => Some(0xDB), // €
        0x2039 => Some(0xDC), // ‹
        0x203A => Some(0xDD), // ›
        0xFB01 => Some(0xDE), // ﬁ → Mac Roman 0xDE
        0xFB02 => Some(0xDF), // ﬂ → Mac Roman 0xDF
        0x2021 => Some(0xE0), // ‡
        0x00B7 => Some(0xE1), // ·
        0x201A => Some(0xE2), // ‚
        0x201E => Some(0xE3), // „
        0x2030 => Some(0xE4), // ‰
        0x00C2 => Some(0xE5), // Â
        0x00CA => Some(0xE6), // Ê
        0x00C1 => Some(0xE7), // Á
        0x00CB => Some(0xE8), // Ë
        0x00C8 => Some(0xE9), // È
        0x00CD => Some(0xEA), // Í
        0x00CE => Some(0xEB), // Î
        0x00CF => Some(0xEC), // Ï
        0x00CC => Some(0xED), // Ì
        0x00D3 => Some(0xEE), // Ó
        0x00D4 => Some(0xEF), // Ô
        0xF8FF => Some(0xF0), //  (Apple logo)
        0x00D2 => Some(0xF1), // Ò
        0x00DA => Some(0xF2), // Ú
        0x00DB => Some(0xF3), // Û
        0x00D9 => Some(0xF4), // Ù
        0x0131 => Some(0xF5), // ı
        0x02C6 => Some(0xF6), // ˆ
        0x02DC => Some(0xF7), // ˜
        0x00AF => Some(0xF8), // ¯
        0x02D8 => Some(0xF9), // ˘
        0x02D9 => Some(0xFA), // ˙
        0x02DA => Some(0xFB), // ˚
        0x00B8 => Some(0xFC), // ¸
        0x02DD => Some(0xFD), // ˝
        0x02DB => Some(0xFE), // ˛
        0x02C7 => Some(0xFF), // ˇ
        _ => None,
    }
}

/// Map a UTF-8 character from a music font GRAPHIC's text to a SMuFL glyph.
///
/// This combines `utf8_char_to_mac_roman()` with `sonata_char_to_smufl()` to handle
/// the full pipeline: NGL Mac Roman bytes → UTF-8 string → back to Mac Roman → SMuFL.
pub fn utf8_music_char_to_smufl(ch: char) -> Option<u32> {
    utf8_char_to_mac_roman(ch).and_then(sonata_char_to_smufl)
}

/// Map a Sonata font character code (Mac Roman byte) to its SMuFL codepoint.
///
/// The Sonata font was the OG Nightingale music font. GRAPHIC text objects
/// that use Sonata (or compatible fonts like Briard) contain music character
/// codes rather than normal text. This maps those codes to SMuFL (Bravura) glyphs.
///
/// Reference: defs.h MCH_* constants (lines 137-210), vars.h (lines 325-343)
///
/// Returns None for characters that have no SMuFL equivalent or are
/// space/control characters that should be skipped.
pub fn sonata_char_to_smufl(ch: u8) -> Option<u32> {
    match ch {
        // Clefs
        0x26 => Some(0xE050), // '&' = MCH_trebleclef -> gClef
        0x42 => Some(0xE05C), // 'B' = MCH_cclef -> cClef
        0x3F => Some(0xE062), // '?' = MCH_bassclef -> fClef
        0x2F => Some(0xE069), // '/' = MCH_percclef -> unpitchedPercussionClef1

        // Accidentals
        0x23 => Some(0xE262), // '#' = MCH_sharp -> accidentalSharp
        0x62 => Some(0xE260), // 'b' = MCH_flat -> accidentalFlat
        0x6E => Some(0xE261), // 'n' = MCH_natural -> accidentalNatural
        0xBA => Some(0xE264), // SonataAcc[1] = double-flat -> accidentalDoubleFlat
        0xDC => Some(0xE263), // SonataAcc[5] = double-sharp -> accidentalDoubleSharp

        // Time signatures
        0x63 => Some(0xE08A), // 'c' = MCH_common -> timeSigCommon
        0x43 => Some(0xE08B), // 'C' = MCH_cut -> timeSigCutCommon

        // Noteheads
        0xDD => Some(0xE0A0), // MCH_breveNoteHead -> noteheadDoubleWhole
        0x77 => Some(0xE0A2), // 'w' = MCH_wholeNoteHead -> noteheadWhole
        0xFA => Some(0xE0A3), // MCH_halfNoteHead -> noteheadHalf
        0xCF => Some(0xE0A4), // MCH_quarterNoteHead -> noteheadBlack
        0xC0 => Some(0xE0A9), // MCH_xShapeHead -> noteheadXBlack
        0x4F => Some(0xE0D3), // 'O' = MCH_harmonicHead -> noteheadDiamondHalf
        0xAD => Some(0xE0B9), // MCH_squareHHead -> noteheadSquareWhite
        0xD0 => Some(0xE0B3), // MCH_squareFHead -> noteheadSquareBlack
        0xE1 => Some(0xE0D3), // MCH_diamondHHead -> noteheadDiamondHalf
        0xE2 => Some(0xE0DB), // MCH_diamondFHead -> noteheadDiamondBlack

        // Flags
        0x6A => Some(0xE240), // 'j' = MCH_eighthFlagUp -> flag8thUp
        0x4A => Some(0xE241), // 'J' = MCH_eighthFlagDown -> flag8thDown
        0x6B => Some(0xE242), // 'k' = MCH_16thFlagUp -> flag16thUp
        0x4B => Some(0xE243), // 'K' = MCH_16thFlagDown -> flag16thDown
        0xFB => Some(0xE250), // MCH_extendFlagUp -> flagInternalUp
        0xF0 => Some(0xE251), // MCH_extendFlagDown -> flagInternalDown

        // Augmentation dot
        0x2E => Some(0xE1E7), // '.' = MCH_dot -> augmentationDot

        // Articulations / Note modifiers
        0x55 => Some(0xE4C0), // 'U' = MCH_fermata -> fermataAbove
        0x75 => Some(0xE4C1), // 'u' = MCH_fermataBelow -> fermataBelow
        0x60 => Some(0xE566), // '`' = MCH_fancyTrill -> ornamentTrill
        0x3E => Some(0xE4A0), // '>' = MCH_accent -> articAccentAbove
        0x5E => Some(0xE4AC), // '^' = MCH_heavyAccent -> articMarcatoAbove
        0x76 => Some(0xE4AD), // 'v' = MCH_heavyAccentBelow -> articMarcatoBelow
        0xAE => Some(0xE4A2), // MCH_wedge -> articStaccatissimoAbove
        0x27 => Some(0xE4A3), // '\'' = MCH_wedgeBelow -> articStaccatissimoBelow
        0x2D => Some(0xE4A4), // '-' = MCH_tenuto -> articTenutoAbove
        0x4D => Some(0xE56C), // 'M' = MCH_mordent -> ornamentMordent
        0x6D => Some(0xE56D), // 'm' = MCH_invMordent -> ornamentMordentInverted
        0x54 => Some(0xE567), // 'T' = MCH_turn -> ornamentTurn
        0x2B => Some(0xE4AF), // '+' = MCH_plus -> articPlusAbove (stopped horn)
        0x6F => Some(0xE4AB), // 'o' = MCH_circle -> articHarmonicAbove
        0xB2 => Some(0xE612), // MCH_upbow -> stringsUpBow
        0xB3 => Some(0xE610), // MCH_downbow -> stringsDownBow
        0xAC => Some(0xE4AE), // MCH_heavyAccAndStaccato -> articMarcatoStaccatoAbove
        0xE8 => Some(0xE4AF), // MCH_heavyAccAndStaccatoBelow -> (approx)
        0xB5 => Some(0xE56E), // MCH_longInvMordent -> ornamentTremblement

        // Dynamic marks
        0xB8 => Some(0xE52F), // MCH_ppp -> dynamicPPP
        0xB9 => Some(0xE531), // MCH_pp -> dynamicPP
        0x70 => Some(0xE520), // 'p' = MCH_p -> dynamicPiano
        0x50 => Some(0xE52C), // 'P' = MCH_mp -> dynamicMP
        0x46 => Some(0xE52D), // 'F' = MCH_mf -> dynamicMF
        0x66 => Some(0xE522), // 'f' = MCH_f -> dynamicForte
        0xC4 => Some(0xE52F), // MCH_ff -> dynamicFF
        0xEC => Some(0xE530), // MCH_fff -> dynamicFFF
        0x53 => Some(0xE539), // 'S' = MCH_sf -> dynamicSforzando1

        // Repeat dots
        0x7B => Some(0xE043), // MCH_rptDots -> repeatDots

        // Braces/brackets
        0xC2 => Some(0xE003), // MCH_topbracket -> bracketTop (U+E003)
        0x4C => Some(0xE004), // 'L' = MCH_bottombracket -> bracketBottom
        0xA7 => Some(0xE000), // MCH_braceup -> brace (top half)
        0xEA => Some(0xE000), // MCH_bracedown -> brace (bottom half)

        // Arpeggio
        0x67 => Some(0xE63C), // 'g' = MCH_arpeggioSign -> arpeggiato

        // Parentheses (music-context)
        0x28 => Some(0xE26A), // '(' = MCH_lParen -> accidentalParensLeft
        0x29 => Some(0xE26B), // ')' = MCH_rParen -> accidentalParensRight

        // Composite note glyphs (notehead + stem, from vars.h symtable[])
        // These are used in Sonata-font GRAPHIC text objects (e.g. "* LoST: q  q")
        // to display inline music notation characters.
        // Reference: vars.h symtable[] lines 180-188
        0x68 => Some(0xE1D3), // 'h' = half note -> noteHalfUp
        0x71 => Some(0xE1D5), // 'q' = quarter note -> noteQuarterUp
        0x65 => Some(0xE1D7), // 'e' = eighth note -> note8thUp
        0x78 => Some(0xE1D9), // 'x' = 16th note -> note16thUp
        0x72 => Some(0xE1DB), // 'r' = 32nd note -> note32ndUp
        0x74 => Some(0xE1DD), // 't' = 64th note -> note64thUp
        0x79 => Some(0xE1DF), // 'y' = 128th note -> note128thUp

        // Grace note slash
        0x47 => Some(0xE560), // 'G' = MCH_graceSlash -> graceNoteSlashStemUp

        // ==== Segno and Coda ====
        // The Sonata font places segno at '%' (0x25) and coda at 0x9E.
        // Briard/Sonata also has a coda glyph at 0xDE (Mac Roman "fi ligature" position).
        0x25 => Some(0xE047), // '%' = segno -> segno (SMuFL U+E047)
        0x9E => Some(0xE048), // coda -> coda (SMuFL U+E048)
        0xDE => Some(0xE048), // coda (alternate position, used by Briard) -> coda

        // Pedal markings (from vars.h symtable[]: GRSusPedalDown at line 263)
        0xB6 => Some(0xE650), // sustain pedal down "Ped." -> keyboardPedalPed

        // Rest glyphs (MCH_rests[] from vars.h)
        0xE3 => Some(0xE4E2), // rests[0] -> restDoubleWhole (breve)
        0xB7 => Some(0xE4E3), // rests[1] -> restWhole
        0xEE => Some(0xE4E4), // rests[2] -> restHalf
        0xCE => Some(0xE4E5), // rests[3] -> restQuarter
        0xE4 => Some(0xE4E6), // rests[4] -> restEighth
        0xC5 => Some(0xE4E7), // rests[5] -> rest16th
        0xA8 => Some(0xE4E8), // rests[6] -> rest32nd
        0xF4 => Some(0xE4E9), // rests[7] -> rest64th
        0xE5 => Some(0xE4EA), // rests[8] -> rest128th

        // Space and unmapped characters
        _ => None,
    }
}
