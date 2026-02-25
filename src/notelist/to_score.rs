//! Convert a parsed Notelist into an InterpretedScore.
//!
//! This is a faithful port of the Nightingale C++ Notelist import pipeline
//! (NotelistOpen.cp + NotelistParse.cp), adapted for our Rust data model.
//!
//! ## Architecture
//!
//! The C++ pipeline works in stages:
//! 1. Parse text → NL_NODE intermediate array (NotelistParse.cp)
//! 2. Create empty Document with correct staves/clefs/keysigs (SetupNLScore)
//! 3. Walk NL_NODEs, converting each to Nightingale objects (NotelistToNight)
//!    - ConvertNoteRest: uses IIInsertSync + SetupNote
//!    - ConvertBarline: uses IIInsertBarline
//!    - ConvertClef: uses IIInsertClef
//!    - etc.
//! 4. Post-process: FixTimeStamps, RespaceBars, AutoBeam, IIAutoMultiVoice, etc.
//!
//! Our port skips step 4 (we don't have the layout engine yet). Instead, we
//! pre-compute positions using:
//! - NLMIDI2HalfLn (brute-force pitch→staff-position table from NotelistOpen.cp)
//! - ClefMiddleCHalfLn (clef→middle-C-position from PitchUtils.cp)
//! - Simple uniform horizontal spacing
//!
//! ## Limitations (known, to resolve later)
//!
//! - No proportional spacing (uniform X advance per time position)
//! - No beaming (would need AutoBeam port)
//! - No slur/tie rendering
//! - No dynamics, text, or tempo mark rendering
//! - Grace notes not yet positioned
//! - No page breaks (single-page layout, multi-system)
//!
//! ## Reference
//!
//! - NotelistOpen.cp: ConvertNoteRest(), NLMIDI2HalfLn(), SetupNLScore()
//! - PitchUtils.cp: ClefMiddleCHalfLn()

use crate::basic_types::*;
use crate::defs::{CLEF_TYPE, EIGHTH_L_DUR, TIMESIG_TYPE};
use crate::duration::{beat_l_dur, code_to_l_dur};
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::notelist::parser::{Notelist, NotelistRecord};
use crate::obj_types::*;
use std::collections::BTreeSet;

// ===========================================================================
// Voice roles — port of Multivoice.h constants
// ===========================================================================

/// Voice role constants from Multivoice.h.
/// Determines stem direction and stem length for each voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceRole {
    /// Single voice on staff: traditional midline-based stem direction.
    /// (VCROLE_SINGLE = 6 in OG)
    Single,
    /// Upper voice in multi-voice notation: stems always UP.
    /// (VCROLE_UPPER = 3 in OG)
    Upper,
    /// Lower voice in multi-voice notation: stems always DOWN.
    /// (VCROLE_LOWER = 4 in OG)
    Lower,
}

// ===========================================================================
// Pitch conversion — faithful port from C++
// ===========================================================================

/// Accidental codes (from NObjTypes.h).
pub const AC_DBLFLAT: u8 = 1;
pub const AC_FLAT: u8 = 2;
pub const AC_NATURAL: u8 = 3;
pub const AC_SHARP: u8 = 4;
pub const AC_DBLSHARP: u8 = 5;

/// Marker for invalid table entries.
const XX: i8 = -99;

/// NLMIDI2HalfLn — brute-force pitch-to-staff-position conversion.
///
/// Given a MIDI note number, effective accidental code, and the half-line
/// position of middle C for the current clef, returns the half-line number
/// relative to the top staff line (0 = top line, positive = downward).
///
/// This is a direct port of NLMIDI2HalfLn from NotelistOpen.cp (line 579).
///
/// Returns `None` if the combination is invalid (e.g., C-natural can't be Db).
pub fn nl_midi_to_half_ln(note_num: u8, e_acc: u8, mid_c_half_ln: i16) -> Option<i16> {
    // Brute-force lookup table: [accidental_index][pitch_class] → half-line offset
    // Index: eAcc - AC_DBLFLAT (so 0=dblflat, 1=flat, 2=natural, 3=sharp, 4=dblsharp)
    //
    // Reference: NotelistOpen.cp lines 583-597
    #[rustfmt::skip]
    static HL_TABLE: [[i8; 12]; 5] = [
        // AC_DBLFLAT:   Dbb C#  Ebb Fbb E   Gbb Gb  Abb Ab  Bbb Cbb B
        [  1,  XX,  2,   3,  XX,  4,  XX,  5,  XX,  6,   7, XX ],
        // AC_FLAT:      C   Db  D   Eb  Fb  F   Gb  G   Ab  A   Bb  Cb
        [ XX,   1, XX,   2,   3, XX,   4, XX,   5, XX,   6,  7 ],
        // AC_NATURAL:   C   C#  D   D#  E   F   F#  G   G#  A   A#  B
        [  0,  XX,  1,  XX,   2,  3,  XX,  4,  XX,  5,  XX,  6 ],
        // AC_SHARP:     B#  C#  D   D#  E   E#  F#  G   G#  A   A#  B
        [ -1,   0, XX,   1,  XX,  2,   3, XX,   4, XX,   5, XX ],
        // AC_DBLSHARP:  C   Bx  Cx  D#  Dx  F   Ex  Fx  G#  Gx  A#  Ax
        [ XX,  -1,  0,  XX,   1, XX,   2,  3,  XX,  4,  XX,  5 ],
    ];

    if !(AC_DBLFLAT..=AC_DBLSHARP).contains(&e_acc) {
        return None;
    }

    let pitch_class = (note_num % 12) as usize;
    let acc_idx = (e_acc - AC_DBLFLAT) as usize;
    let half_steps = HL_TABLE[acc_idx][pitch_class];

    if half_steps == XX {
        return None;
    }

    let octave = (note_num as i16 / 12) - 5;
    let half_lines = octave * 7 + half_steps as i16;

    Some(-half_lines + mid_c_half_ln)
}

/// ClefMiddleCHalfLn — get half-line position of middle C for a given clef.
///
/// Direct port from PitchUtils.cp (line 118).
/// Returns the staff half-line number where middle C sits.
/// Top line of staff = 0, each half-line step goes down by 1.
///
/// Clef type codes (from NObjTypes.h):
///   1=TREBLE8, 2=FRVIOLIN(unused), 3=TREBLE, 4=SOPRANO, 5=MZSOPRANO,
///   6=ALTO, 7=TRTENOR, 8=TENOR, 9=BARITONE, 10=BASS, 11=BASS8B, 12=PERC
pub fn clef_middle_c_half_ln(clef_type: u8) -> i16 {
    match clef_type {
        1 => 17,  // TREBLE8_CLEF
        3 => 10,  // TREBLE_CLEF
        4 => 8,   // SOPRANO_CLEF
        5 => 6,   // MZSOPRANO_CLEF
        6 => 4,   // ALTO_CLEF
        7 => 3,   // TRTENOR_CLEF
        8 => 2,   // TENOR_CLEF
        9 => 0,   // BARITONE_CLEF
        10 => -2, // BASS_CLEF
        11 => -9, // BASS8B_CLEF
        12 => 10, // PERC_CLEF (same as treble)
        _ => 10,  // Default to treble
    }
}

