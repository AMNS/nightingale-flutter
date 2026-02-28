//! General global definitions: object types, enumerations, and constants.
//!
//! Ported from:
//! - `Nightingale/src/Precomps/defs.h`
//! - `Nightingale/src/Precomps/NBasicTypes.h` (object types)
//! - `Nightingale/src/Precomps/NObjTypes.h` (clefs, dynamics, barlines, time sigs)
//! - `Nightingale/src/Precomps/NMiscTypes.h` (justification types)
//! - `Nightingale/src/Precomps/style.h` (style functions)

// ============================================================================
// Object Types (from NBasicTypes.h:96-129)
// ============================================================================

/// Object type: Document header
/// Source: NBasicTypes.h:99
pub const HEADER_TYPE: u8 = 0;

/// Object type: Document tail
/// Source: NBasicTypes.h:100
pub const TAIL_TYPE: u8 = 1;

/// Object type: Note/rest Sync
/// Source: NBasicTypes.h:101
pub const SYNC_TYPE: u8 = 2;

/// Object type: Repeat end
/// Source: NBasicTypes.h:102
pub const RPTEND_TYPE: u8 = 3;

/// Object type: Page
/// Source: NBasicTypes.h:103
pub const PAGE_TYPE: u8 = 4;

/// Object type: System
/// Source: NBasicTypes.h:105
pub const SYSTEM_TYPE: u8 = 5;

/// Object type: Staff
/// Source: NBasicTypes.h:106
pub const STAFF_TYPE: u8 = 6;

/// Object type: Measure
/// Source: NBasicTypes.h:107
pub const MEASURE_TYPE: u8 = 7;

/// Object type: Clef
/// Source: NBasicTypes.h:108
pub const CLEF_TYPE: u8 = 8;

/// Object type: Key signature
/// Source: NBasicTypes.h:109
pub const KEYSIG_TYPE: u8 = 9;

/// Object type: Time signature
/// Source: NBasicTypes.h:111
pub const TIMESIG_TYPE: u8 = 10;

/// Object type: Beamset
/// Source: NBasicTypes.h:112
pub const BEAMSET_TYPE: u8 = 11;

/// Object type: Connect (brace/bracket)
/// Source: NBasicTypes.h:113
pub const CONNECT_TYPE: u8 = 12;

/// Object type: Dynamic
/// Source: NBasicTypes.h:114
pub const DYNAMIC_TYPE: u8 = 13;

/// Object type: Note modifier
/// Source: NBasicTypes.h:115
pub const MODNR_TYPE: u8 = 14;

/// Object type: Graphic
/// Source: NBasicTypes.h:117
pub const GRAPHIC_TYPE: u8 = 15;

/// Object type: Ottava
/// Source: NBasicTypes.h:118
pub const OTTAVA_TYPE: u8 = 16;

/// Object type: Slur or set of ties
/// Source: NBasicTypes.h:119
pub const SLUR_TYPE: u8 = 17;

/// Object type: Tuplet
/// Source: NBasicTypes.h:120
pub const TUPLET_TYPE: u8 = 18;

/// Object type: Grace note Sync
/// Source: NBasicTypes.h:121
pub const GRSYNC_TYPE: u8 = 19;

/// Object type: Tempo
/// Source: NBasicTypes.h:123
pub const TEMPO_TYPE: u8 = 20;

/// Object type: Spacer
/// Source: NBasicTypes.h:124
pub const SPACER_TYPE: u8 = 21;

/// Object type: Ending
/// Source: NBasicTypes.h:125
pub const ENDING_TYPE: u8 = 22;

/// Object type: Pseudomeasure
/// Source: NBasicTypes.h:126
pub const PSMEAS_TYPE: u8 = 23;

/// Object heap type (must be last heap)
/// Source: NBasicTypes.h:127
pub const OBJ_TYPE: u8 = 24;

/// First object type
/// Source: NBasicTypes.h:131
pub const LOW_TYPE: u8 = HEADER_TYPE;

/// Last object type
/// Source: NBasicTypes.h:132
pub const HIGH_TYPE: u8 = 25; // LASTtype

// ============================================================================
// Font Style Indices (from defs.h:6-23)
// ============================================================================

/// Font style: This item only
/// Source: defs.h:7
pub const FONT_THISITEMONLY: u8 = 0;

/// Font style: Measure numbers
/// Source: defs.h:8
pub const FONT_MN: u8 = 1;

/// Font style: Part names
/// Source: defs.h:9
pub const FONT_PN: u8 = 2;

/// Font style: Regular music text
/// Source: defs.h:10
pub const FONT_RM: u8 = 3;

/// Font style: Regular 1
/// Source: defs.h:11
pub const FONT_R1: u8 = 4;

/// Font style: Regular 2
/// Source: defs.h:12
pub const FONT_R2: u8 = 5;

/// Font style: Regular 3
/// Source: defs.h:13
pub const FONT_R3: u8 = 6;

/// Font style: Regular 4
/// Source: defs.h:14
pub const FONT_R4: u8 = 7;

/// Font style: Tempo marks
/// Source: defs.h:15
pub const FONT_TM: u8 = 8;

/// Font style: Chord symbols
/// Source: defs.h:16
pub const FONT_CS: u8 = 9;

/// Font style: Page numbers
/// Source: defs.h:17
pub const FONT_PG: u8 = 10;

/// Font style: Regular 5
/// Source: defs.h:18
pub const FONT_R5: u8 = 11;

/// Font style: Regular 6
/// Source: defs.h:19
pub const FONT_R6: u8 = 12;

/// Font style: Regular 7
/// Source: defs.h:20
pub const FONT_R7: u8 = 13;

/// Font style: Regular 8
/// Source: defs.h:21
pub const FONT_R8: u8 = 14;

/// Font style: Regular 9
/// Source: defs.h:22
pub const FONT_R9: u8 = 15;

// ============================================================================
// Duration Codes (from defs.h:38-52)
// ============================================================================

/// Duration code: Whole measure rest
/// Source: defs.h:39
pub const WHOLEMR_L_DUR: i8 = -1;

/// Duration code: Unknown CMN value
/// Source: defs.h:40
pub const UNKNOWN_L_DUR: i8 = 0;

/// Duration code: Breve
/// Source: defs.h:41
pub const BREVE_L_DUR: i8 = 1;

/// Duration code: Whole note/rest (not whole measure rest)
/// Source: defs.h:42
pub const WHOLE_L_DUR: i8 = 2;

/// Duration code: Half note/rest
/// Source: defs.h:43
pub const HALF_L_DUR: i8 = 3;

/// Duration code: Quarter note/rest
/// Source: defs.h:44
pub const QTR_L_DUR: i8 = 4;

/// Duration code: Eighth note/rest
/// Source: defs.h:45
pub const EIGHTH_L_DUR: i8 = 5;

/// Duration code: 16th note/rest
/// Source: defs.h:46
pub const SIXTEENTH_L_DUR: i8 = 6;

/// Duration code: 32nd note/rest
/// Source: defs.h:47
pub const THIRTY2ND_L_DUR: i8 = 7;

/// Duration code: 64th note/rest
/// Source: defs.h:48
pub const SIXTY4TH_L_DUR: i8 = 8;

/// Duration code: 128th note/rest
/// Source: defs.h:49
pub const ONE28TH_L_DUR: i8 = 9;

/// Duration code: None (illegal)
/// Source: defs.h:50
pub const NO_L_DUR: i8 = 10;

/// Last duration code
/// Source: defs.h:51
pub const LAST_L_DUR: i8 = NO_L_DUR;

// ============================================================================
// Accidental Codes (from defs.h:54-60)
// ============================================================================

/// Accidental: Double flat
/// Source: defs.h:55
pub const AC_DBLFLAT: u8 = 1;

/// Accidental: Flat
/// Source: defs.h:56
pub const AC_FLAT: u8 = 2;

/// Accidental: Natural
/// Source: defs.h:57
pub const AC_NATURAL: u8 = 3;

/// Accidental: Sharp
/// Source: defs.h:58
pub const AC_SHARP: u8 = 4;