/// Half-line position to DDIST Y offset from staff top.
///
/// Each half-line = half the inter-line distance.
/// In a standard 5-line staff with staff_height, inter-line = staff_height / 4.
/// Half-line 0 = top line, half-line 8 = bottom line.
fn half_ln_to_yd(half_ln: i16, staff_height: Ddist) -> Ddist {
    // Inter-line distance in DDIST
    let d_interline = staff_height / 4; // For 5-line staff
                                        // Each half-line = half of d_interline
    let d_half_line = d_interline / 2;
    half_ln * d_half_line
}

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
fn calc_ystem(
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

/// NFLAGS — number of flags for a given duration code.
///
/// Port of NFLAGS macro from defs.h.
/// l_dur: BREVE=1, WHOLE=2, HALF=3, QTR=4, 8TH=5, 16TH=6, 32ND=7, 64TH=8
fn nflags(l_dur: i8) -> i16 {
    match l_dur {
        x if x >= 5 => (x - 4) as i16, // 8th=1, 16th=2, 32nd=3, 64th=4
        _ => 0,                        // quarter and longer have no flags
    }
}

/// HeadWidth — width of common (beamable) note heads.
///
/// Port of HeadWidth macro from defs.h:355:
///   #define HeadWidth(lnSp) (9*(lnSp)*4/32)
///
/// Simplifies to: 9*lnSp/8 = 1.125 * lnSpace
#[allow(dead_code)]
fn head_width(ln_space: Ddist) -> Ddist {
    (9 * ln_space * 4) / 32
}

/// STD_ACCWIDTH — width of common accidentals in STDIST units.
///
/// From defs.h:318: #define STD_ACCWIDTH (9*STD_LINEHT/8) = 9*8/8 = 9
#[allow(dead_code)]
const STD_ACCWIDTH: i16 = 9;

/// STD_LINEHT — STDIST units per interline space.
#[allow(dead_code)]
const STD_LINEHT: i16 = 8;

/// DFLT_XMOVEACC — default note xMoveAcc value.
///
/// From defs.h:256.
const DFLT_XMOVEACC: i16 = 5;

/// std2d — convert STDIST to DDIST.
///
/// Port of std2d macro from defs.h:521:
///   #define std2d(std, stfHt, lines) ( ((long)(std)*(stfHt)) / (8*((lines)-1)) )
#[allow(dead_code)]
fn std2d(stdist: i16, staff_height: Ddist, staff_lines: i16) -> Ddist {
    (stdist as i32 * staff_height as i32 / (STD_LINEHT as i32 * (staff_lines as i32 - 1))) as Ddist
}

/// AccXOffset — compute X offset for accidental placement.
///
/// Port of AccXOffset from DrawNRGR.cp:396-406.
/// Returns DDIST offset to place accidental to the left of the notehead.
#[allow(dead_code)]
fn acc_x_offset(xmove_acc: i16, staff_height: Ddist, staff_lines: i16) -> Ddist {
    let d_acc_width = std2d(STD_ACCWIDTH, staff_height, staff_lines);
    let mut x_offset = d_acc_width; // Default offset
    x_offset += (d_acc_width * (xmove_acc - DFLT_XMOVEACC)) / 4; // Fine-tune
    x_offset
}

/// MusFontStemSpaceWidthDDIST — stem-space width (distance from left edge of
/// notehead to where the stem attaches on the right side for stems-up).
///
/// Port of MusFontStemSpaceWidthDDIST from MusicFont.cp.
/// For Bravura, we use the notehead width from metadata (1.18 staff spaces),
/// which maps to stemSpaceWidth = 118 (percent of interline space).
/// Sonata's stemSpaceWidth was typically ~80.
///
/// Formula: (stemSpaceWidth * lnSpace) / 100
#[allow(dead_code)]
fn stem_space_width_ddist(ln_space: Ddist) -> Ddist {
    // Bravura noteheadBlack bbox NE.x = 1.18 spaces → 118% of lnSpace
    // This is the distance from the left edge of the notehead to the right edge
    // where an upstem attaches.
    (118 * ln_space as i32 / 100) as Ddist
}

// ===========================================================================
// Layout parameters
// ===========================================================================

/// Layout configuration for the Notelist typesetter.
#[derive(Debug, Clone)]
pub struct NotelistLayoutConfig {
    /// Page width in points (US Letter = 612).
    pub page_width: i16,
    /// Page height in points (US Letter = 792).
    pub page_height: i16,
    /// Staff rastral size (5 = default from NotelistOpen.cp NL_RASTRAL).
    pub rastral: u8,
    /// Staff height in DDIST (derived from rastral).
    pub staff_height: Ddist,
    /// System left margin in DDIST.
    pub system_left: Ddist,
    /// System right limit (page_width - right_margin) in DDIST.
    pub system_right: Ddist,
    /// System top in DDIST.
    pub system_top: Ddist,
    /// Distance between systems in DDIST (for multi-system layout).
    pub inter_system: Ddist,
    /// Distance between staves within a system in DDIST (top-to-top).
    pub inter_staff: Ddist,
    /// Default stem length in quarter-spaces (OG Nightingale config.stemLenNormal).
    /// Default: 14 (= 3.5 interline spaces). Reference: Initialize.cp:757.
    pub stem_len_normal: i8,
    /// Stem length for 2-voice notation (quarter-spaces).
    /// Default: 12. Reference: Initialize.cp:758.
    pub stem_len_2v: i8,
    /// Stem length when stem is entirely outside staff (quarter-spaces).
    /// Default: 10. Reference: Initialize.cp:759.
    pub stem_len_outside: i8,
    /// Maximum measures per system (0 = no limit).
    pub max_measures: usize,
    /// Maximum voices per staff (0 = no limit). Voice 1 is always included;
    /// higher-numbered voices are filtered out if this is set to 1.
    pub max_voices_per_staff: usize,
    /// Rest vertical offset (in half-lines) for multi-voice notation.
    /// Rests in upper voice move up by this amount, lower voice down.
    /// Default: 2 (= 1 staff space). Reference: config.restMVOffset (Initialize.cp).
    pub rest_mv_offset: i16,
    /// If true, skip events before the first barline (anacrusis/pickup).
    pub skip_anacrusis: bool,
    /// Beam slope as percentage of natural slope (0=flat, 100=full slope).
    /// OG default: 33 (config.relBeamSlope). Reference: Beam.cp:214.
    pub rel_beam_slope: i16,
}

impl Default for NotelistLayoutConfig {
    fn default() -> Self {
        // Standard values matching Nightingale defaults.
        // Rastral 5 → staff height ≈ 24 points = 384 DDIST (24 * 16).
        // Line spacing = 384/4 = 96 DDIST = 6 points.
        let staff_height: Ddist = 384; // 24pt staff
        let page_width: i16 = 612; // US Letter portrait (8.5")
        let margin_left_pt: i16 = 72; // 1 inch
        let margin_right_pt: i16 = 54;
        let margin_top_pt: i16 = 72;

        Self {
            page_width,
            page_height: 792, // US Letter portrait (11")
            rastral: 5,
            staff_height,
            system_left: margin_left_pt * 16, // 1152 DDIST
            system_right: (page_width - margin_right_pt) * 16, // 8928 DDIST
            system_top: margin_top_pt * 16,   // 1152 DDIST
            inter_system: 2800,               // ~175pt (system height + gap)
            inter_staff: staff_height * 5 / 2, // 2.5× staff height (Score.cp:200 initStfTop2)
            stem_len_normal: 14,              // 3.5 interline spaces (Initialize.cp:757)
            stem_len_2v: 12,                  // 3 interline spaces (Initialize.cp:758)
            stem_len_outside: 10,             // 2.5 interline spaces (Initialize.cp:759)
            max_measures: 4,                  // Anacrusis + 3 full measures
            max_voices_per_staff: 0,          // No limit — render all voices
            rest_mv_offset: 2,                // 1 staff space (Initialize.cp config.restMVOffset)
            skip_anacrusis: false,            // Include pickup beats by default
            rel_beam_slope: 33,               // 33% of natural slope (Beam.cp:214)
        }
    }
}

impl NotelistLayoutConfig {
    /// Usable music width in DDIST.
    fn content_width(&self) -> Ddist {
        self.system_right - self.system_left
    }

    /// Inter-line distance in DDIST.
    #[allow(dead_code)]
    fn d_interline(&self) -> Ddist {
        self.staff_height / 4
    }
}

// ===========================================================================
// Score builder
// ===========================================================================

/// Convert a parsed Notelist into an InterpretedScore.
///
/// This is the main entry point — equivalent to NotelistToNight() in the C++.
///
/// The resulting score has:
/// - Correct object structure (Header→Page→System→Staff→[Measure/Sync]→Tail)
/// - Pre-computed DDIST positions for all notes/rests (via NLMIDI2HalfLn)
/// - Simple uniform horizontal spacing
pub fn notelist_to_score(notelist: &Notelist) -> InterpretedScore {
    notelist_to_score_with_config(notelist, &NotelistLayoutConfig::default())
}

/// Convert with explicit layout configuration.
pub fn notelist_to_score_with_config(
    notelist: &Notelist,
    config: &NotelistLayoutConfig,
) -> InterpretedScore {
    let mut score = InterpretedScore::new();

    // Determine number of staves from header
    let num_staves: usize = notelist
        .part_staves
        .iter()
        .filter(|&&s| s > 0)
        .map(|&s| s as usize)
        .sum::<usize>()
        .max(1);

    // Build voice filter: determine which voices to keep per staff.
    // If max_voices_per_staff > 0, for each staff we keep only the N lowest-numbered voices.
    let max_vpstaff = config.max_voices_per_staff;
    let voice_allowed: Box<dyn Fn(i8, i8) -> bool> = if max_vpstaff > 0 {
        // Find the primary voice(s) for each staff
        let mut staff_voices: Vec<std::collections::BTreeSet<i8>> =
            vec![std::collections::BTreeSet::new(); num_staves + 1];
        for record in &notelist.records {
            match record {
                NotelistRecord::Note { voice, staff, .. }
                | NotelistRecord::Rest { voice, staff, .. } => {
                    let s = *staff as usize;
                    if s > 0 && s <= num_staves {
                        staff_voices[s].insert(*voice);
                    }
                }
                _ => {}
            }
        }
        // For each staff, collect the first N voices (sorted)
        let mut allowed: std::collections::HashSet<(i8, i8)> = std::collections::HashSet::new();
        #[allow(clippy::needless_range_loop)] // 1-based staff indexing matches OG
        for s in 1..=num_staves {
            for (i, &v) in staff_voices[s].iter().enumerate() {
                if i < max_vpstaff {
                    allowed.insert((s as i8, v));
                }
            }
        }
        Box::new(move |staff: i8, voice: i8| allowed.contains(&(staff, voice)))
    } else {
        Box::new(|_staff: i8, _voice: i8| true) // no filter
    };

    // ---- VOICE ROLE DETERMINATION ----
    // Port of IIAutoMultiVoice / Multivoice.h voice role system.
    //
    // For each staff, determine which voices are present. If only one voice
    // exists on a staff, it's SINGLE (traditional stem rules). If 2+ voices
    // exist, lowest-numbered = UPPER (stems always up), highest = LOWER
    // (stems always down). Any middle voices get UPPER.
    //
    // The voice role table maps (staff, voice) → VoiceRole.
    let voice_roles: std::collections::HashMap<(i8, i8), VoiceRole> = {
        let mut staff_voices: Vec<BTreeSet<i8>> = vec![BTreeSet::new(); num_staves + 1];
        for record in &notelist.records {
            match record {
                NotelistRecord::Note { voice, staff, .. }
                | NotelistRecord::Rest { voice, staff, .. } => {
                    let s = *staff as usize;
                    if s > 0 && s <= num_staves {
                        // Only count allowed voices
                        if voice_allowed(*staff, *voice) {
                            staff_voices[s].insert(*voice);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut roles = std::collections::HashMap::new();
        #[allow(clippy::needless_range_loop)]
        for s in 1..=num_staves {
            let voices: Vec<i8> = staff_voices[s].iter().copied().collect();
            if voices.len() <= 1 {
                // Single voice: traditional stem rules
                for &v in &voices {
                    roles.insert((s as i8, v), VoiceRole::Single);
                }
            } else {
                // Multi-voice: lowest-numbered = UPPER, highest = LOWER
                // Middle voices (if any) are UPPER
                let last_idx = voices.len() - 1;
                for (i, &v) in voices.iter().enumerate() {
                    let role = if i == last_idx {
                        VoiceRole::Lower
                    } else {
                        VoiceRole::Upper
                    };
                    roles.insert((s as i8, v), role);
                }
            }
        }
        roles
    };

    // Collect clef assignments (staff→clef_type)
    let mut clef_types: Vec<u8> = vec![3; num_staves + 1]; // Default treble (type 3)
    for record in &notelist.records {
        if let NotelistRecord::Clef { staff, clef_type } = record {
            let s = *staff as usize;
            if s > 0 && s <= num_staves {
                clef_types[s] = *clef_type;
            }
        }
    }

    // Collect time signature assignments (staff→(numerator, denominator))
    // Use the FIRST time signature per staff (initial time sig), not the last.
    // Mid-piece time signature changes are not yet handled.
    let mut time_sigs: Vec<(i8, i8)> = vec![(4, 4); num_staves + 1]; // Default 4/4
    let mut time_sig_set: Vec<bool> = vec![false; num_staves + 1];
    for record in &notelist.records {
        if let NotelistRecord::TimeSig {
            staff,
            numerator,
            denominator,
        } = record
        {
            let s = *staff as usize;
            if s > 0 && s <= num_staves && !time_sig_set[s] {
                time_sigs[s] = (*numerator, *denominator);
                time_sig_set[s] = true;
            }
        }
    }

    // ===========================================================================
    // MEASURE-BASED PROPORTIONAL SPACING
    // Port of SpaceTime.cp IdealSpace / FillSpaceMap logic
    // ===========================================================================

    // Nightingale's ideal spacing table (dfltSpaceMap from SpaceTime.cp line 701).
    // Fibonacci/sqrt(2) progression. Index: 0=128th .. 8=breve.
    // Values are multiplied by STD_LINEHT to get STDIST.
    const IDEAL_SPACE_MAP: [f32; 9] = [0.625, 1.0, 1.625, 2.50, 3.75, 5.50, 8.00, 11.5, 16.25];
    const STD_LINEHT_F: f32 = 8.0;

    /// Convert l_dur to ideal space in STDIST (port of IdealSpace).
    fn ideal_space_stdist(l_dur: i8) -> f32 {
        // l_dur: BREVE=1, WHOLE=2, HALF=3, QTR=4, 8TH=5, 16TH=6, 32ND=7
        // Map to spaceMap index (reverse order)
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

    /// Convert STDIST to DDIST for standard 5-line staff.
    /// std2d(s, staffHeight, staffLines) = s * staffHeight / (STD_LINEHT * (staffLines-1))
    /// For staffHeight=384, staffLines=5: 1 STDIST = 384 / 32 = 12 DDIST
    fn stdist_to_ddist(stdist: f32, staff_height: Ddist) -> Ddist {
        (stdist * staff_height as f32 / (STD_LINEHT_F * 4.0)) as Ddist
    }

    // Preamble layout: faithful port of CreateSystem (Score.cp:1785-1814).
    // OG positions each preamble object using dLineSp-based formulas from Ross.
    let d_line_sp = config.staff_height / 4; // STFLINES-1 = 4 for standard 5-line staff
                                             //   Clef xd      = dLineSp                        (Score.cp:1406 MakeClef)
    let clef_xd: Ddist = d_line_sp;
    //   KeySig xd    = Clef.xd + 3.5*dLineSp          (Score.cp:1449 MakeKeySig, Ross p.145)
    //   TimeSig xd   = KeySig.xd + spBefore            (Score.cp:1501 MakeTimeSig)
    //     When no key sig accidentals, spBefore = 0, so TimeSig.xd = KeySig.xd
    let timesig_xd: Ddist = clef_xd + (7 * d_line_sp) / 2; // 3.5 * dLineSp
                                                           //   Measure xd   = TimeSig.xd + 3*dLineSp          (Score.cp:1547 MakeMeasure)
    let preamble_width: Ddist = timesig_xd + 3 * d_line_sp;
    // Continuation systems (no time sig) get a narrower preamble:
    // just clef + small gap. Port of CreateSystem variant for continuation.
    let continuation_preamble: Ddist = clef_xd + (5 * d_line_sp) / 2; // clef + 2.5*dLineSp
    let available_width: Ddist = config.content_width() - preamble_width;
    let continuation_available: Ddist = config.content_width() - continuation_preamble;

    // --- Step 1: Identify measure boundaries from barlines ---

    let mut barline_times: Vec<(i32, u8)> = Vec::new();
    for record in &notelist.records {
        if let NotelistRecord::Barline { time, bar_type, .. } = record {
            barline_times.push((*time, *bar_type));
        }
    }
    barline_times.sort_by_key(|b| b.0);

    // --- Step 2: Collect unique time→events within each measure ---
    // A "measure span" is [start_time .. barline_time).
    // Events at the barline time belong to the NEXT measure.

    struct MeasureSpan {
        start_time: i32,
        end_time: i32, // barline time (exclusive)
        #[allow(dead_code)]
        bar_type: u8, // barline type at end
        event_times: Vec<i32>, // sorted unique event times
    }

    let mut measure_spans: Vec<MeasureSpan> = Vec::new();
    let mut meas_start: i32 = 0;
    for &(bar_time, bar_type) in &barline_times {
        measure_spans.push(MeasureSpan {
            start_time: meas_start,
            end_time: bar_time,
            bar_type,
            event_times: Vec::new(),
        });
        meas_start = bar_time;
    }
    // Trailing events after last barline
    let last_event_time = notelist
        .records
        .iter()
        .filter_map(|r| match r {
            NotelistRecord::Note { time, .. } | NotelistRecord::Rest { time, .. } => Some(*time),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    if last_event_time >= meas_start {
        measure_spans.push(MeasureSpan {
            start_time: meas_start,
            end_time: last_event_time + 1,
            bar_type: 1,
            event_times: Vec::new(),
        });
    }

    // Skip anacrusis (first partial measure before first barline) if configured
    if config.skip_anacrusis && !measure_spans.is_empty() && !barline_times.is_empty() {
        let first_barline_time = barline_times[0].0;
        if measure_spans[0].start_time == 0 && measure_spans[0].end_time == first_barline_time {
            measure_spans.remove(0);
            // Remove the first barline (it's now the start of our first measure)
            barline_times.remove(0);
        }
    }

    // ---- SYSTEM BREAK: group measures into systems ----
    // Port of NewSysNums (Reformat.cp): split measures across systems.
    // max_measures > 0 means N measures per system; 0 = all on one system.
    let measures_per_sys = if config.max_measures > 0 {
        config.max_measures
    } else {
        measure_spans.len() // all on one system
    };

    // Build system_measure_ranges: Vec<(start_idx, end_idx)> into measure_spans
    let mut system_measure_ranges: Vec<(usize, usize)> = Vec::new();
    {
        let total = measure_spans.len();
        let mut start = 0;
        while start < total {
            let end = (start + measures_per_sys).min(total);
            system_measure_ranges.push((start, end));
            start = end;
        }
    }
    let num_systems = system_measure_ranges.len();

    // Populate event_times in each measure (voice-filtered)
    for record in &notelist.records {
        let t = match record {
            NotelistRecord::Note {
                time, staff, voice, ..
            } if voice_allowed(*staff, *voice) => Some(*time),
            NotelistRecord::Rest {
                time, staff, voice, ..
            } if voice_allowed(*staff, *voice) => Some(*time),
            _ => None,
        };
        if let Some(t) = t {
            for span in &mut measure_spans {
                if t >= span.start_time && t < span.end_time {
                    if !span.event_times.contains(&t) {
                        span.event_times.push(t);
                    }
                    break;
                }
            }
        }
    }
    for span in &mut measure_spans {
        span.event_times.sort();
    }

    // --- Step 3: Compute ideal width per measure ---
    // For each event time, find the shortest l_dur (highest rhythmic value =
    // needs the MOST space, because shorter durations like 8ths at the same time
    // slot are part of faster motion). Actually — Nightingale uses SyncMaxDur:
    // the LONGEST play duration at each Sync determines its spacing.
    // For l_dur: *smaller* l_dur = longer note. So we want min(l_dur) per time.

    let mut measure_ideal_stdist: Vec<f32> = Vec::new();
    for span in &measure_spans {
        let mut total: f32 = 0.0;
        for &et in &span.event_times {
            let mut min_ldur: i8 = 6; // default 16th
            for record in &notelist.records {
                match record {
                    NotelistRecord::Note {
                        time,
                        dur,
                        staff,
                        voice,
                        ..
                    } if *time == et && voice_allowed(*staff, *voice) => {
                        if *dur < min_ldur {
                            min_ldur = *dur;
                        }
                    }
                    NotelistRecord::Rest {
                        time,
                        dur,
                        staff,
                        voice,
                        ..
                    } if *time == et && voice_allowed(*staff, *voice) => {
                        if *dur < min_ldur {
                            min_ldur = *dur;
                        }
                    }
                    _ => {}
                }
            }
            total += ideal_space_stdist(min_ldur);
        }
        // Ensure non-empty measures get at least a quarter-note's space
        if total < ideal_space_stdist(4) && !span.event_times.is_empty() {
            total = ideal_space_stdist(4);
        }
        measure_ideal_stdist.push(total);
    }

    // --- Step 4: Scale measures to fit available width (PER SYSTEM) ---
    // Each system has its own preamble. Only system 0 gets timesig; subsequent
    // systems still get clef at the preamble. For simplicity, use same preamble
    // width for all systems (timesig is narrow enough not to matter much).

    let mut measure_abs_xd: Vec<Ddist> = Vec::new();
    let mut measure_width_ddist: Vec<Ddist> = Vec::new();

    for (sys_idx_sp, &(sys_start, sys_end)) in system_measure_ranges.iter().enumerate() {
        // Use narrower preamble for continuation systems (no time sig)
        let sys_preamble = if sys_idx_sp == 0 {
            preamble_width
        } else {
            continuation_preamble
        };
        let sys_available = if sys_idx_sp == 0 {
            available_width
        } else {
            continuation_available
        };

        // Ideal space for measures in this system
        let sys_ideal: f32 = measure_ideal_stdist[sys_start..sys_end].iter().sum();
        let sys_ideal_ddist = stdist_to_ddist(sys_ideal, config.staff_height);

        // Scale factor for this system's measures
        let sys_scale = if sys_ideal_ddist > 0 {
            sys_available as f32 / sys_ideal_ddist as f32
        } else {
            1.0
        };

        // Absolute X positions within this system (relative to system_left)
        let mut x_cursor: Ddist = sys_preamble;
        #[allow(clippy::needless_range_loop)] // need index into measure_ideal_stdist
        for mi in sys_start..sys_end {
            measure_abs_xd.push(x_cursor);
            let w = (stdist_to_ddist(measure_ideal_stdist[mi], config.staff_height) as f32
                * sys_scale) as Ddist;
            let w = w.max(200); // minimum 200 DDIST per measure
            measure_width_ddist.push(w);
            x_cursor += w;
        }
    }

    // --- Step 5: Compute per-event xd (relative to measure) ---
    // Within each measure, distribute events proportionally using their
    // accumulated ideal spacing, scaled to the measure's actual width.

    let mut event_rel_xd: std::collections::HashMap<i32, Ddist> = std::collections::HashMap::new();

    for (mi, span) in measure_spans.iter().enumerate() {
        let ideal_total_std = measure_ideal_stdist[mi];
        let actual_width = measure_width_ddist[mi];

        let inner_scale = if ideal_total_std > 0.0 {
            actual_width as f32 / stdist_to_ddist(ideal_total_std, config.staff_height) as f32
        } else {
            1.0
        };

        // Leave a small indent from the barline before the first event
        let indent: Ddist = 100; // ~6pt indent from barline
        let mut acc_stdist: f32 = 0.0;

        for &et in &span.event_times {
            let raw_ddist = stdist_to_ddist(acc_stdist, config.staff_height);
            let rel = indent + (raw_ddist as f32 * inner_scale) as Ddist;
            event_rel_xd.insert(et, rel);

            // Advance by ideal space for this event's dominant duration
            let mut min_ldur: i8 = 6;
            for record in &notelist.records {
                match record {
                    NotelistRecord::Note {
                        time,
                        dur,
                        staff,
                        voice,
                        ..
                    } if *time == et && voice_allowed(*staff, *voice) => {
                        if *dur < min_ldur {
                            min_ldur = *dur;
                        }
                    }
                    NotelistRecord::Rest {
                        time,
                        dur,
                        staff,
                        voice,
                        ..
                    } if *time == et && voice_allowed(*staff, *voice) => {
                        if *dur < min_ldur {
                            min_ldur = *dur;
                        }
                    }
                    _ => {}
                }
            }
            acc_stdist += ideal_space_stdist(min_ldur);
        }
    }

    // ===========================================================================
    // Build the object list — multi-system layout
    // ===========================================================================
    //
    // Structure: HEADER → PAGE → [SYSTEM → STAFF → CONNECT → CLEF → (TIMESIG) →
    //            initial-MEASURE → barline-MEASUREs + SYNCs] × N → TAIL
    //
    // Port of CreateSystem (Score.cp:1785) applied per system.

    let mut link_counter: Link = 0;
    let mut next_link = || -> Link {
        link_counter += 1;
        link_counter
    };

    // ---- HEADER and PAGE (shared across systems) ----
    let header_link = next_link();
    let page_link = next_link();

    // Pre-allocate system links so we can wire left/right pointers
    let mut system_links: Vec<Link> = Vec::new();
    for _ in 0..num_systems {
        system_links.push(next_link());
    }
    // Reserve tail link
    let tail_link = next_link();

    // Coordinate system: context computes note X as:
    //   note_x = measure_left + sync.xd + note.xd
    // where measure_left = staff_left + measure_obj.xd
    //
    // Measure xd is absolute (relative to staff_left).
    // Sync xd is relative to its containing measure.

    struct MusicObj {
        link: Link,
        kind: MusicObjKind,
    }
    enum MusicObjKind {
        Measure {
            bar_type: u8,
            xd: Ddist,
            visible: bool,
        },
        Sync {
            time: i32,
            xd: Ddist,
        },
    }

    // HEADER object
    score.objects.push(InterpretedObject {
        index: header_link,
        header: ObjectHeader {
            right: page_link,
            left: NILINK,
            first_sub_obj: NILINK,
            xd: 0,
            yd: 0,
            obj_type: 0, // HEADERtype
            selected: false,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ohdr_filler1: 0,
            obj_rect: Rect::default(),
            rel_size: 0,
            ohdr_filler2: 0,
            n_entries: 0,
        },
        data: ObjData::Header(Header {
            header: ObjectHeader::default(),
        }),
    });

    // PAGE object
    score.objects.push(InterpretedObject {
        index: page_link,
        header: ObjectHeader {
            right: system_links[0],
            left: header_link,
            first_sub_obj: NILINK,
            xd: 0,
            yd: 0,
            obj_type: 4, // PAGEtype
            selected: false,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ohdr_filler1: 0,
            obj_rect: Rect::default(),
            rel_size: 0,
            ohdr_filler2: 0,
            n_entries: 0,
        },
        data: ObjData::Page(Page {
            header: ObjectHeader::default(),
            l_page: NILINK,
            r_page: NILINK,
            sheet_num: 0,
            header_str_offset: 0,
            footer_str_offset: 0,
        }),
    });

    // Subobject link namespace base — incremented as we create subobjects
    let mut sub_link_counter: Link = 1;
    let mut next_sub_link = || -> Link {
        let l = sub_link_counter;
        sub_link_counter += 1;
        l
    };

    let mut note_sub_counter: Link = 200; // Subobject link namespace for notes

    // Track the last music object link per system for TAIL wiring
    let mut last_obj_link_per_system: Vec<Link> = Vec::new();

    // Collect all music_objs across all systems for beam processing later
    let mut all_music_objs: Vec<MusicObj> = Vec::new();

    // ---- BUILD EACH SYSTEM ----
    // Port of CreateSystem loop from Score.cp
    let system_height = (num_staves as Ddist - 1) * config.inter_staff + config.staff_height;

    for (sys_idx, &(sys_meas_start, sys_meas_end)) in system_measure_ranges.iter().enumerate() {
        let system_link = system_links[sys_idx];

        // System vertical position — stacked with inter_system spacing
        // Port of PageFixSysRects (SFormat.cp)
        let sys_top = config.system_top + sys_idx as Ddist * config.inter_system;

        let system_rect = DRect {
            top: sys_top,
            left: config.system_left,
            bottom: sys_top + system_height,
            right: config.system_right,
        };

        // Allocate STAFF, CONNECT, CLEF links for this system
        let staff_link = next_link();
        let connect_link = next_link();
        let clef_link = next_link();

        // Only first system gets a time signature in the preamble
        let has_timesig = sys_idx == 0;
        let timesig_link = if has_timesig { next_link() } else { NILINK };

        // --- SYSTEM object ---
        // left = previous system or page; right = staff
        let sys_left = if sys_idx == 0 {
            page_link
        } else {
            // last object of previous system
            *last_obj_link_per_system.last().unwrap_or(&page_link)
        };
        // right will point to staff_link of this system
        score.objects.push(InterpretedObject {
            index: system_link,
            header: ObjectHeader {
                right: staff_link,
                left: sys_left,
                first_sub_obj: NILINK,
                xd: 0,
                yd: 0,
                obj_type: 5, // SYSTEMtype
                selected: false,
                visible: true,
                soft: false,
                valid: true,
                tweaked: false,
                spare_flag: false,
                ohdr_filler1: 0,
                obj_rect: Rect::default(),
                rel_size: 0,
                ohdr_filler2: 0,
                n_entries: 0,
            },
            data: ObjData::System(System {
                header: ObjectHeader::default(),
                l_system: if sys_idx > 0 {
                    system_links[sys_idx - 1]
                } else {
                    NILINK
                },
                r_system: if sys_idx + 1 < num_systems {
                    system_links[sys_idx + 1]
                } else {
                    NILINK
                },
                page_l: page_link,
                system_num: (sys_idx + 1) as i16,
                system_rect,
                sys_desc_ptr: 0,
            }),
        });

        // Fix the right-link of the previous system's last object to point here
        if sys_idx > 0 {
            if let Some(prev_last_link) = last_obj_link_per_system.last() {
                if let Some(prev_obj) = score
                    .objects
                    .iter_mut()
                    .find(|o| o.index == *prev_last_link)
                {
                    prev_obj.header.right = system_link;
                }
            }
        }

        // --- STAFF object — one subobject per staff ---
        let staff_sub_link = next_sub_link();
        let mut staff_subs: Vec<AStaff> = Vec::new();

        #[allow(clippy::needless_range_loop)]
        for s in 1..=num_staves {
            let staff_top = (s as Ddist - 1) * config.inter_staff;
            staff_subs.push(AStaff {
                next: if s < num_staves {
                    staff_sub_link + s as Link
                } else {
                    NILINK
                },
                staffn: s as i8,
                selected: false,
                visible: true,
                filler_stf: false,
                staff_top,
                staff_left: 0,
                staff_right: config.content_width(),
                staff_height: config.staff_height,
                staff_lines: 5,
                font_size: 24,
                flag_leading: 0,
                min_stem_free: 0,
                ledger_width: 96,
                note_head_width: 96,
                frac_beam_width: 48,
                space_below: config.inter_staff - config.staff_height,
                clef_type: clef_types[s] as i8,
                dynamic_type: 0,
                ks_info: KsInfo::default(),
                time_sig_type: 0,
                numerator: 4,
                denominator: 4,
                filler: 0,
                show_ledgers: 1,
                show_lines: SHOW_ALL_LINES,
            });
        }
        score.staffs.insert(staff_sub_link, staff_subs);

        score.objects.push(InterpretedObject {
            index: staff_link,
            header: ObjectHeader {
                right: connect_link,
                left: system_link,
                first_sub_obj: staff_sub_link,
                xd: 0,
                yd: 0,
                obj_type: 6, // STAFFtype
                selected: false,
                visible: true,
                soft: false,
                valid: true,
                tweaked: false,
                spare_flag: false,
                ohdr_filler1: 0,
                obj_rect: Rect::default(),
                rel_size: 0,
                ohdr_filler2: 0,
                n_entries: num_staves as u8,
            },
            data: ObjData::Staff(Staff {
                header: ObjectHeader::default(),
                l_staff: NILINK,
                r_staff: NILINK,
                system_l: system_link,
            }),
        });

        // --- CONNECT object (brace for grand staff) ---
        let connect_sub_link = next_sub_link();
        if num_staves > 1 {
            let connect_subs = vec![AConnect {
                next: NILINK,
                selected: false,
                filler: 0,
                conn_level: 1,
                connect_type: 3,
                staff_above: 1,
                staff_below: num_staves as i8,
                xd: -48,
                first_part: NILINK,
                last_part: NILINK,
            }];
            score.connects.insert(connect_sub_link, connect_subs);
        }

        score.objects.push(InterpretedObject {
            index: connect_link,
            header: ObjectHeader {
                right: clef_link,
                left: staff_link,
                first_sub_obj: if num_staves > 1 {
                    connect_sub_link
                } else {
                    NILINK
                },
                xd: 0,
                yd: 0,
                obj_type: 12, // CONNECTtype
                selected: false,
                visible: true,
                soft: false,
                valid: true,
                tweaked: false,
                spare_flag: false,
                ohdr_filler1: 0,
                obj_rect: Rect::default(),
                rel_size: 0,
                ohdr_filler2: 0,
                n_entries: if num_staves > 1 { 1 } else { 0 },
            },
            data: ObjData::Connect(Connect {
                header: ObjectHeader::default(),
                conn_filler: NILINK,
            }),
        });

        // --- CLEF object ---
        let clef_sub_link = next_sub_link();
        let mut clef_subs: Vec<AClef> = Vec::new();

        #[allow(clippy::needless_range_loop)]
        for s in 1..=num_staves {
            clef_subs.push(AClef {
                header: SubObjHeader {
                    next: NILINK,
                    staffn: s as i8,
                    sub_type: clef_types[s] as i8,
                    selected: false,
                    visible: true,
                    soft: false,
                },
                filler1: 0,
                small: 0,
                filler2: 0,
                xd: 0,
                yd: 0,
            });
        }
        score.clefs.insert(clef_sub_link, clef_subs);

        let clef_right = if has_timesig {
            timesig_link
        } else {
            // clef links directly to first music object (will be updated below)
            NILINK // placeholder — fixed after music_objs are built
        };

        score.objects.push(InterpretedObject {
            index: clef_link,
            header: ObjectHeader {
                right: clef_right,
                left: connect_link,
                first_sub_obj: clef_sub_link,
                xd: clef_xd,
                yd: 0,
                obj_type: CLEF_TYPE as i8,
                selected: false,
                visible: true,
                soft: false,
                valid: true,
                tweaked: false,
                spare_flag: false,
                ohdr_filler1: 0,
                obj_rect: Rect::default(),
                rel_size: 0,
                ohdr_filler2: 0,
                n_entries: num_staves as u8,
            },
            data: ObjData::Clef(Clef {
                header: ObjectHeader::default(),
                in_measure: false,
            }),
        });

        // --- TIMESIG object (first system only) ---
        if has_timesig {
            let timesig_sub_link = next_sub_link();
            let mut timesig_subs: Vec<ATimeSig> = Vec::new();

            #[allow(clippy::needless_range_loop)]
            for s in 1..=num_staves {
                let (numerator, denominator) = time_sigs[s];
                timesig_subs.push(ATimeSig {
                    header: SubObjHeader {
                        next: NILINK,
                        staffn: s as i8,
                        sub_type: 1,
                        selected: false,
                        visible: true,
                        soft: false,
                    },
                    filler: 0,
                    small: 0,
                    conn_staff: 0,
                    xd: 0,
                    yd: 0,
                    numerator,
                    denominator,
                });
            }
            score.timesigs.insert(timesig_sub_link, timesig_subs);

            // timesig right pointer → first music obj (placeholder, fixed below)
            score.objects.push(InterpretedObject {
                index: timesig_link,
                header: ObjectHeader {
                    right: NILINK, // placeholder
                    left: clef_link,
                    first_sub_obj: timesig_sub_link,
                    xd: timesig_xd,
                    yd: 0,
                    obj_type: TIMESIG_TYPE as i8,
                    selected: false,
                    visible: true,
                    soft: false,
                    valid: true,
                    tweaked: false,
                    spare_flag: false,
                    ohdr_filler1: 0,
                    obj_rect: Rect::default(),
                    rel_size: 0,
                    ohdr_filler2: 0,
                    n_entries: num_staves as u8,
                },
                data: ObjData::TimeSig(TimeSig {
                    header: ObjectHeader::default(),
                    in_measure: false,
                }),
            });
        }

        // --- MUSIC OBJECTS for this system (Measures and Syncs) ---
        let mut music_objs: Vec<MusicObj> = Vec::new();

        // Initial (invisible) measure for this system
        let sys_preamble = if sys_idx == 0 {
            preamble_width
        } else {
            continuation_preamble
        };
        let initial_meas_xd = if sys_meas_start < measure_abs_xd.len() {
            measure_abs_xd[sys_meas_start]
        } else {
            sys_preamble
        };
        let initial_meas_link = next_link();
        music_objs.push(MusicObj {
            link: initial_meas_link,
            kind: MusicObjKind::Measure {
                bar_type: 1,
                xd: initial_meas_xd,
                visible: false,
            },
        });

        // Cutoff time for this system: end of last measure in this system
        let sys_cutoff_time = measure_spans[sys_meas_end - 1].end_time;
        // Start time for this system
        let sys_start_time = measure_spans[sys_meas_start].start_time;

        // Walk records, emitting Syncs and Measures within this system's range
        let mut seen_times: Vec<i32> = Vec::new();

        // Figure out which barlines belong to this system
        // Barlines within [sys_start_time .. sys_cutoff_time]
        let sys_barlines: Vec<(i32, u8)> = barline_times
            .iter()
            .filter(|&&(t, _)| t > sys_start_time && t <= sys_cutoff_time)
            .copied()
            .collect();
        let mut sys_barline_idx: usize = 0;

        for record in &notelist.records {
            match record {
                NotelistRecord::Barline { time, bar_type, .. } => {
                    if sys_barline_idx >= sys_barlines.len() {
                        continue;
                    }
                    if *time != sys_barlines[sys_barline_idx].0 {
                        continue;
                    }

                    // Find which measure index this barline corresponds to
                    // The barline at time T starts the next measure after the one ending at T
                    let global_meas_idx = measure_spans
                        .iter()
                        .position(|s| s.end_time == *time)
                        .map(|i| i + 1);

                    // Place barline at the RIGHT edge of the current system's
                    // last measure — not at the start of the next system's first
                    // measure (which would be in a different coordinate space).
                    let bar_xd = if let Some(gmi) = global_meas_idx {
                        if gmi >= sys_meas_end {
                            // System-boundary barline: next measure is on the next system.
                            // Place at right edge of this system's last measure.
                            let last = sys_meas_end - 1;
                            measure_abs_xd[last] + measure_width_ddist[last]
                        } else if gmi < measure_abs_xd.len() {
                            measure_abs_xd[gmi]
                        } else {
                            let last = measure_abs_xd.len() - 1;
                            measure_abs_xd[last] + measure_width_ddist[last]
                        }
                    } else {
                        let last = sys_meas_end - 1;
                        measure_abs_xd[last] + measure_width_ddist[last]
                    };
                    sys_barline_idx += 1;

                    let link = next_link();
                    music_objs.push(MusicObj {
                        link,
                        kind: MusicObjKind::Measure {
                            bar_type: *bar_type,
                            xd: bar_xd,
                            visible: true,
                        },
                    });
                }
                NotelistRecord::Note { time, .. } | NotelistRecord::Rest { time, .. } => {
                    // Only include events within this system's time range
                    if *time < sys_start_time || *time >= sys_cutoff_time {
                        continue;
                    }

                    if !seen_times.contains(time) {
                        seen_times.push(*time);
                        let rel_xd = event_rel_xd.get(time).copied().unwrap_or(100);
                        let link = next_link();
                        music_objs.push(MusicObj {
                            link,
                            kind: MusicObjKind::Sync {
                                time: *time,
                                xd: rel_xd,
                            },
                        });
                    }
                }
                _ => {}
            }
        }

        // Wire the preamble's last object to first music object
        let preamble_last_link = if has_timesig { timesig_link } else { clef_link };
        let first_music_link = if music_objs.is_empty() {
            tail_link
        } else {
            music_objs[0].link
        };

        // Fix preamble last → first music
        if let Some(obj) = score
            .objects
            .iter_mut()
            .find(|o| o.index == preamble_last_link)
        {
            obj.header.right = first_music_link;
        }

        // Track the last object link for this system (for TAIL/next-system wiring)
        let last_music_link = music_objs
            .last()
            .map(|m| m.link)
            .unwrap_or(preamble_last_link);
        last_obj_link_per_system.push(last_music_link);

        // Add music_objs to all_music_objs (for beam processing) and append to score
        all_music_objs.extend(music_objs.iter().map(|m| MusicObj {
            link: m.link,
            kind: match &m.kind {
                MusicObjKind::Measure {
                    bar_type,
                    xd,
                    visible,
                } => MusicObjKind::Measure {
                    bar_type: *bar_type,
                    xd: *xd,
                    visible: *visible,
                },
                MusicObjKind::Sync { time, xd } => MusicObjKind::Sync {
                    time: *time,
                    xd: *xd,
                },
            },
        }));

        // Now emit the music objects into score.objects
        // (This replaces the old single-system music object emission)
        // Note: we reference music_objs here, not all_music_objs
        let music_objs_ref = &all_music_objs[all_music_objs.len() - music_objs.len()..];

        // Track system_link and staff_link for Measure.system_l / staff_l references
        let _sys_staff_link = staff_link;

        // ---- MUSIC OBJECTS (Measures and Syncs) for this system ----

        for (i, mobj) in music_objs_ref.iter().enumerate() {
            // Right link: next music obj, or next system, or tail
            let right = if i + 1 < music_objs_ref.len() {
                music_objs_ref[i + 1].link
            } else if sys_idx + 1 < num_systems {
                system_links[sys_idx + 1]
            } else {
                tail_link
            };
            let left = if i > 0 {
                music_objs_ref[i - 1].link
            } else {
                preamble_last_link
            };

            match &mobj.kind {
                MusicObjKind::Measure {
                    bar_type,
                    xd: measure_xd,
                    visible: meas_visible,
                } => {
                    // Create AMeasure subobjects (one per staff)
                    let measure_sub_link = note_sub_counter;
                    note_sub_counter += 1;

                    let mut measure_subs: Vec<AMeasure> = Vec::new();
                    #[allow(clippy::needless_range_loop)]
                    for s in 1..=num_staves {
                        measure_subs.push(AMeasure {
                            header: SubObjHeader {
                                next: NILINK,
                                staffn: s as i8,
                                sub_type: *bar_type as i8,
                                selected: false,
                                visible: *meas_visible,
                                soft: false,
                            },
                            measure_visible: true,
                            conn_above: s > 1,
                            filler1: 0,
                            filler2: 0,
                            reserved_m: 0,
                            measure_num: 0,
                            meas_size_rect: DRect::default(),
                            conn_staff: 0,
                            clef_type: clef_types[s] as i8,
                            dynamic_type: 0,
                            ks_info: KsInfo::default(),
                            time_sig_type: 0,
                            numerator: 4,
                            denominator: 4,
                            x_mn_std_offset: 0,
                            y_mn_std_offset: 0,
                        });
                    }
                    score.measures.insert(measure_sub_link, measure_subs);

                    score.objects.push(InterpretedObject {
                        index: mobj.link,
                        header: ObjectHeader {
                            right,
                            left,
                            first_sub_obj: measure_sub_link,
                            xd: *measure_xd,
                            yd: 0,
                            obj_type: 7, // MEASUREtype
                            selected: false,
                            visible: true,
                            soft: false,
                            valid: true,
                            tweaked: false,
                            spare_flag: false,
                            ohdr_filler1: 0,
                            obj_rect: Rect::default(),
                            rel_size: 0,
                            ohdr_filler2: 0,
                            n_entries: num_staves as u8,
                        },
                        data: ObjData::Measure(Measure {
                            header: ObjectHeader::default(),
                            filler_m: 0,
                            l_measure: NILINK,
                            r_measure: NILINK,
                            system_l: system_link,
                            staff_l: staff_link,
                            fake_meas: 0,
                            space_percent: 100,
                            measure_b_box: Rect::default(),
                            l_time_stamp: 0,
                        }),
                    });
                }

                MusicObjKind::Sync { time, xd: sync_xd } => {
                    // Collect all notes/rests at this time
                    let note_sub_link = note_sub_counter;
                    note_sub_counter += 1;

                    let mut notes: Vec<ANote> = Vec::new();

                    for record in &notelist.records {
                        match record {
                            NotelistRecord::Note {
                                time: t,
                                voice,
                                staff,
                                dur,
                                dots,
                                note_num,
                                acc,
                                effective_acc,
                                play_dur,
                                velocity,
                                stem_info,
                                appear,
                                ..
                            } if *t == *time => {
                                let s = *staff as usize;
                                if s == 0 || s > num_staves {
                                    continue;
                                }
                                // Voice filter: skip notes not in allowed voices
                                if !voice_allowed(*staff, *voice) {
                                    continue;
                                }

                                let clef = clef_types[s];
                                let mid_c_hl = clef_middle_c_half_ln(clef);

                                // Compute Y position using NLMIDI2HalfLn
                                let half_ln =
                                    nl_midi_to_half_ln(*note_num, *effective_acc, mid_c_hl)
                                        .unwrap_or(4); // Default to middle of staff

                                let yd = half_ln_to_yd(half_ln, config.staff_height);

                                // Stem and accidental calculations — faithful port from OG Nightingale
                                let n_staff_lines: i16 = 5; // Standard 5-line staff

                                // Determine voice role for stem direction
                                let role = voice_roles
                                    .get(&(*staff, *voice))
                                    .copied()
                                    .unwrap_or(VoiceRole::Single);

                                // Compute stem direction — port of NormalStemUpDown (Objects.cp:1457-1497)
                                // VCROLE_SINGLE: position-based (halfLn <= staffLines-1)
                                // VCROLE_UPPER: always stem up (return 1)
                                // VCROLE_LOWER: always stem down (return -1)
                                let stem_down = match stem_info.chars().next() {
                                    Some('+') => false, // Explicit: stem up
                                    Some('-') => true,  // Explicit: stem down
                                    _ => match role {
                                        VoiceRole::Upper => false, // Always stem up
                                        VoiceRole::Lower => true,  // Always stem down
                                        VoiceRole::Single => {
                                            // Position-based (SINGLE_DI case from GetCStemInfo)
                                            // Reference: Utility.cp:174
                                            half_ln < n_staff_lines
                                        }
                                    },
                                };

                                // Compute stem endpoint — port of CalcYStem (Utility.cp:49-89)
                                let num_flags = nflags(*dur);
                                // Use shorter stems for multi-voice notation
                                // Port of QSTEMLEN macro (defs.h:417)
                                let qtr_sp = match role {
                                    VoiceRole::Single => config.stem_len_normal as i16,
                                    _ => config.stem_len_2v as i16,
                                };

                                let ystem = if *dur >= 3 {
                                    // Has stem (half note and shorter)
                                    calc_ystem(
                                        yd,
                                        num_flags,
                                        stem_down,
                                        config.staff_height,
                                        n_staff_lines,
                                        qtr_sp,
                                        false, // allow midline extension
                                    )
                                } else {
                                    yd // Whole notes/breves: no stem
                                };

                                // Note: accidental X offset and stem X offset are computed at
                                // render time by score_renderer.rs, matching OG Nightingale.
                                // TODO: port AccXOffset from DrawNRGR.cp:396-406 properly
                                // TODO: port stem X from DrawNRGR.cp:1094-1097 properly

                                notes.push(ANote {
                                    header: SubObjHeader {
                                        next: NILINK,
                                        staffn: *staff,
                                        sub_type: *dur, // l_dur
                                        selected: false,
                                        visible: true,
                                        soft: false,
                                    },
                                    in_chord: false,
                                    rest: false,
                                    unpitched: false,
                                    beamed: false,
                                    other_stem_side: false,
                                    yqpit: 0,
                                    xd: 0, // Relative to sync
                                    yd,
                                    ystem,
                                    play_time_delta: 0,
                                    play_dur: *play_dur,
                                    p_time: 0,
                                    note_num: *note_num,
                                    on_velocity: *velocity,
                                    off_velocity: 64,
                                    // Tie flags from stem_info (NotelistSave.cp:130-136):
                                    // pos 1 = ')' => tiedL, pos 2 = '(' => tiedR
                                    tied_l: stem_info.as_bytes().get(1) == Some(&b')'),
                                    tied_r: stem_info.as_bytes().get(2) == Some(&b'('),
                                    // OG: xMoveDots = 3 + WIDEHEAD(subType) (Objects.cp:857)
                                    // WIDEHEAD: breve=2, whole=1, else=0. 3 = "default" in
                                    // AugDotXDOffset formula: std2d(STD_LINEHT*(xMoveDots-3)/4)
                                    x_move_dots: {
                                        let wide: u8 = if *dur <= 2 {
                                            if *dur == 1 {
                                                2
                                            } else {
                                                1
                                            }
                                        } else {
                                            0
                                        };
                                        3 + wide
                                    },
                                    // OG: yMoveDots via GetLineAugDotPos (Utility.cp:262)
                                    // Note on line: single/upper voice → 1 (above), lower → 3 (below)
                                    // Note in space: 2 (same level). 0 = invisible.
                                    // (Objects.cp:858-861)
                                    y_move_dots: if *dots > 0 {
                                        let half_ln_unit = config.staff_height / 8;
                                        let half_ln = if half_ln_unit > 0 {
                                            yd / half_ln_unit
                                        } else {
                                            0
                                        };
                                        if half_ln % 2 == 0 {
                                            // On a line — GetLineAugDotPos
                                            if role == VoiceRole::Lower && stem_down {
                                                3
                                            } else {
                                                1
                                            }
                                        } else {
                                            2 // in a space
                                        }
                                    } else {
                                        0
                                    },
                                    ndots: *dots,
                                    voice: *voice,
                                    rsp_ignore: 0,
                                    accident: *acc,
                                    acc_soft: false,
                                    courtesy_acc: 0,
                                    xmove_acc: 0,
                                    play_as_cue: false,
                                    micropitch: 0,
                                    merged: 0,
                                    double_dur: 0,
                                    head_shape: *appear,
                                    first_mod: NILINK,
                                    slurred_l: false,
                                    slurred_r: false,
                                    in_tuplet: false,
                                    in_ottava: false,
                                    small: false,
                                    temp_flag: 0,
                                    art_harmonic: 0,
                                    user_id: 0,
                                    nh_segment: [0; 6],
                                    reserved_n: 0,
                                });
                            }
                            NotelistRecord::Rest {
                                time: t,
                                voice,
                                staff,
                                dur,
                                dots,
                                appear,
                                ..
                            } if *t == *time => {
                                let s = *staff as usize;
                                if s == 0 || s > num_staves {
                                    continue;
                                }
                                // Voice filter: skip rests not in allowed voices
                                if !voice_allowed(*staff, *voice) {
                                    continue;
                                }

                                // Rest Y position — port of GetRestMultivoiceRole (Multivoice.cp:258-269)
                                // SINGLE: centered on staff (half-line 4 for 5-line staff)
                                // UPPER: raised above center by rest_mv_offset half-lines
                                // LOWER: lowered below center by rest_mv_offset half-lines
                                let rest_role = voice_roles
                                    .get(&(*staff, *voice))
                                    .copied()
                                    .unwrap_or(VoiceRole::Single);
                                let base_half_ln: i16 = 4; // Center of 5-line staff
                                let rest_half_ln = match rest_role {
                                    VoiceRole::Single => base_half_ln,
                                    VoiceRole::Upper => base_half_ln - config.rest_mv_offset,
                                    VoiceRole::Lower => base_half_ln + config.rest_mv_offset,
                                };
                                let yd = half_ln_to_yd(rest_half_ln, config.staff_height);

                                notes.push(ANote {
                                    header: SubObjHeader {
                                        next: NILINK,
                                        staffn: *staff,
                                        sub_type: *dur,
                                        selected: false,
                                        visible: true,
                                        soft: false,
                                    },
                                    in_chord: false,
                                    rest: true,
                                    unpitched: false,
                                    beamed: false,
                                    other_stem_side: false,
                                    yqpit: 0,
                                    xd: 0,
                                    yd,
                                    ystem: yd,
                                    play_time_delta: 0,
                                    play_dur: 0,
                                    p_time: 0,
                                    note_num: 0,
                                    on_velocity: 0,
                                    off_velocity: 0,
                                    tied_l: false,
                                    tied_r: false,
                                    x_move_dots: 0,
                                    y_move_dots: if *dots > 0 { 2 } else { 0 },
                                    ndots: *dots,
                                    voice: *voice,
                                    rsp_ignore: 0,
                                    accident: 0,
                                    acc_soft: false,
                                    courtesy_acc: 0,
                                    xmove_acc: 0,
                                    play_as_cue: false,
                                    micropitch: 0,
                                    merged: 0,
                                    double_dur: 0,
                                    head_shape: *appear,
                                    first_mod: NILINK,
                                    slurred_l: false,
                                    slurred_r: false,
                                    in_tuplet: false,
                                    in_ottava: false,
                                    small: false,
                                    temp_flag: 0,
                                    art_harmonic: 0,
                                    user_id: 0,
                                    nh_segment: [0; 6],
                                    reserved_n: 0,
                                });
                            }
                            _ => {}
                        }
                    }

                    if notes.is_empty() {
                        continue; // Skip empty syncs
                    }

                    // ---- CHORD PROCESSING ----
                    // Port of FixSyncForChord / NormalStemUpDown / GetNCYStem / FixChordForYStem
                    // from Objects.cp (lines 1594-1744).
                    //
                    // Group notes by (staff, voice). If a group has 2+ notes, it's a chord.
                    // For each chord:
                    //   1. Determine stem direction (NormalStemUpDown: compare extreme notes to midline)
                    //   2. Find the "far note" (furthest from middle in stem direction)
                    //   3. Compute ystem from that note using CalcYStem
                    //   4. Main note gets the computed ystem; others get ystem = yd (hiding stem)
                    //   5. Mark all as in_chord = true

                    // Build a map of (staff, voice) → indices into notes vec
                    let mut chord_groups: std::collections::HashMap<(i8, i8), Vec<usize>> =
                        std::collections::HashMap::new();
                    for (idx, note) in notes.iter().enumerate() {
                        if !note.rest {
                            chord_groups
                                .entry((note.header.staffn, note.voice))
                                .or_default()
                                .push(idx);
                        }
                    }

                    for (&(staffn, voice), indices) in &chord_groups {
                        // Find extreme notes (highest = min yd, lowest = max yd)
                        // Y increases downward in Nightingale coordinates
                        let mut min_yd = i16::MAX;
                        let mut max_yd = i16::MIN;
                        let mut hi_idx = indices[0]; // highest pitch (min yd)
                        let mut lo_idx = indices[0]; // lowest pitch (max yd)

                        for &idx in indices {
                            let yd = notes[idx].yd;
                            if yd < min_yd {
                                min_yd = yd;
                                hi_idx = idx;
                            }
                            if yd > max_yd {
                                max_yd = yd;
                                lo_idx = idx;
                            }
                        }

                        // NormalStemUpDown — voice-role-aware (Objects.cp:1457-1497):
                        // VCROLE_UPPER: always stem up
                        // VCROLE_LOWER: always stem down
                        // VCROLE_SINGLE: compare extreme notes to midline
                        let role = voice_roles
                            .get(&(staffn, voice))
                            .copied()
                            .unwrap_or(VoiceRole::Single);
                        let stem_down = match role {
                            VoiceRole::Upper => false,
                            VoiceRole::Lower => true,
                            VoiceRole::Single => {
                                let mid_line = config.staff_height / 2;
                                (max_yd as i32 - mid_line as i32)
                                    <= (mid_line as i32 - min_yd as i32)
                            }
                        };

                        // Use shorter stems for multi-voice
                        let chord_qtr_sp = match role {
                            VoiceRole::Single => config.stem_len_normal as i16,
                            _ => config.stem_len_2v as i16,
                        };

                        let is_chord = indices.len() >= 2;

                        if is_chord {
                            // GetNCYStem (Objects.cp:1674-1680):
                            // Far note = stem_down ? lowest : highest
                            let far_idx = if stem_down { lo_idx } else { hi_idx };
                            let far_note_yd = notes[far_idx].yd;
                            let far_note_dur = notes[far_idx].header.sub_type;
                            let n_staff_lines: i16 = 5;

                            let chord_ystem = if far_note_dur >= 3 {
                                calc_ystem(
                                    far_note_yd,
                                    nflags(far_note_dur),
                                    stem_down,
                                    config.staff_height,
                                    n_staff_lines,
                                    chord_qtr_sp,
                                    false,
                                )
                            } else {
                                far_note_yd
                            };

                            // FixChordForYStem (Objects.cp:1710-1743):
                            // Far note gets the real ystem; others get ystem = yd (hiding stem)
                            for &idx in indices {
                                notes[idx].in_chord = true;
                                if idx == far_idx {
                                    notes[idx].ystem = chord_ystem;
                                } else {
                                    notes[idx].ystem = notes[idx].yd; // Hide stem
                                }
                            }
                        } else {
                            // Single note: recompute ystem with voice-aware stem length
                            // Port of CalcYStem (Objects.cp:1638-1670)
                            let idx = indices[0];
                            let note_yd = notes[idx].yd;
                            let note_dur = notes[idx].header.sub_type;
                            let n_staff_lines: i16 = 5;

                            if note_dur >= 3 {
                                // quarter note or shorter gets a stem
                                notes[idx].ystem = calc_ystem(
                                    note_yd,
                                    nflags(note_dur),
                                    stem_down,
                                    config.staff_height,
                                    n_staff_lines,
                                    chord_qtr_sp,
                                    false,
                                );
                            }
                            // whole/breve (l_dur <= 2): ystem stays = yd (no stem)
                        }
                    }

                    let n_entries = notes.len() as u8;
                    score.notes.insert(note_sub_link, notes);

                    score.objects.push(InterpretedObject {
                        index: mobj.link,
                        header: ObjectHeader {
                            right,
                            left,
                            first_sub_obj: note_sub_link,
                            xd: *sync_xd,
                            yd: 0,
                            obj_type: 2, // SYNCtype
                            selected: false,
                            visible: true,
                            soft: false,
                            valid: true,
                            tweaked: false,
                            spare_flag: false,
                            ohdr_filler1: 0,
                            obj_rect: Rect::default(),
                            rel_size: 0,
                            ohdr_filler2: 0,
                            n_entries,
                        },
                        data: ObjData::Sync(Sync {
                            header: ObjectHeader::default(),
                            time_stamp: 0,
                        }),
                    });
                }
            }
        }
    } // end for sys_idx (system loop)

    // Use all_music_objs for beam processing (replaces old music_objs)
    let music_objs = all_music_objs;

    // ---- BEAM GROUPING (AutoBeam) ----
    // Create BeamSet objects for consecutive beamable notes (8th notes and shorter).
    // Rules:
    // - Only beam notes with l_dur >= EIGHTH_L_DUR (5)
    // - Group consecutive beamable notes in same voice, staff, and measure
    // - Rests and barlines break beam groups
    // - Minimum 2 notes per beam group

    // For simplicity in this initial implementation, we'll create beam groups by walking
    // through the records again and identifying consecutive beamable notes.

    #[derive(Clone, Debug)]
    struct BeamableNote {
        sync_link: Link,
        time: i32,
        voice: i8,
        staff: i8,
        dur: i8,
    }

    let mut beam_counter: Link = note_sub_counter; // Continue from where notes left off
    let mut beam_groups: Vec<(Link, Vec<BeamableNote>)> = Vec::new(); // (beamset_link, [notes])

    // Build a list of beamable notes in order
    let mut beamable_notes: Vec<BeamableNote> = Vec::new();

    for record in &notelist.records {
        match record {
            NotelistRecord::Note {
                time,
                voice,
                staff,
                dur,
                ..
            } if *dur >= EIGHTH_L_DUR && voice_allowed(*staff, *voice) => {
                // This is a beamable note in an allowed voice
                // Find its sync_link
                if let Some(mobj) = music_objs
                    .iter()
                    .find(|m| matches!(m.kind, MusicObjKind::Sync { time: t, .. } if t == *time))
                {
                    beamable_notes.push(BeamableNote {
                        sync_link: mobj.link,
                        time: *time,
                        voice: *voice,
                        staff: *staff,
                        dur: *dur,
                    });
                }
            }
            _ => {}
        }
    }

    // Sort by (voice, staff, time) so notes for each voice are consecutive
    beamable_notes.sort_by(|a, b| {
        a.voice
            .cmp(&b.voice)
            .then(a.staff.cmp(&b.staff))
            .then(a.time.cmp(&b.time))
    });

    // Deduplicate: only keep one beamable note per (voice, staff, time)
    beamable_notes.dedup_by(|a, b| a.voice == b.voice && a.staff == b.staff && a.time == b.time);

    // Collect barline times as a set for fast lookup, plus system boundary times
    // to ensure beams don't span across system breaks.
    let mut barline_time_set: Vec<i32> = barline_times.iter().map(|&(t, _)| t).collect();
    for &(_sys_start, sys_end) in &system_measure_ranges {
        if sys_end > 0 && sys_end <= measure_spans.len() {
            let boundary_time = measure_spans[sys_end - 1].end_time;
            if !barline_time_set.contains(&boundary_time) {
                barline_time_set.push(boundary_time);
            }
        }
    }
    barline_time_set.sort_unstable();

    // Compute beat duration (in ticks) for beat-boundary beam breaking.
    // Port of AutoBeam.cp CreateNBeamBeatList: beams break at beat boundaries.
    // For simple meters (2/4, 3/4, 4/4), beat = one denominator unit.
    // For compound meters (6/8, 9/8, 12/8), beat = dotted denominator (3 sub-beats).
    let beat_dur_per_staff: Vec<i32> = (0..=num_staves)
        .map(|s| {
            let (num, denom) = time_sigs[s];
            let is_compound = num >= 6 && num % 3 == 0;
            if is_compound {
                // Compound meter: group in dotted beats (3 sub-beats)
                code_to_l_dur(beat_l_dur(denom), 1) as i32 // dotted = 1.5× base
            } else {
                code_to_l_dur(beat_l_dur(denom), 0) as i32
            }
        })
        .collect();

    // Group consecutive beamable notes by voice, staff, within same measure,
    // AND within the same beat (port of AutoBeam beat-boundary breaking).
    let mut current_group: Vec<BeamableNote> = Vec::new();

    for note in &beamable_notes {
        if current_group.is_empty() {
            current_group.push(note.clone());
        } else {
            let last = current_group.last().unwrap();
            // Check if this note continues the current group:
            // - Same voice and staff
            // - No barline/system-break between last note and this note
            // - Same beat (no beat-boundary crossing)
            let same_voice_staff = last.voice == note.voice && last.staff == note.staff;
            let crosses_barline = barline_time_set
                .iter()
                .any(|&bt| bt > last.time && bt <= note.time);

            // Beat-boundary check: find measure start, compute beat index
            let crosses_beat = if same_voice_staff && !crosses_barline {
                let staff_idx = note.staff as usize;
                let beat_dur = if staff_idx < beat_dur_per_staff.len() {
                    beat_dur_per_staff[staff_idx]
                } else {
                    480 // fallback: quarter note
                };
                if beat_dur > 0 {
                    // Find which measure this note is in
                    let meas_start = measure_spans
                        .iter()
                        .filter(|s| note.time >= s.start_time && note.time < s.end_time)
                        .map(|s| s.start_time)
                        .next()
                        .unwrap_or(0);
                    let last_beat = (last.time - meas_start) / beat_dur;
                    let note_beat = (note.time - meas_start) / beat_dur;
                    last_beat != note_beat
                } else {
                    false
                }
            } else {
                false // already breaking for other reasons
            };

            if same_voice_staff && !crosses_barline && !crosses_beat {
                current_group.push(note.clone());
            } else {
                // Save old group and start new
                if current_group.len() >= 2 {
                    let beamset_link = beam_counter;
                    beam_counter += 1;
                    beam_groups.push((beamset_link, current_group.clone()));
                }
                current_group = vec![note.clone()];
            }
        }
    }

    // Save final group if valid
    if current_group.len() >= 2 {
        let beamset_link = beam_counter;
        beam_groups.push((beamset_link, current_group));
    }

    // Create BeamSet objects and mark notes as beamed
    let mut notebeam_sub_counter: Link = 1000; // Separate namespace for notebeam subobjects

    for (beamset_link, group) in &beam_groups {
        let voice = group[0].voice;
        let staffn = group[0].staff;
        let n_entries = group.len() as u8;

        // Calculate number of beams from first note's duration
        let num_beams = (group[0].dur - 4).max(1);

        // Create ANoteBeam subobjects
        let notebeam_sub_link = notebeam_sub_counter;
        notebeam_sub_counter += 1;
        let mut notebeams: Vec<ANoteBeam> = Vec::new();

        for (idx, note) in group.iter().enumerate() {
            let startend = if idx == 0 {
                num_beams // Start N beams
            } else if idx == group.len() - 1 {
                -num_beams // End N beams
            } else {
                0 // Middle note
            };

            notebeams.push(ANoteBeam {
                next: NILINK,
                bp_sync: note.sync_link,
                startend,
                fracs: 0,
                frac_go_left: 0,
                filler: 0,
            });
        }

        score.notebeams.insert(notebeam_sub_link, notebeams);

        // Mark notes as beamed - find them in score.notes
        for note in group {
            // Find the sync object
            if let Some(sync_obj) = score.objects.iter().find(|obj| obj.index == note.sync_link) {
                let note_sub_link = sync_obj.header.first_sub_obj;
                if let Some(notes) = score.notes.get_mut(&note_sub_link) {
                    for n in notes.iter_mut() {
                        if n.voice == voice && n.header.staffn == staffn && !n.rest {
                            n.beamed = true;
                        }
                    }
                }
            }
        }

        // Find the last sync in the group for positioning
        let last_sync_link = group.last().unwrap().sync_link;

        // Insert BeamSet object right after the last sync
        let beamset_obj = InterpretedObject {
            index: *beamset_link,
            header: ObjectHeader {
                right: NILINK, // Will be updated below
                left: NILINK,  // Will be updated below
                first_sub_obj: notebeam_sub_link,
                xd: 0,
                yd: 0,
                obj_type: 11, // BEAMSETtype
                selected: false,
                visible: true,
                soft: false,
                valid: true,
                tweaked: false,
                spare_flag: false,
                ohdr_filler1: 0,
                obj_rect: Rect::default(),
                rel_size: 0,
                ohdr_filler2: 0,
                n_entries,
            },
            data: ObjData::BeamSet(BeamSet {
                header: ObjectHeader::default(),
                ext_header: ExtObjHeader { staffn },
                voice,
                thin: 0,
                beam_rests: 0,
                feather: 0,
                grace: 0,
                first_system: 1,
                cross_staff: 0,
                cross_system: 0,
            }),
        };

        // Find position to insert: right after last_sync_link
        let insert_pos = score
            .objects
            .iter()
            .position(|obj| obj.index == last_sync_link)
            .unwrap()
            + 1;

        // Update links
        let left_link = last_sync_link;
        let right_link = if insert_pos < score.objects.len() {
            score.objects[insert_pos].index
        } else {
            tail_link
        };

        // Update the beamset's links
        let mut beamset_obj = beamset_obj;
        beamset_obj.header.left = left_link;
        beamset_obj.header.right = right_link;

        // Update the object to the left (last sync)
        if let Some(left_obj) = score.objects.iter_mut().find(|obj| obj.index == left_link) {
            left_obj.header.right = *beamset_link;
        }

        // Update the object to the right
        if let Some(right_obj) = score.objects.iter_mut().find(|obj| obj.index == right_link) {
            right_obj.header.left = *beamset_link;
        }

        score.objects.insert(insert_pos, beamset_obj);
    }

    // ---- BEAM GROUP STEM DIRECTION UNIFICATION ----
    // Port of NormalStemUpDown (Objects.cp:1594-1633) applied to beam groups.
    //
    // In OG Nightingale, all notes in a beamset share the same stem direction.
    // For multi-voice: UPPER = always up, LOWER = always down.
    // For SINGLE_DI: compare extreme notes to midline.
    //
    // After determining group direction, recompute ystem for each note.
    {
        let n_staff_lines: i16 = 5;

        for (_beamset_link, group) in &beam_groups {
            let voice = group[0].voice;
            let staffn = group[0].staff;

            // Look up voice role for this beam group
            let role = voice_roles
                .get(&(staffn, voice))
                .copied()
                .unwrap_or(VoiceRole::Single);

            // Use shorter stems for multi-voice
            let qtr_sp = match role {
                VoiceRole::Single => config.stem_len_normal as i16,
                _ => config.stem_len_2v as i16,
            };

            // Determine beam group stem direction based on voice role
            let group_stem_down = match role {
                VoiceRole::Upper => false,
                VoiceRole::Lower => true,
                VoiceRole::Single => {
                    // Collect all yd values for notes in this beam group
                    let mut max_yd: Ddist = i16::MIN;
                    let mut min_yd: Ddist = i16::MAX;

                    for bnote in group {
                        if let Some(sync_obj) =
                            score.objects.iter().find(|o| o.index == bnote.sync_link)
                        {
                            if let Some(notes) = score.notes.get(&sync_obj.header.first_sub_obj) {
                                for n in notes {
                                    if n.voice == voice && n.header.staffn == staffn && !n.rest {
                                        if n.yd > max_yd {
                                            max_yd = n.yd;
                                        }
                                        if n.yd < min_yd {
                                            min_yd = n.yd;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if max_yd == i16::MIN || min_yd == i16::MAX {
                        continue;
                    }

                    let mid_line = config.staff_height / 2;
                    (max_yd as i32 - mid_line as i32) <= (mid_line as i32 - min_yd as i32)
                }
            };

            // Recompute ystem for all notes in the group using the unified direction
            for bnote in group {
                if let Some(sync_obj) = score.objects.iter().find(|o| o.index == bnote.sync_link) {
                    let sub_link = sync_obj.header.first_sub_obj;
                    if let Some(notes) = score.notes.get_mut(&sub_link) {
                        for n in notes.iter_mut() {
                            if n.voice == voice && n.header.staffn == staffn && n.beamed && !n.rest
                            {
                                let num_flags = nflags(bnote.dur);
                                n.ystem = calc_ystem(
                                    n.yd,
                                    num_flags,
                                    group_stem_down,
                                    config.staff_height,
                                    n_staff_lines,
                                    qtr_sp,
                                    false,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- BEAM SLOPE ADJUSTMENT ----
    // Port of GetBeamEndYStems (Beam.cp:181-235) + FixSyncInBeamset (Beam.cp:272-299).
    //
    // For each beamset, compute a reduced slope from the natural CalcYStem endpoints
    // of the first and last notes, then interpolate all intermediate stems along
    // the beam line.
    //
    // Algorithm:
    // 1. Get CalcYStem for first and last notes (their current ystem values)
    // 2. Compute natural slope = lastYstem - firstYstem
    // 3. Apply relBeamSlope percentage to reduce slope
    // 4. Recompute first/last ystem based on the "base" note (the one whose
    //    CalcYStem is the extreme — highest for stems up, lowest for stems down)
    // 5. Linearly interpolate all intermediate stems
    if config.rel_beam_slope > 0 {
        for (_beamset_link, group) in &beam_groups {
            if group.len() < 2 {
                continue;
            }

            let voice = group[0].voice;
            let staffn = group[0].staff;

            // Collect (sync_xd, note_yd, note_ystem, sync_link) for each note in group
            struct BeamNoteInfo {
                sync_xd: Ddist,
                note_yd: Ddist,
                note_ystem: Ddist,
                sync_link: Link,
            }
            let mut infos: Vec<BeamNoteInfo> = Vec::new();

            for bnote in group {
                if let Some(sync_obj) = score.objects.iter().find(|o| o.index == bnote.sync_link) {
                    let sync_xd = sync_obj.header.xd;
                    if let Some(notes) = score.notes.get(&sync_obj.header.first_sub_obj) {
                        if let Some(note) = notes
                            .iter()
                            .find(|n| n.voice == voice && n.header.staffn == staffn && n.beamed)
                        {
                            infos.push(BeamNoteInfo {
                                sync_xd,
                                note_yd: note.yd,
                                note_ystem: note.ystem,
                                sync_link: bnote.sync_link,
                            });
                        }
                    }
                }
            }

            if infos.len() < 2 {
                continue;
            }

            // Determine stem direction from first note
            let stem_down = infos[0].note_ystem > infos[0].note_yd;

            // Natural CalcYStem endpoints (already computed)
            let first_ystem = infos[0].note_ystem;
            let last_ystem = infos[infos.len() - 1].note_ystem;

            // endDiff: vertical difference between endpoints
            // (Beam.cp:208: fEndDiff = (double)(firstystem1 - lastystem1))
            let end_diff = first_ystem - last_ystem;

            // Apply reduced slope (Beam.cp:214: fSlope = fEndDiff * relBeamSlope / 100)
            let slope = end_diff as f32 * config.rel_beam_slope as f32 / 100.0;

            // Find the "base" note — the one whose CalcYStem is the extreme.
            // For stems up: base = note with smallest (highest) ystem
            // For stems down: base = note with largest (lowest) ystem
            // (Beam.cp:191-196)
            let base_idx = if stem_down {
                infos
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, info)| info.note_ystem)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            } else {
                infos
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, info)| info.note_ystem)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            };

            let base_ystem = infos[base_idx].note_ystem;

            // Compute horizontal positions
            let first_xd = infos[0].sync_xd;
            let last_xd = infos[infos.len() - 1].sync_xd;
            let beam_length = (last_xd - first_xd) as f32;

            if beam_length.abs() < 1.0 {
                continue; // Degenerate beam
            }

            // Compute offset from base note to first note
            // (Beam.cp:220-224)
            let base_xd = infos[base_idx].sync_xd;
            let base_to_first = (first_xd - base_xd) as f32;
            let base_frac = base_to_first / beam_length;
            let base_offset = slope * base_frac;

            // First and last ystem based on base note position + slope
            // (Beam.cp:226-227)
            let new_first_ystem = base_ystem as f32 + base_offset;
            let new_last_ystem = new_first_ystem - slope;

            // Interpolate all intermediate stems linearly along the beam line
            for (i, info) in infos.iter().enumerate() {
                let t = if beam_length.abs() > 0.0 {
                    (info.sync_xd - first_xd) as f32 / beam_length
                } else {
                    0.0
                };
                let interpolated_ystem = new_first_ystem + t * (new_last_ystem - new_first_ystem);
                let new_ystem = interpolated_ystem.round() as Ddist;

                // Update the note's ystem in score.notes
                if let Some(sync_obj) = score.objects.iter().find(|o| o.index == infos[i].sync_link)
                {
                    if let Some(notes) = score.notes.get_mut(&sync_obj.header.first_sub_obj) {
                        for note in notes.iter_mut() {
                            if note.voice == voice && note.header.staffn == staffn && note.beamed {
                                note.ystem = new_ystem;
                            }
                        }
                    }
                }
            }
        }
    }

    // TAIL object
    let tail_left = {
        // Find the last object in the list (may be a beamset now)
        score
            .objects
            .iter()
            .rev()
            .find(|obj| obj.header.obj_type != 1) // Skip header-type objects
            .map(|obj| obj.index)
            .unwrap_or(page_link)
    };

    score.objects.push(InterpretedObject {
        index: tail_link,
        header: ObjectHeader {
            right: NILINK,
            left: tail_left,
            first_sub_obj: NILINK,
            xd: 0,
            yd: 0,
            obj_type: 1, // TAILtype
            selected: false,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ohdr_filler1: 0,
            obj_rect: Rect::default(),
            rel_size: 0,
            ohdr_filler2: 0,
            n_entries: 0,
        },
        data: ObjData::Tail(Tail {
            header: ObjectHeader::default(),
        }),
    });

    score
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notelist::parser::parse_notelist;

    #[test]
    fn test_nl_midi_to_half_ln_middle_c_treble() {
        // Middle C (MIDI 60) in treble clef: midCHalfLn = 10
        // pitchClass = 0, eAcc = AC_NATURAL (3)
        // hLinesTable[2][0] = 0 (C natural)
        // octave = (60/12) - 5 = 0
        // halfLines = 0*7 + 0 = 0
        // result = -0 + 10 = 10
        let hl = nl_midi_to_half_ln(60, AC_NATURAL, 10).unwrap();
        assert_eq!(
            hl, 10,
            "Middle C in treble should be at half-line 10 (ledger line below)"
        );
    }

    #[test]
    fn test_nl_midi_to_half_ln_g4_treble() {
        // G4 (MIDI 67) in treble clef
        // pitchClass = 7, eAcc = AC_NATURAL (3)
        // hLinesTable[2][7] = 4
        // octave = (67/12) - 5 = 0
        // halfLines = 0*7 + 4 = 4
        // result = -4 + 10 = 6
        let hl = nl_midi_to_half_ln(67, AC_NATURAL, 10).unwrap();
        assert_eq!(
            hl, 6,
            "G4 in treble should be at half-line 6 (second line from bottom)"
        );
    }

    #[test]
    fn test_nl_midi_to_half_ln_f5_treble() {
        // F5 (MIDI 77) = top line of treble staff
        // pitchClass = 5, eAcc = AC_NATURAL (3)
        // hLinesTable[2][5] = 3
        // octave = (77/12) - 5 = 1
        // halfLines = 1*7 + 3 = 10
        // result = -10 + 10 = 0
        let hl = nl_midi_to_half_ln(77, AC_NATURAL, 10).unwrap();
        assert_eq!(hl, 0, "F5 in treble should be at half-line 0 (top line)");
    }

    #[test]
    fn test_nl_midi_to_half_ln_e4_treble() {
        // E4 (MIDI 64) = first space below treble staff
        // pitchClass = 4, eAcc = AC_NATURAL (3)
        // hLinesTable[2][4] = 2
        // octave = (64/12) - 5 = 0
        // halfLines = 0*7 + 2 = 2
        // result = -2 + 10 = 8
        let hl = nl_midi_to_half_ln(64, AC_NATURAL, 10).unwrap();
        assert_eq!(hl, 8, "E4 in treble should be at half-line 8 (bottom line)");
    }

    #[test]
    fn test_nl_midi_to_half_ln_eb4_treble() {
        // Eb4 (MIDI 63) with eAcc=flat(2) in treble clef
        // pitchClass = 3, eAcc = AC_FLAT (2)
        // hLinesTable[1][3] = 2  (Eb)
        // octave = (63/12) - 5 = 0
        // halfLines = 0*7 + 2 = 2
        // result = -2 + 10 = 8
        let hl_flat = nl_midi_to_half_ln(63, AC_FLAT, 10).unwrap();
        assert_eq!(
            hl_flat, 8,
            "Eb4 in treble should be at E position (half-line 8)"
        );

        // D#4 (MIDI 63) with eAcc=sharp(4) in treble clef
        // pitchClass = 3, eAcc = AC_SHARP (4)
        // hLinesTable[3][3] = 1  (D#)
        // halfLines = 0*7 + 1 = 1
        // result = -1 + 10 = 9
        let hl_sharp = nl_midi_to_half_ln(63, AC_SHARP, 10).unwrap();
        assert_eq!(
            hl_sharp, 9,
            "D#4 in treble should be at D position (half-line 9)"
        );

        // Eb4 should be one half-line above D#4
        assert_eq!(
            hl_flat,
            hl_sharp - 1,
            "Eb4 should be one half-line above D#4"
        );
    }

    #[test]
    fn test_nl_midi_to_half_ln_bass_clef() {
        // Middle C (MIDI 60) in bass clef: midCHalfLn = -2
        // halfLines = 0, result = -0 + (-2) = -2
        let hl = nl_midi_to_half_ln(60, AC_NATURAL, -2).unwrap();
        assert_eq!(
            hl, -2,
            "Middle C in bass should be at half-line -2 (ledger line above)"
        );

        // G2 (MIDI 43) = bottom line of bass staff
        // pitchClass = 7, eAcc = AC_NATURAL
        // hLinesTable[2][7] = 4
        // octave = (43/12) - 5 = -2
        // halfLines = -2*7 + 4 = -10
        // result = 10 + (-2) = 8
        let hl_g2 = nl_midi_to_half_ln(43, AC_NATURAL, -2).unwrap();
        assert_eq!(
            hl_g2, 8,
            "G2 in bass should be at half-line 8 (bottom line)"
        );

        // A3 (MIDI 57) = top line of bass staff
        // pitchClass = 9, eAcc = AC_NATURAL
        // hLinesTable[2][9] = 5
        // octave = (57/12) - 5 = -1
        // halfLines = -1*7 + 5 = -2
        // result = 2 + (-2) = 0
        let hl_a3 = nl_midi_to_half_ln(57, AC_NATURAL, -2).unwrap();
        assert_eq!(hl_a3, 0, "A3 in bass should be at half-line 0 (top line)");
    }

    #[test]
    fn test_clef_middle_c_half_ln_values() {
        assert_eq!(clef_middle_c_half_ln(3), 10); // Treble
        assert_eq!(clef_middle_c_half_ln(10), -2); // Bass
        assert_eq!(clef_middle_c_half_ln(6), 4); // Alto
        assert_eq!(clef_middle_c_half_ln(8), 2); // Tenor
    }

    #[test]
    fn test_half_ln_to_yd() {
        let staff_height: Ddist = 384; // 24pt = 384 DDIST
                                       // Half-line 0 (top line) → yd = 0
        assert_eq!(half_ln_to_yd(0, staff_height), 0);
        // Half-line 8 (bottom line of 5-line staff) → yd = staff_height
        assert_eq!(half_ln_to_yd(8, staff_height), 384);
        // Half-line 4 (middle line) → yd = staff_height/2
        assert_eq!(half_ln_to_yd(4, staff_height), 192);
    }

    #[test]
    fn test_notelist_to_score_hbd33() {
        let file = std::fs::File::open("tests/notelist_examples/HBD_33.nl").unwrap();
        let notelist = parse_notelist(file).unwrap();
        let score = notelist_to_score(&notelist);

        // Should have objects
        assert!(!score.objects.is_empty(), "Score should have objects");

        // Should have staves
        assert!(
            !score.staffs.is_empty(),
            "Score should have staff subobjects"
        );

        // Count object types
        let mut staff_count = 0;
        let mut measure_count = 0;
        let mut sync_count = 0;
        for obj in &score.objects {
            match &obj.data {
                ObjData::Staff(_) => staff_count += 1,
                ObjData::Measure(_) => measure_count += 1,
                ObjData::Sync(_) => sync_count += 1,
                _ => {}
            }
        }

        // With multi-system layout, we get one Staff object per system
        assert!(
            staff_count >= 1,
            "Should have at least 1 Staff object (got {staff_count})"
        );
        assert!(measure_count > 0, "Should have Measure objects (barlines)");
        assert!(sync_count > 0, "Should have Sync objects (notes/rests)");

        // Walk should work
        let walked: Vec<_> = score.walk().collect();
        assert!(walked.len() > 5, "Walk should traverse multiple objects");

        // Notes should have plausible Y positions
        for notes in score.notes.values() {
            for note in notes {
                if !note.rest {
                    // Note Y should be within reasonable range
                    // (-10 to +20 half-lines from top → yd roughly -480 to +960 for 384-height staff)
                    assert!(
                        note.yd > -1000 && note.yd < 2000,
                        "Note yd {} seems unreasonable (note_num={})",
                        note.yd,
                        note.note_num
                    );
                }
            }
        }
    }
}