/// Accidental: Double sharp
/// Source: defs.h:59
pub const AC_DBLSHARP: u8 = 5;

// ============================================================================
// Clef Types (from NObjTypes.h:299-314)
// ============================================================================

/// Clef: Treble 8va (unused)
/// Source: NObjTypes.h:299
pub const TREBLE8_CLEF: u8 = 1;

/// Clef: French violin (unused)
/// Source: NObjTypes.h:300
pub const FRVIOLIN_CLEF: u8 = 2;

/// Clef: Treble
/// Source: NObjTypes.h:301
pub const TREBLE_CLEF: u8 = 3;

/// Clef: Soprano
/// Source: NObjTypes.h:302
pub const SOPRANO_CLEF: u8 = 4;

/// Clef: Mezzo-soprano
/// Source: NObjTypes.h:303
pub const MZSOPRANO_CLEF: u8 = 5;

/// Clef: Alto
/// Source: NObjTypes.h:304
pub const ALTO_CLEF: u8 = 6;

/// Clef: Treble-tenor (C clef on 4th line)
/// Source: NObjTypes.h:305
pub const TRTENOR_CLEF: u8 = 7;

/// Clef: Tenor
/// Source: NObjTypes.h:306
pub const TENOR_CLEF: u8 = 8;

/// Clef: Baritone
/// Source: NObjTypes.h:307
pub const BARITONE_CLEF: u8 = 9;

/// Clef: Bass
/// Source: NObjTypes.h:308
pub const BASS_CLEF: u8 = 10;

/// Clef: Bass 8vb (unused)
/// Source: NObjTypes.h:309
pub const BASS8B_CLEF: u8 = 11;

/// Clef: Percussion
/// Source: NObjTypes.h:310
pub const PERC_CLEF: u8 = 12;

/// Lowest clef type
/// Source: NObjTypes.h:313
pub const LOW_CLEF: u8 = TREBLE8_CLEF;

/// Highest clef type
/// Source: NObjTypes.h:314
pub const HIGH_CLEF: u8 = PERC_CLEF;

// ============================================================================
// Barline Types (from NObjTypes.h:271-280)
// ============================================================================

/// Barline: Single
/// Source: NObjTypes.h:272
pub const BAR_SINGLE: u8 = 1;

/// Barline: Double
/// Source: NObjTypes.h:273
pub const BAR_DOUBLE: u8 = 2;

/// Barline: Final double
/// Source: NObjTypes.h:274
pub const BAR_FINALDBL: u8 = 3;

/// Barline: Heavy double (unused)
/// Source: NObjTypes.h:275
pub const BAR_HEAVYDBL: u8 = 4;

/// Barline: Repeat left (codes match equivalent RPTENDs)
/// Source: NObjTypes.h:276
pub const BAR_RPT_L: u8 = 5;

/// Barline: Repeat right
/// Source: NObjTypes.h:277
pub const BAR_RPT_R: u8 = 6;

/// Barline: Repeat both sides
/// Source: NObjTypes.h:278
pub const BAR_RPT_LR: u8 = 7;

/// Last barline type
/// Source: NObjTypes.h:279
pub const BAR_LAST: u8 = BAR_RPT_LR;

// ============================================================================
// Time Signature Types (from NObjTypes.h:352-366)
// ============================================================================

/// Time signature: Numerator over denominator
/// Source: NObjTypes.h:353
pub const N_OVER_D: i8 = 1;

/// Time signature: Common time (C)
/// Source: NObjTypes.h:354
pub const C_TIME: i8 = 2;

/// Time signature: Cut time (C with slash)
/// Source: NObjTypes.h:355
pub const CUT_TIME: i8 = 3;

/// Time signature: Numerator only
/// Source: NObjTypes.h:356
pub const N_ONLY: i8 = 4;

/// Time signature: Zero (no time signature)
/// Source: NObjTypes.h:357
pub const ZERO_TIME: i8 = 5;

/// Time signature: Numerator over quarter note
/// Source: NObjTypes.h:358
pub const N_OVER_QUARTER: i8 = 6;

/// Time signature: Numerator over eighth note
/// Source: NObjTypes.h:359
pub const N_OVER_EIGHTH: i8 = 7;

/// Time signature: Numerator over half note
/// Source: NObjTypes.h:360
pub const N_OVER_HALF: i8 = 8;

/// Time signature: Numerator over dotted quarter
/// Source: NObjTypes.h:361
pub const N_OVER_DOTTEDQUARTER: i8 = 9;

/// Time signature: Numerator over dotted eighth
/// Source: NObjTypes.h:362
pub const N_OVER_DOTTEDEIGHTH: i8 = 10;

/// Lowest time signature type
/// Source: NObjTypes.h:365
pub const LOW_TSTYPE: i8 = N_OVER_D;

/// Highest time signature type
/// Source: NObjTypes.h:366
pub const HIGH_TSTYPE: i8 = N_OVER_DOTTEDEIGHTH;

// ============================================================================
// Dynamic Types (from NObjTypes.h:466-493)
// ============================================================================

/// Dynamic: pppp
/// Source: NObjTypes.h:467
pub const PPPP_DYNAM: u8 = 1;

/// Dynamic: ppp
/// Source: NObjTypes.h:468
pub const PPP_DYNAM: u8 = 2;

/// Dynamic: pp
/// Source: NObjTypes.h:469
pub const PP_DYNAM: u8 = 3;

/// Dynamic: p
/// Source: NObjTypes.h:470
pub const P_DYNAM: u8 = 4;

/// Dynamic: mp
/// Source: NObjTypes.h:471
pub const MP_DYNAM: u8 = 5;

/// Dynamic: mf
/// Source: NObjTypes.h:472
pub const MF_DYNAM: u8 = 6;

/// Dynamic: f
/// Source: NObjTypes.h:473
pub const F_DYNAM: u8 = 7;

/// Dynamic: ff
/// Source: NObjTypes.h:474
pub const FF_DYNAM: u8 = 8;

/// Dynamic: fff
/// Source: NObjTypes.h:475
pub const FFF_DYNAM: u8 = 9;

/// Dynamic: ffff
/// Source: NObjTypes.h:476
pub const FFFF_DYNAM: u8 = 10;

/// First relative dynamic
/// Source: NObjTypes.h:477
pub const FIRSTREL_DYNAM: u8 = 11;

/// Dynamic: piu p (more soft)
/// Source: NObjTypes.h:478
pub const PIUP_DYNAM: u8 = FIRSTREL_DYNAM;

/// Dynamic: meno p (less soft)
/// Source: NObjTypes.h:479
pub const MENOP_DYNAM: u8 = 12;

/// Dynamic: meno f (less loud)
/// Source: NObjTypes.h:480
pub const MENOF_DYNAM: u8 = 13;

/// Dynamic: piu f (more loud)
/// Source: NObjTypes.h:481
pub const PIUF_DYNAM: u8 = 14;

/// First sf dynamic
/// Source: NObjTypes.h:482
pub const FIRSTSF_DYNAM: u8 = 15;

/// Dynamic: sf
/// Source: NObjTypes.h:483
pub const SF_DYNAM: u8 = FIRSTSF_DYNAM;

/// Dynamic: fz
/// Source: NObjTypes.h:484
pub const FZ_DYNAM: u8 = 16;

/// Dynamic: sfz
/// Source: NObjTypes.h:485
pub const SFZ_DYNAM: u8 = 17;

/// Dynamic: rf
/// Source: NObjTypes.h:486
pub const RF_DYNAM: u8 = 18;

/// Dynamic: rfz
/// Source: NObjTypes.h:487
pub const RFZ_DYNAM: u8 = 19;

/// Dynamic: fp
/// Source: NObjTypes.h:488
pub const FP_DYNAM: u8 = 20;

/// Dynamic: sfp
/// Source: NObjTypes.h:489
pub const SFP_DYNAM: u8 = 21;

/// First hairpin dynamic (ONLY hairpins after this)
/// Source: NObjTypes.h:490
pub const FIRSTHAIRPIN_DYNAM: u8 = 22;

/// Dynamic: Diminuendo (hairpin open at left)
/// Source: NObjTypes.h:491
pub const DIM_DYNAM: u8 = FIRSTHAIRPIN_DYNAM;

/// Dynamic: Crescendo (hairpin open at right)
/// Source: NObjTypes.h:492
pub const CRESC_DYNAM: u8 = 23;

// ============================================================================
// Justification Types (from NMiscTypes.h:16-21)
// ============================================================================

/// Justification: Independent, Totally ordered
/// Source: NMiscTypes.h:17
pub const J_IT: u8 = 1;

/// Justification: Independent, Partially ordered
/// Source: NMiscTypes.h:18
pub const J_IP: u8 = 2;

/// Justification: Dependent
/// Source: NMiscTypes.h:19
pub const J_D: u8 = 3;

/// Structural object, no justification type
/// Source: NMiscTypes.h:20
pub const J_STRUC: u8 = 4;

// ============================================================================
// Special Constants
// ============================================================================

/// No link (null link pointer)
/// Source: defs.h comments and usage throughout codebase
pub const NILINK: u16 = 0;

/// Standard number of lines in staff
/// Source: defs.h:296
pub const STFLINES: u8 = 5;

/// Height of standard staff in half-spaces
/// Source: defs.h:297
pub const STFHALFSP: u8 = STFLINES + STFLINES - 2;

/// Resolution factor for justification routines
/// Source: defs.h:302
pub const RESFACTOR: i32 = 50;

/// Points per inch (approximately; really about 72.27)
/// Source: defs.h:257
pub const POINTSPERIN: u8 = 72;

/// p_dur code for shortest note (w/dur.code=MAX_L_DUR)
/// Source: defs.h:259
pub const PDURUNIT: i16 = 15;

/// Staff half-space offset in accTable
/// Source: defs.h:261
pub const ACCTABLE_OFF: u8 = 30;

/// A very large signed long, less than LONG_MAX but not much less
/// Source: defs.h:270
pub const BIGNUM: i32 = 1999999999;

/// Any type wildcard
/// Source: defs.h:272
pub const ANYTYPE: i8 = -1;

/// Any subtype wildcard
/// Source: defs.h:273
pub const ANYSUBTYPE: i8 = -1;

/// Any voice/staff wildcard
/// Source: defs.h:275
pub const ANYONE: i8 = -1;

/// No one (returned by FindStaffSetSys, GetStaffFromSel, etc.)
/// Source: defs.h:276
pub const NOONE: i8 = -2;

/// No match
/// Source: defs.h:278
pub const NOMATCH: i8 = -1;

// ============================================================================
// Default Values
// ============================================================================

/// Default clef type for new staves
/// Source: defs.h:286
pub const DFLT_CLEF: u8 = TREBLE_CLEF;

/// Default key signature items
/// Source: defs.h:287
pub const DFLT_NKSITEMS: u8 = 0;

/// Default time signature type
/// Source: defs.h:288
pub const DFLT_TSTYPE: i8 = N_OVER_D;

/// Default time signature numerator
/// Source: defs.h:289
pub const DFLT_NUMER: i8 = 4;

/// Default time signature denominator
/// Source: defs.h:290
pub const DFLT_DENOM: i8 = 4;

/// Default dynamic marking for new staves
/// Source: defs.h:292
pub const DFLT_DYNAMIC: u8 = MF_DYNAM;

/// Default space table number
/// Source: defs.h:293
pub const DFLT_SPACETABLE: u8 = 0;

/// Default note xMoveAcc
/// Source: defs.h:294
pub const DFLT_XMOVEACC: i8 = 5;

// ============================================================================
// Style Functions (from style.h)
// ============================================================================

/// Width of common (beamable) note heads
/// Source: style.h:130
#[inline]
pub fn head_width(ln_sp: i16) -> i16 {
    9 * ln_sp / 8
}

/// Vertical distance between flags
/// Source: style.h:68
#[inline]
pub fn flag_leading(ln_sp: i16) -> i16 {
    3 * ln_sp / 4
}

/// Size for "small" versions of symbols (notes/rests, clefs, dynamics, etc.)
/// Source: style.h:16
#[inline]
pub fn small_size(size: i16) -> i16 {
    3 * size / 4
}

/// Size for grace notes
/// Source: style.h:15
#[inline]
pub fn grace_size(size: i16) -> i16 {
    7 * size / 10
}

/// Size for metronome marks
/// Source: style.h:17
#[inline]
pub fn metro_size(size: i16) -> i16 {
    8 * size / 10
}

/// Length of ledger line on notehead's side of stem
/// Source: style.h:53
#[inline]
pub fn ledger_len(ln_sp: i16) -> i16 {
    12 * ln_sp / 8
}

/// Length of ledger line on side away from notehead
/// Source: style.h:54
#[inline]
pub fn ledger_other_len(ln_sp: i16) -> i16 {
    3 * ln_sp / 8
}

/// Fractional beam length
/// Source: style.h:131
#[inline]
pub fn frac_beam_width(ln_sp: i16) -> i16 {
    ln_sp
}

/// PostScript thickness of tuplet bracket lines
/// Source: style.h:116
#[inline]
pub fn tuple_brackthick(ln_sp: i16) -> i16 {
    6 * ln_sp / 50
}

/// PostScript thickness of dotted lines in ottavas
/// Source: style.h:111
#[inline]
pub fn ottava_thick(ln_sp: i16) -> i16 {
    6 * ln_sp / 50
}

/// Length of vertical cutoff line in ottavas
/// Source: style.h:112
#[inline]
pub fn ottava_cutofflen(ln_sp: i16) -> i16 {
    ln_sp
}

/// PostScript thickness of lines in endings
/// Source: style.h:121
#[inline]
pub fn ending_thick(ln_sp: i16) -> i16 {
    6 * ln_sp / 50
}

/// Length of ending's vertical cutoff line
/// Source: style.h:122
#[inline]
pub fn ending_cutofflen(ln_sp: i16) -> i16 {
    2 * ln_sp
}

// ============================================================================
// Graphic Justification Types (from NObjTypes.h:612-617)
// ============================================================================

/// GRAPHIC text justification: left-aligned (default)
/// Source: NObjTypes.h:613
pub const GR_JUST_LEFT: u8 = 1;

/// GRAPHIC text justification: right-aligned
/// Source: NObjTypes.h:614
pub const GR_JUST_RIGHT: u8 = 2;

/// GRAPHIC text justification: both (left + right)
/// Source: NObjTypes.h:615
pub const GR_JUST_BOTH: u8 = 3;

/// GRAPHIC text justification: centered
/// Source: NObjTypes.h:616
pub const GR_JUST_CENTER: u8 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_type_constants() {
        assert_eq!(HEADER_TYPE, 0);
        assert_eq!(SYNC_TYPE, 2);
        assert_eq!(OBJ_TYPE, 24);
    }

    #[test]
    fn test_duration_codes() {
        assert_eq!(WHOLEMR_L_DUR, -1);
        assert_eq!(BREVE_L_DUR, 1);
        assert_eq!(WHOLE_L_DUR, 2);
        assert_eq!(QTR_L_DUR, 4);
        assert_eq!(EIGHTH_L_DUR, 5);
    }

    #[test]
    fn test_clef_types() {
        assert_eq!(TREBLE_CLEF, 3);
        assert_eq!(ALTO_CLEF, 6);
        assert_eq!(BASS_CLEF, 10);
    }

    #[test]
    fn test_style_functions() {
        // Test head_width: 9 * ln_sp / 8
        assert_eq!(head_width(16), 18);

        // Test flag_leading: 3 * ln_sp / 4
        assert_eq!(flag_leading(16), 12);

        // Test small_size: 3 * size / 4
        assert_eq!(small_size(12), 9);

        // Test grace_size: 7 * size / 10
        assert_eq!(grace_size(10), 7);
    }

    #[test]
    fn test_justification_types() {
        assert_eq!(J_IT, 1);
        assert_eq!(J_IP, 2);
        assert_eq!(J_D, 3);
        assert_eq!(J_STRUC, 4);
    }
}
