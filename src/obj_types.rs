// obj_types.rs - Nightingale object and subobject type definitions
//
// Ported from NObjTypes.h and NObjTypesN105.h
//
// Source: Nightingale/src/Precomps/NObjTypes.h
// Source: Nightingale/src/Precomps/NObjTypesN105.h
//
// These types represent the core data model of Nightingale scores.
// Information spans three domains:
//   L = Logical (musical semantics)
//   G = Graphical (visual appearance)
//   P = Performance/Playback (MIDI/audio)

use crate::basic_types::{
    DPoint, DRect, Ddist, KsInfo, Link, Point, Rect, ShortQd, ShortStd, Stdist, StringOffset,
};

// =============================================================================
// Object and Subobject Size Tables (N105 format)
// =============================================================================

/// N105 object sizes in bytes (indexed by object type 0-24)
/// Source: Ngale5ProgQuickRef-TN1.txt lines 158-193
pub const N105_OBJ_SIZES: [u16; 25] = [
    24, // type  0: HEADER
    24, // type  1: TAIL
    26, // type  2: SYNC
    32, // type  3: RPTEND
    38, // type  4: PAGE
    44, // type  5: SYSTEM
    30, // type  6: STAFF
    46, // type  7: MEASURE
    24, // type  8: CLEF
    24, // type  9: KEYSIG
    24, // type 10: TIMESIG
    26, // type 11: BEAMSET
    26, // type 12: CONNECT
    30, // type 13: DYNAMIC
    0,  // type 14: MODNR
    44, // type 15: GRAPHIC
    40, // type 16: OTTAVA
    30, // type 17: SLUR
    40, // type 18: TUPLET
    24, // type 19: GRSYNC
    38, // type 20: TEMPO
    28, // type 21: SPACER
    32, // type 22: ENDING
    24, // type 23: PSMEAS
    0,  // type 24: OBJ
];

/// N103 object sizes in bytes (indexed by object type 0-24).
/// N103 differs from N105 in HEADER/TAIL/GRSYNC/PSMEAS (+1 pad) and SLUR (30 not 32).
pub const N103_OBJ_SIZES: [u16; 25] = [
    24, // type  0: HEADER
    24, // type  1: TAIL
    26, // type  2: SYNC
    32, // type  3: RPTEND
    38, // type  4: PAGE
    44, // type  5: SYSTEM
    30, // type  6: STAFF
    46, // type  7: MEASURE
    24, // type  8: CLEF
    24, // type  9: KEYSIG
    24, // type 10: TIMESIG
    26, // type 11: BEAMSET
    26, // type 12: CONNECT
    30, // type 13: DYNAMIC
    0,  // type 14: MODNR (subobject only)
    44, // type 15: GRAPHIC
    40, // type 16: OTTAVA
    30, // type 17: SLUR
    40, // type 18: TUPLET  (N105: 44)
    24, // type 19: GRSYNC
    38, // type 20: TEMPO
    28, // type 21: SPACER
    32, // type 22: ENDING
    24, // type 23: PSMEAS
    0,  // type 24: OBJtype (object heap)
];

/// N105 subobject sizes in bytes (indexed by object type 0-24, 0 = no subobjects)
pub const N105_SUBOBJ_SIZES: [u16; 25] = [
    62, 0, 30, 6, 0, 0, 50, 40, 10, 24, 12, 6, 12, 12, 6, 6, 4, 42, 4, 30, 0, 0, 0, 6, 46,
];

// =============================================================================
// Core Header Structures
// =============================================================================

/// Object header (appears at the start of every object)
/// Source: NObjTypes.h lines 26-41 (OBJECTHEADER macro)
///
/// IMPORTANT: The first six fields MUST NOT be reordered - MemMacros.h depends on their positions.
#[derive(Debug, Clone, Default)]
pub struct ObjectHeader {
    pub right: Link,         // Link to right object
    pub left: Link,          // Link to left object
    pub first_sub_obj: Link, // Link to first subobject
    pub xd: Ddist,           // X position of object
    pub yd: Ddist,           // Y position of object
    pub obj_type: i8,        // Object type (at offset +10)
    pub selected: bool,      // True if object or any part is selected
    pub visible: bool,       // True if object or any part is visible
    pub soft: bool,          // True if object is program-generated
    pub valid: bool,         // True if objRect (for Measures, measureBBox too) is valid
    pub tweaked: bool,       // True if object dragged or position edited with Get Info
    pub spare_flag: bool,    // Available for general use
    pub ohdr_filler1: i8,    // Unused; could use for specific "tweak" flags
    pub obj_rect: Rect,      // Enclosing rectangle (paper-relative pixels, at offset +18)
    pub rel_size: i8,        // (unused) Size relative to normal for object & context
    pub ohdr_filler2: i8,    // Unused
    pub n_entries: u8,       // Number of subobjects in object (at offset +28)
}

/// Subobject header (appears at the start of most subobjects)
/// Source: NObjTypes.h lines 43-49 (SUBOBJHEADER macro)
#[derive(Debug, Clone)]
pub struct SubObjHeader {
    pub next: Link,     // Index of next subobject
    pub staffn: i8,     // Staff no. For cross-stf objs, top stf (Slur,Beamset) or 1st stf (Tuplet)
    pub sub_type: i8,   // Subobject subtype. NB: Signed; see ANOTE
    pub selected: bool, // True if subobject is selected
    pub visible: bool,  // True if subobject is visible
    pub soft: bool,     // True if subobject is program-generated
}

/// Extended object header (for objects that span staves)
/// Source: NObjTypes.h lines 51-52 (EXTOBJHEADER macro)
#[derive(Debug, Clone)]
pub struct ExtObjHeader {
    pub staffn: i8, // Staff number: for cross-staff objs, of top staff (FIXME: except tuplets!)
}

// =============================================================================
// Type 0: HEADER (with PARTINFO subobject)
// =============================================================================

/// Header object (Type 0)
/// Source: NObjTypes.h lines 55-60
#[derive(Debug, Clone)]
pub struct Header {
    pub header: ObjectHeader,
}

/// Part information subobject (for HEADER)
/// Source: NBasicTypes.h lines 171-201
#[derive(Debug, Clone)]
pub struct PartInfo {
    pub next: Link,           // Index of next subobj
    pub part_velocity: i8,    // MIDI playback velocity offset
    pub first_staff: i8,      // Index of first staff in the part
    pub patch_num: u8,        // MIDI program no.
    pub last_staff: i8,       // Index of last staff in the part (>= first staff)
    pub channel: u8,          // MIDI channel no.
    pub transpose: i8,        // Transposition, in semitones (0=none)
    pub lo_key_num: i16,      // MIDI note no. of lowest playable note
    pub hi_key_num: i16,      // MIDI note no. of highest playable note
    pub name: [u8; 32],       // Full name, e.g., to label 1st system (C string)
    pub short_name: [u8; 12], // Short name, e.g., for systems after 1st (C string)
    pub hi_key_name: u8,      // Name and accidental of highest playable note
    pub hi_key_acc: u8,
    pub tran_name: u8, // ...of transposition
    pub tran_acc: u8,
    pub lo_key_name: u8, // ...of lowest playable note
    pub lo_key_acc: u8,
    pub bank_number0: u8,  // If device uses cntl 0 for bank select msgs (N103+)
    pub bank_number32: u8, // If device uses cntl 32 for bank select msgs (N103+)
    pub fms_output_device: u16, // FreeMIDI device (obsolete, kept for file compat)
    pub fms_output_destination: [u8; 280], // FreeMIDI destination (obsolete, ~280 bytes)
}

impl PartInfo {
    /// Extract the full part name as a Rust string (from C string in name[32]).
    /// NGL files store strings in Mac Roman encoding; we convert to UTF-8.
    pub fn name_str(pi: &PartInfo) -> String {
        let pos = pi
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(pi.name.len());
        crate::ngl::reader::mac_roman_to_string(&pi.name[..pos])
    }

    /// Extract the short part name as a Rust string (from C string in short_name[12]).
    /// NGL files store strings in Mac Roman encoding; we convert to UTF-8.
    pub fn short_name_str(pi: &PartInfo) -> String {
        let pos = pi
            .short_name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(pi.short_name.len());
        crate::ngl::reader::mac_roman_to_string(&pi.short_name[..pos])
    }
}

// =============================================================================
// Type 1: TAIL
// =============================================================================

/// Tail object (Type 1) - marks end of object list
/// Source: NObjTypes.h lines 63-67
#[derive(Debug, Clone)]
pub struct Tail {
    pub header: ObjectHeader,
}

// =============================================================================
// Type 2: SYNC (with ANOTE subobject)
// =============================================================================

/// Note subobject (for SYNC and GRSYNC)
/// Source: NObjTypes.h lines 75-118
///
/// This is the most complex subobject, with ~30 fields spanning L/G/P domains.
/// A "note" is a normal or small note or rest, perhaps a cue note, but not a grace note.
#[derive(Debug, Clone)]
pub struct ANote {
    pub header: SubObjHeader, // subType (l_dur): LG: <0=n measure rest, 0=unknown, >0=Logical (CMN) dur. code
    pub in_chord: bool,       // True if note is part of a chord
    pub rest: bool,           // LGP: True=rest (=> ignore accident, ystem, etc.)
    pub unpitched: bool,      // LGP: True=unpitched note
    pub beamed: bool,         // True if beamed
    pub other_stem_side: bool, // G: True if note goes on "wrong" side of stem
    pub yqpit: ShortQd, // LG: clef-independent dist. below middle C ("pitch") (unused for rests)
    pub xd: Ddist,      // G: head X position
    pub yd: Ddist,      // G: head Y position
    pub ystem: Ddist,   // G: endpoint of stem (unused for rests)
    pub play_time_delta: i16, // P: PDURticks before/after timeStamp when note starts
    pub play_dur: i16,  // P: PDURticks that note plays for
    pub p_time: i16,    // P: PDURticks play time; for internal use by Tuplet routines
    pub note_num: u8,   // P: MIDI note number (unused for rests)
    pub on_velocity: u8, // P: MIDI note-on velocity, normally loudness (unused for rests)
    pub off_velocity: u8, // P: MIDI note-off (release) velocity (unused for rests)
    pub tied_l: bool,   // LGP: True if tied to left
    pub tied_r: bool,   // LGP: True if tied to right
    pub x_move_dots: u8, // G: X-offset on aug. dot position (quarter-spaces)
    pub y_move_dots: u8, // G: Y-offset on aug. dot pos. (half-spaces, 2=same as note, except 0=invisible)
    pub ndots: u8,       // LG: No. of aug. dots
    pub voice: i8,       // L: Voice number
    pub rsp_ignore: u8, // True if note's chord should not affect automatic spacing (unused as of v. 5.9)
    pub accident: u8,   // LG: 0=none, 1--5=dbl. flat--dbl. sharp (unused for rests)
    pub acc_soft: bool, // L: Was accidental generated by Nightingale?
    pub courtesy_acc: u8, // G: Accidental is a "courtesy accidental"
    pub xmove_acc: u8,  // G: X-offset to left on accidental position
    pub play_as_cue: bool, // LP: True = play note as cue, ignoring dynamic marks (unused as of v. 5.9)
    pub micropitch: u8,    // LP: Microtonal pitch modifier (unused as of v. 5.9)
    pub merged: u8,        // Temporary flag for Merge functions
    pub double_dur: u8,    // G: Draw as if double the actual duration
    pub head_shape: u8,    // G: Special notehead or rest shape; see HeadShape enum
    pub first_mod: Link,   // LG: Note-related symbols (articulation, fingering, etc.)
    pub slurred_l: bool,   // G: True if endpoint of slur to left
    pub slurred_r: bool,   // G: True if endpoint of slur to right
    pub in_tuplet: bool,   // True if in a tuplet
    pub in_ottava: bool,   // True if in an octave sign
    pub small: bool,       // G: True if a small (cue, cadenza-like, etc.) note
    pub temp_flag: u8,     // Temporary flag for benefit of functions that need it
    pub art_harmonic: u8, // Artificial harmonic: stopped, touched, sounding, normal note (unused as of v. 6.0)
    pub user_id: u16,     // User ID number (unused as of v. 6.0)
    pub nh_segment: [u8; 6], // Segments of notehead graph
    pub reserved_n: i32,  // For future use (unused as of v. 6.0)
}

/// Sync object (Type 2) - synchronous collection of notes/rests
/// Source: NObjTypes.h lines 120-123
#[derive(Debug, Clone)]
pub struct Sync {
    pub header: ObjectHeader,
    pub time_stamp: u16, // P: PDURticks since beginning of measure
}

/// Notehead and rest appearances
/// Source: NObjTypes.h lines 125-137
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadShape {
    NoVis = 0,         // Notehead/rest invisible
    NormalVis = 1,     // Normal appearance
    XShape = 2,        // "X" head (notes only)
    HarmonicShape = 3, // "Harmonic" hollow head (notes only)
    SquareHShape = 4,  // Square hollow head (notes only)
    SquareFShape = 5,  // Square filled head (notes only)
    DiamondHShape = 6, // Diamond-shaped hollow head (notes only)
    DiamondFShape = 7, // Diamond-shaped filled head (notes only)
    HalfnoteShape = 8, // Halfnote head (for Schenker, etc.) (notes only)
    SlashShape = 9,    // Chord slash
    NothingVis = 10,   // EVERYTHING (head/rest, stem, aug. dots, etc.) invisible
}

// =============================================================================
// Type 3: RPTEND (Repeat End, with ARPTEND subobject)
// =============================================================================

/// Repeat end subobject
/// Source: NObjTypes.h lines 142-147
#[derive(Debug, Clone)]
pub struct ARptEnd {
    pub header: SubObjHeader, // subType is in object so unused here
    pub conn_above: u8,       // True if connected above
    pub filler: u8,           // (unused)
    pub conn_staff: i8,       // Staff to connect to; valid if connAbove True
}

/// Repeat end object (Type 3) - D.C., D.S., etc.
/// Source: NObjTypes.h lines 149-156
#[derive(Debug, Clone)]
pub struct RptEnd {
    pub header: ObjectHeader,
    pub first_obj: Link, // Beginning of ending or NILINK
    pub start_rpt: Link, // Repeat start point or NILINK
    pub end_rpt: Link,   // Repeat end point or NILINK
    pub sub_type: i8,    // Code from RptEndType enum
    pub count: u8,       // Number of times to repeat
}

/// Repeat end types
/// Source: NObjTypes.h lines 158-166
/// Note: Codes must be the same as equivalent MEASUREs!
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RptEndType {
    RptDc = 1,
    RptDs = 2,
    RptSegno1 = 3,
    RptSegno2 = 4,
    RptL = 5,  // Must match BAR_RPT_L
    RptR = 6,  // Must match BAR_RPT_R
    RptLr = 7, // Must match BAR_RPT_LR
}

// =============================================================================
// Type 4: PAGE
// =============================================================================

/// Page object (Type 4)
/// Source: NObjTypes.h lines 171-178
#[derive(Debug, Clone)]
pub struct Page {
    pub header: ObjectHeader,
    pub l_page: Link, // Links to left and right Pages
    pub r_page: Link,
    pub sheet_num: i16,                  // Sheet number: indexed from 0
    pub header_str_offset: StringOffset, // (unused; when used, should be STRINGOFFSETs)
    pub footer_str_offset: StringOffset,
}

// =============================================================================
// Type 5: SYSTEM
// =============================================================================

/// System object (Type 5)
/// Source: NObjTypes.h lines 183-191
#[derive(Debug, Clone)]
pub struct System {
    pub header: ObjectHeader,
    pub l_system: Link, // Links to left and right Systems
    pub r_system: Link,
    pub page_l: Link,       // Link to previous (enclosing) Page
    pub system_num: i16,    // System number: indexed from 1
    pub system_rect: DRect, // DRect bounding box for entire system, rel to Page
    pub sys_desc_ptr: u64,  // (unused) ptr to data describing left edge of System (was Ptr)
}

// =============================================================================
// Type 6: STAFF (with ASTAFF subobject)
// =============================================================================

pub const SHOW_ALL_LINES: u8 = 15;

/// Staff subobject
/// Source: NObjTypes.h lines 198-226
#[derive(Debug, Clone)]
pub struct AStaff {
    pub next: Link,             // Index of next subobject
    pub staffn: i8,             // Staff number
    pub selected: bool,         // True if subobject is selected
    pub visible: bool,          // True if object is visible
    pub filler_stf: bool,       // Unused
    pub staff_top: Ddist,       // Relative to systemRect.top
    pub staff_left: Ddist,      // Always 0 now; rel to systemRect.left
    pub staff_right: Ddist,     // Relative to systemRect.left
    pub staff_height: Ddist,    // Staff height
    pub staff_lines: i8,        // Number of lines in staff: 0..6 (always 5 for now)
    pub font_size: i16,         // Preferred font size for this staff
    pub flag_leading: Ddist,    // (unused) Vertical space between flags
    pub min_stem_free: Ddist,   // (unused) Min. flag-free length of note stem
    pub ledger_width: Ddist,    // (unused) Standard ledger line length
    pub note_head_width: Ddist, // Width of common note heads
    pub frac_beam_width: Ddist, // Fractional beam length
    pub space_below: Ddist,     // Vert space occupied by stf; stored in case stf made invis
    pub clef_type: i8,          // Clef context
    pub dynamic_type: i8,       // Dynamic marking context
    pub ks_info: KsInfo,        // Key signature context (WHOLE_KSINFO)
    pub time_sig_type: i8,      // Time signature context
    pub numerator: i8,
    pub denominator: i8,
    pub filler: u8,       // Unused
    pub show_ledgers: u8, // True if drawing ledger lines of notes on this staff (default if showLines>0)
    pub show_lines: u8, // 0=show 0 staff lines, 1=only middle line (of 5-line staff), SHOW_ALL_LINES=all lines
}

/// Staff object (Type 6)
/// Source: NObjTypes.h lines 228-233
#[derive(Debug, Clone)]
pub struct Staff {
    pub header: ObjectHeader,
    pub l_staff: Link, // Links to left and right Staffs
    pub r_staff: Link,
    pub system_l: Link, // Link to previous (enclosing) System
}

// =============================================================================
// Type 7: MEASURE (with AMEASURE subobject)
// =============================================================================

/// Measure subobject
/// Source: NObjTypes.h lines 238-256
#[derive(Debug, Clone)]
pub struct AMeasure {
    pub header: SubObjHeader,  // subType=barline type (see BarlineType enum)
    pub measure_visible: bool, // True if measure contents are visible
    pub conn_above: bool,      // True if connected to barline above
    pub filler1: u8,
    pub filler2: i8,
    pub reserved_m: i16,  // Formerly <oldFakeMeas>; keep space for future use
    pub measure_num: i16, // Internal measure number; first is always 0
    pub meas_size_rect: DRect, // Bounding box of measure, V rel. to System top & H to meas. xd
    pub conn_staff: i8,   // Staff to connect to (valid if >0 and !connAbove)
    pub clef_type: i8,    // Clef context
    pub dynamic_type: i8, // Dynamic marking context
    pub ks_info: KsInfo,  // Key signature context (WHOLE_KSINFO)
    pub time_sig_type: i8, // Time signature context
    pub numerator: i8,
    pub denominator: i8,
    pub x_mn_std_offset: ShortStd, // Horiz. offset on measure number position
    pub y_mn_std_offset: ShortStd, // Vert. offset on measure number position
}

/// Measure object (Type 7)
/// Source: NObjTypes.h lines 258-269
#[derive(Debug, Clone)]
pub struct Measure {
    pub header: ObjectHeader,
    pub filler_m: i8,    // Unused
    pub l_measure: Link, // Links to left and right Measures
    pub r_measure: Link,
    pub system_l: Link,      // Link to owning System
    pub staff_l: Link,       // Link to owning Staff
    pub fake_meas: i16,      // True=not really a measure (i.e., barline ending system)
    pub space_percent: i16,  // Percentage of normal horizontal spacing used
    pub measure_b_box: Rect, // Bounding box of all measure subobjs, in pixels, paper-rel.
    pub l_time_stamp: i32,   // P: PDURticks since beginning of score
}

/// Barline types
/// Source: NObjTypes.h lines 271-280
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarlineType {
    BarSingle = 1,
    BarDouble = 2,
    BarFinalDbl = 3,
    BarHeavyDbl = 4, // (unused)
    BarRptL = 5,     // Codes must be the same as equivalent RPTENDs!
    BarRptR = 6,
    BarRptLr = 7,
}

pub const BAR_LAST: u8 = BarlineType::BarRptLr as u8;

// =============================================================================
// Type 8: CLEF (with ACLEF subobject)
// =============================================================================

/// Clef subobject
/// Source: NObjTypes.h lines 285-291
#[derive(Debug, Clone)]
pub struct AClef {
    pub header: SubObjHeader,
    pub filler1: u8,
    pub small: u8, // True to draw in small characters
    pub filler2: u8,
    pub xd: Ddist, // DDIST position
    pub yd: Ddist,
}

/// Clef object (Type 8)
/// Source: NObjTypes.h lines 293-296
#[derive(Debug, Clone)]
pub struct Clef {
    pub header: ObjectHeader,
    pub in_measure: bool, // True if object is in a Measure, False if not
}

/// Clef subtypes
/// Source: NObjTypes.h lines 298-314
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClefType {
    Treble8Clef = 1,  // unused
    FrViolinClef = 2, // unused
    TrebleClef = 3,
    SopranoClef = 4,
    MzSopranoClef = 5,
    AltoClef = 6,
    TrTenorClef = 7,
    TenorClef = 8,
    BaritoneClef = 9,
    BassClef = 10,
    Bass8bClef = 11, // unused
    PercClef = 12,
}

pub const LOW_CLEF: u8 = ClefType::Treble8Clef as u8;
pub const HIGH_CLEF: u8 = ClefType::PercClef as u8;

// =============================================================================
// Type 9: KEYSIG (with AKEYSIG subobject)
// =============================================================================

/// Key signature subobject
/// Source: NObjTypes.h lines 319-327
#[derive(Debug, Clone)]
pub struct AKeySig {
    pub header: SubObjHeader, // subType=no. of naturals, if nKSItems==0
    pub nonstandard: u8,      // True if not a standard CMN key sig.
    pub filler1: u8,
    pub small: u8, // (unused so far) True to draw in small characters
    pub filler2: i8,
    pub xd: Ddist,       // DDIST horizontal position
    pub ks_info: KsInfo, // WHOLE_KSINFO
}

/// Key signature object (Type 9)
/// Source: NObjTypes.h lines 329-332
#[derive(Debug, Clone)]
pub struct KeySig {
    pub header: ObjectHeader,
    pub in_measure: bool, // True if object is in a Measure, False if not
}

// =============================================================================
// Type 10: TIMESIG (with ATIMESIG subobject)
// =============================================================================

/// Time signature subobject
/// Source: NObjTypes.h lines 337-345
#[derive(Debug, Clone)]
pub struct ATimeSig {
    pub header: SubObjHeader,
    pub filler: u8,     // Unused--put simple/compound/other here?
    pub small: u8,      // (unused) True to draw in small characters
    pub conn_staff: i8, // (unused) bottom staff no.
    pub xd: Ddist,      // DDIST position
    pub yd: Ddist,
    pub numerator: i8,   // Numerator
    pub denominator: i8, // Denominator
}

/// Time signature object (Type 10)
/// Source: NObjTypes.h lines 347-350
#[derive(Debug, Clone)]
pub struct TimeSig {
    pub header: ObjectHeader,
    pub in_measure: bool, // True if object is in a Measure, False if not
}

/// Time signature subtypes
/// Source: NObjTypes.h lines 352-366
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSigType {
    NOverD = 1,
    CTime = 2,
    CutTime = 3,
    NOnly = 4,
    ZeroTime = 5,
    NOverQuarter = 6,
    NOverEighth = 7,
    NOverHalf = 8,
    NOverDottedQuarter = 9,
    NOverDottedEighth = 10,
}

pub const LOW_TSTYPE: u8 = TimeSigType::NOverD as u8;
pub const HIGH_TSTYPE: u8 = TimeSigType::NOverDottedEighth as u8;

// =============================================================================
// Type 11: BEAMSET (with ANOTEBEAM subobject)
// =============================================================================

/// Note beam subobject
/// Source: NObjTypes.h lines 371-378
#[derive(Debug, Clone)]
pub struct ANoteBeam {
    pub next: Link,       // Index of next subobject
    pub bp_sync: Link,    // Link to Sync containing note/chord
    pub startend: i8,     // No. of beams to start/end (+/-) on note/chord
    pub fracs: u8,        // No. of fractional beams on note/chord
    pub frac_go_left: u8, // Do fractional beams point left?
    pub filler: u8,       // Unused
}

/// Beamset object (Type 11)
/// Source: NObjTypes.h lines 380-391
#[derive(Debug, Clone)]
pub struct BeamSet {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub voice: i8,        // Voice number
    pub thin: u8,         // True=narrow lines, False=normal width
    pub beam_rests: u8,   // True if beam can contain rests
    pub feather: u8,      // (unused) 0=normal, 1=feather L end (accel.), 2=feather R (decel.)
    pub grace: u8,        // True if beam consists of grace notes
    pub first_system: u8, // True if on first system of cross-system beam
    pub cross_staff: u8,  // True if the beam is cross-staff: staffn=top staff
    pub cross_system: u8, // True if the beam is cross-system
}

/// Beam type enum
/// Source: NObjTypes.h lines 393-396
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeamType {
    NoteBeam = 0,
    GrNoteBeam = 1,
}

/// Beam info structure (not an object/subobject)
/// Source: NObjTypes.h lines 398-404
#[derive(Debug, Clone)]
pub struct BeamInfo {
    pub start: i8,        // Index of starting note/rest in BEAMSET
    pub stop: i8,         // Index of ending note/rest in BEAMSET
    pub start_lev: i8,    // Vertical slot no. at start (0=end of stem; -=above)
    pub stop_lev: i8,     // Vertical slot no. at end (0=end of stem; -=above)
    pub frac_go_left: i8, // Do fractional beams point left?
}

// =============================================================================
// Type 12: CONNECT (with ACONNECT subobject)
// =============================================================================

/// Connect subobject
/// Source: NObjTypes.h lines 409-420
#[derive(Debug, Clone)]
pub struct AConnect {
    pub next: Link,     // Index of next subobject
    pub selected: bool, // True if subobject is selected
    pub filler: u8,
    pub conn_level: u8,   // Code from ConnLevel enum
    pub connect_type: u8, // Code from ConnectType enum
    pub staff_above: i8,  // Upper staff no. (top of line or curly) (valid if connLevel!=0)
    pub staff_below: i8,  // Lower staff no. (bottom of " ) (valid if connLevel!=0)
    pub xd: Ddist,        // DDIST position
    pub first_part: Link, // (Unused) LINK to first part of group or connected part if not a group
    pub last_part: Link,  // (Unused) LINK to last part of group or NILINK if not a group
}

/// Connect object (Type 12)
/// Source: NObjTypes.h lines 422-425
#[derive(Debug, Clone)]
pub struct Connect {
    pub header: ObjectHeader,
    pub conn_filler: Link, // Unused
}

/// Connect types
/// Source: NObjTypes.h lines 427-437
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectType {
    ConnectLine = 1,
    ConnectBracket = 2,
    ConnectCurly = 3,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnLevel {
    SystemLevel = 0,
    GroupLevel = 1,
    PartLevel = 7,
}

// =============================================================================
// Type 13: DYNAMIC (with ADYNAMIC subobject)
// =============================================================================

/// Dynamic subobject
/// Source: NObjTypes.h lines 442-454
#[derive(Debug, Clone)]
pub struct ADynamic {
    pub header: SubObjHeader, // subType is unused
    pub mouth_width: u8,      // Width of mouth for hairpin
    pub small: u8,            // True to draw in small characters
    pub other_width: u8,      // Width of other (non-mouth) end for hairpin
    pub xd: Ddist,            // (unused)
    pub yd: Ddist,            // Position offset from staff top
    pub endxd: Ddist,         // Position offset from lastSyncL for hairpins
    pub endyd: Ddist,         // Position offset from staff top for hairpins
    pub d_mod_code: u8,       // Code for modifier (see enum below) (unused)
    pub cross_staff: u8,      // 0=normal, 1=also affects staff above, 2=also staff below
}

/// Dynamic object (Type 13)
/// Source: NObjTypes.h lines 456-463
#[derive(Debug, Clone)]
pub struct Dynamic {
    pub header: ObjectHeader,
    pub dynamic_type: i8, // Code for dynamic marking (see DynamicType enum)
    pub filler: bool,
    pub cross_sys: bool,    // (unused) Whether cross-system
    pub first_sync_l: Link, // Sync dynamic or hairpin start is attached to
    pub last_sync_l: Link,  // Sync hairpin end is attached to or NILINK
}

/// Dynamic marking types
/// Source: NObjTypes.h lines 466-494
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicType {
    PpppDynam = 1,
    PppDynam = 2,
    PpDynam = 3,
    PDynam = 4,
    MpDynam = 5,
    MfDynam = 6,
    FDynam = 7,
    FfDynam = 8,
    FffDynam = 9,
    FfffDynam = 10,
    PiupDynam = 11, // FIRSTREL_DYNAM
    MenopDynam = 12,
    MenofDynam = 13,
    PiufDynam = 14,
    SfDynam = 15, // FIRSTSF_DYNAM
    FzDynam = 16,
    SfzDynam = 17,
    RfDynam = 18,
    RfzDynam = 19,
    FpDynam = 20,
    SfpDynam = 21,
    DimDynam = 22,   // FIRSTHAIRPIN_DYNAM - Hairpin open at left ("diminuendo")
    CrescDynam = 23, // Hairpin open at right ("crescendo")
}

pub const FIRSTREL_DYNAM: u8 = DynamicType::PiupDynam as u8;
pub const FIRSTSF_DYNAM: u8 = DynamicType::SfDynam as u8;
pub const FIRSTHAIRPIN_DYNAM: u8 = DynamicType::DimDynam as u8;
pub const LAST_DYNAM: u8 = 24;

// =============================================================================
// Type 14: MODNR (subobject only - no main object)
// =============================================================================

/// Note/rest modifier subobject
/// Source: NObjTypes.h lines 512-521
#[derive(Debug, Clone)]
pub struct AModNr {
    pub next: Link,        // Index of next subobject
    pub selected: bool,    // True if subobject is selected
    pub visible: bool,     // True if subobject is visible
    pub soft: bool,        // True if subobject is program-generated
    pub xstd: u8,          // Note-relative position (FIXME: should be STDIST)
    pub mod_code: u8,      // Which note modifier
    pub data: i8,          // Modifier-dependent
    pub ystdpit: ShortStd, // Clef-independent dist. below middle C ("pitch")
}

pub const XSTD_OFFSET: i8 = 16; // 2**(xstd fieldwidth-1) to fake signed value

/// Modifier codes
/// Source: NObjTypes.h lines 525-549
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModCode {
    ModFermata = 10, // Leave 0 thru 9 for digits
    ModTrill = 11,
    ModAccent = 12,
    ModHeavyAccent = 13,
    ModStaccato = 14,
    ModWedge = 15,
    ModTenuto = 16,
    ModMordent = 17,
    ModInvMordent = 18,
    ModTurn = 19,
    ModPlus = 20,
    ModCircle = 21,
    ModUpbow = 22,
    ModDownbow = 23,
    ModTremolo1 = 24,
    ModTremolo2 = 25,
    ModTremolo3 = 26,
    ModTremolo4 = 27,
    ModTremolo5 = 28,
    ModTremolo6 = 29,
    ModHeavyAccStacc = 30,
    ModLongInvMordent = 31,
    ModFakeAugDot = 127, // Augmentation dot is not really a MODNR
}

// =============================================================================
// Type 15: GRAPHIC (with AGRAPHIC subobject)
// =============================================================================

/// Graphic subobject
/// Source: NObjTypes.h lines 554-557
#[derive(Debug, Clone)]
pub struct AGraphic {
    pub next: Link,
    pub str_offset: StringOffset, // Index return by String Manager library
}

/// Graphic object (Type 15)
/// Source: NObjTypes.h lines 559-583
#[derive(Debug, Clone)]
pub struct Graphic {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader, // NB: staff number can be 0 here
    pub graphic_type: i8,         // Graphic class (subtype)
    pub voice: i8,                // Voice number (but with some types of relObjs, NOONE)
    pub enclosure: u8,            // Enclosure type; see EnclosureType enum
    pub justify: u8,              // (unused) justify left/center/right
    pub v_constrain: bool,        // (unused) True if object is vertically constrained
    pub h_constrain: bool,        // (unused) True if object is horizontally constrained
    pub multi_line: u8, // True if string contains multiple lines of text (delimited by CR)
    pub info: i16,      // PICT res. ID (GRPICT); char (GRChar); length (GRArpeggio); etc.
    pub gu_handle: u64, // Handle to resource, or NULL (union with thickness)
    pub gu_thickness: i16, // Percent of interline space (union with handle)
    pub font_ind: i8,   // Index into font name table (GRChar,GRString only)
    pub rel_f_size: u8, // True if size is relative to staff size (GRChar,GRString only)
    pub font_size: u8,  // If relSize, small..large code, else point size (GRChar,GRString only)
    pub font_style: i16, // (GRChar,GRString only)
    pub info2: i16,     // Sub-subtype (GRArpeggio), 2nd y (GRDraw), _expanded_ (GRString)
    pub first_obj: Link, // Link to obj left end is relative to or NULL
    pub last_obj: Link, // Link to obj right end is relative to or NULL; ignored for most graphicTypes
}

/// Graphic types
/// Source: NObjTypes.h lines 587-641
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicType {
    GrPict = 1,        // (unimplemented; anyway PICTs are obsolete) PICT
    GrChar = 2,        // (unimplemented) single character
    GrString = 3,      // Character string
    GrLyric = 4,       // Lyric character string
    GrDraw = 5,        // Pure graphic: so far, only lines; someday, MiniDraw
    GrMidiPatch = 6,   // (unimplemented) MIDI program change
    GrRehearsal = 7,   // Rehearsal mark
    GrChordSym = 8,    // Chord symbol
    GrArpeggio = 9,    // Arpeggio or non-arpeggio sign
    GrChordFrame = 10, // Chord frame (for guitar, etc.)
    GrMidiPan = 11,
    GrSusPedalDown = 12,
    GrSusPedalUp = 13,
}

pub const GR_LAST_TYPE: u8 = GraphicType::GrSusPedalUp as u8;

pub const MIDI_SUSTAIN_ON: u8 = 127;
pub const MIDI_SUSTAIN_OFF: u8 = 0;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrJustify {
    GrJustLeft = 1,   // Graphic is left justified
    GrJustRight = 2,  // Graphic is right justified
    GrJustBoth = 3,   // Graphic is left and right justified
    GrJustCenter = 4, // Graphic is centered
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrRelSize {
    GrTiny = 1,
    GrVSmall = 2,
    GrSmall = 3,
    GrMedium = 4,
    GrLarge = 5,
    GrVLarge = 6,
    GrJumbo = 7,
    Gr1 = 8,
    GrStaffHeight = 9,
}

pub const GR_LAST_SIZE: u8 = GrRelSize::GrStaffHeight as u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnclosureType {
    EnclNone = 0,
    EnclBox = 1,
    EnclCircle = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpeggioType {
    Arp = 0,
    NonArp = 1,
}

// Macro for extracting arpeggio info from info2
// Source: NObjTypes.h line 585
pub fn arpinfo(info2: i16) -> u16 {
    (info2 as u16) >> 13
}

// =============================================================================
// Type 16: OTTAVA (with ANOTEOTTAVA subobject)
// =============================================================================

/// Note ottava subobject
/// Source: NObjTypes.h lines 646-649
#[derive(Debug, Clone)]
pub struct ANoteOttava {
    pub next: Link,    // Index of next subobject
    pub op_sync: Link, // Link to Sync containing note/chord (not rest)
}

/// Ottava object (Type 16)
/// Source: NObjTypes.h lines 651-666
#[derive(Debug, Clone)]
pub struct Ottava {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub no_cutoff: u8,     // True to suppress cutoff at right end of octave sign
    pub cross_staff: u8,   // (unused) True if the octave sign is cross-staff
    pub cross_system: u8,  // (unused) True if the octave sign is cross-system
    pub oct_sign_type: u8, // Class of octave sign
    pub filler: i8,        // Unused
    pub number_vis: bool,
    pub unused1: bool,
    pub brack_vis: bool,
    pub unused2: bool,
    pub nxd: Ddist, // (unused) DDIST position of number
    pub nyd: Ddist,
    pub xd_first: Ddist, // DDIST position of bracket
    pub yd_first: Ddist,
    pub xd_last: Ddist,
    pub yd_last: Ddist,
}

/// Ottava types
/// Source: NObjTypes.h lines 668-675
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OttavaType {
    Ottava8va = 1,
    Ottava15ma = 2,
    Ottava22ma = 3,
    Ottava8vaBassa = 4,
    Ottava15maBassa = 5,
    Ottava22maBassa = 6,
}

// =============================================================================
// Type 17: SLUR (with ASLUR subobject)
// =============================================================================

/// Slur behavior types
/// Source: NObjTypes.h line 682
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlursorType {
    SNew = 0,
    SStart = 1,
    SC0 = 2,
    SC1 = 3,
    SEnd = 4,
    SWhole = 5,
    SExtend = 6,
}

/// Spline segment for slur
/// Source: NObjTypes.h lines 684-688
#[derive(Debug, Clone)]
pub struct SplineSeg {
    pub knot: DPoint, // Coordinates of knot relative to startPt
    pub c0: DPoint,   // Coordinates of first control point relative to knot
    pub c1: DPoint,   // Coordinates of second control pt relative to endpoint
}

/// Slur subobject
/// Source: NObjTypes.h lines 690-703
#[derive(Debug, Clone)]
pub struct ASlur {
    pub next: Link,     // Index of next subobject
    pub selected: bool, // True if subobject is selected
    pub visible: bool,  // True if subobject is visible
    pub soft: bool,     // True if subobject is program-generated
    pub dashed: bool,   // True if slur should be shown as dashed line
    pub filler: bool,
    pub bounds: Rect,    // Bounding box of whole slur
    pub first_ind: i8,   // Starting note index in chord of tie
    pub last_ind: i8,    // Ending note index in chord of tie
    pub reserved: i32,   // For later expansion (e.g., to multi-segment slurs)
    pub seg: SplineSeg,  // For now, one slur spline segment always defined
    pub start_pt: Point, // Base points (note positions), paper-rel.
    pub end_pt: Point,
    pub end_knot: DPoint, // End point of last spline segment, relative to endPt
}

/// Slur object (Type 17)
/// Source: NObjTypes.h lines 705-718
#[derive(Debug, Clone)]
pub struct Slur {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub voice: i8,          // Voice number
    pub philler: u8, // A "filler" (unused); funny name to avoid confusion with ASLUR's filler
    pub cross_staff: u8, // True if the slur is cross-staff: staffn=top staff(?)
    pub cross_stf_back: u8, // True if the slur goes from a lower (position, not no.) stf to higher
    pub cross_system: u8, // True if the slur is cross-system
    pub temp_flag: bool, // Temporary flag for benefit of functions that need it
    pub used: bool,  // (Unused :-) )
    pub tie: bool,   // True if tie, else slur
    pub first_sync_l: Link, // Link to sync with 1st slurred note or to slur's system's init. measure
    pub last_sync_l: Link,  // Link to sync with last slurred note or to slur's system
}

// =============================================================================
// Type 18: TUPLET (with ANOTETUPLE subobject)
// =============================================================================

/// Tuplet parameter structure (used by TupletDialog)
/// Source: NObjTypes.h lines 725-733
#[derive(Debug, Clone)]
pub struct TupleParam {
    pub acc_num: u8,   // Accessory numeral (numerator) for Tuplet
    pub acc_denom: u8, // Accessory denominator
    pub dur_unit: i16, // Duration units of denominator
    pub num_vis: bool,
    pub denom_vis: bool,
    pub brack_vis: bool,
    pub is_fancy: bool,
}

/// Note tuple subobject
/// Source: NObjTypes.h lines 735-738
#[derive(Debug, Clone)]
pub struct ANoteTuple {
    pub next: Link,    // Index of next subobject
    pub tp_sync: Link, // Link to Sync containing note/chord/rest
}

/// Tuplet object (Type 18)
/// Source: NObjTypes.h lines 740-754
#[derive(Debug, Clone)]
pub struct Tuplet {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub acc_num: u8,   // Accessory numeral (numerator) for Tuplet
    pub acc_denom: u8, // Accessory denominator
    pub voice: i8,     // Voice number
    pub num_vis: u8,
    pub denom_vis: u8,
    pub brack_vis: u8,
    pub small: u8, // (unused so far) True to draw in small characters
    pub filler: u8,
    pub acnxd: Ddist, // DDIST position of accNum (now unused)
    pub acnyd: Ddist,
    pub xd_first: Ddist, // DDIST position of bracket
    pub yd_first: Ddist,
    pub xd_last: Ddist,
    pub yd_last: Ddist,
}

// =============================================================================
// Type 19: GRSYNC (Grace note sync, with AGRNOTE subobject)
// =============================================================================

/// Grace note subobject (same struct as ANOTE)
/// Source: NObjTypes.h lines 759-760
pub type AGrNote = ANote;

/// Grace sync object (Type 19)
/// Source: NObjTypes.h lines 762-764
#[derive(Debug, Clone)]
pub struct GrSync {
    pub header: ObjectHeader,
}

// =============================================================================
// Type 20: TEMPO
// =============================================================================

/// Tempo object (Type 20)
/// Source: NObjTypes.h lines 769-782
#[derive(Debug, Clone)]
pub struct Tempo {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub sub_type: i8,   // "Beat": same units as note's l_dur
    pub expanded: bool, // Stretch out the text?
    pub no_mm: bool,    // False = play at _tempoMM_ BPM, True = ignore it
    pub filler: u8,
    pub dotted: bool,                   // Does beat unit have an augmentation dot?
    pub hide_mm: bool,                  // False = show Metronome mark, True = don't show it
    pub tempo_mm: i16,                  // New playback speed in beats per minute
    pub str_offset: StringOffset,       // "tempo" string index return by String Manager
    pub first_obj_l: Link,              // Object tempo depends on
    pub metro_str_offset: StringOffset, // "metronome mark" index return by String Manager
}

// =============================================================================
// Type 21: SPACER
// =============================================================================

/// Spacer object (Type 21)
/// Source: NObjTypes.h lines 787-792
#[derive(Debug, Clone)]
pub struct Spacer {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub bottom_staff: i8, // Last staff on which space to be left
    pub sp_width: Stdist, // Amount of blank space to leave
}

// =============================================================================
// Type 22: ENDING
// =============================================================================

/// Ending object (Type 22)
/// Source: NObjTypes.h lines 797-806
#[derive(Debug, Clone)]
pub struct Ending {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
    pub first_obj_l: Link, // Object left end of ending is attached to
    pub last_obj_l: Link,  // Object right end of ending is attached to or NILINK
    pub no_l_cutoff: u8,   // True to suppress cutoff at left end of Ending
    pub no_r_cutoff: u8,   // True to suppress cutoff at right end of Ending
    pub end_num: u8,       // 0=no ending number or label, else code for the ending label
    pub endxd: Ddist,      // Position offset from lastObjL
}

// =============================================================================
// Type 23: PSMEAS (Pseudomeasure, with APSMEAS subobject)
// =============================================================================

/// Pseudomeasure subobject
/// Source: NObjTypes.h lines 814-819
///
/// Pseudomeasures are symbols that look like barlines but have no semantics, i.e., dotted
/// barlines and double bars that don't coincide with "real" barlines: they're G domain only,
/// while Measures are L and G domain.
#[derive(Debug, Clone)]
pub struct APsMeas {
    pub header: SubObjHeader, // subType=barline type (see PsMeasType enum)
    pub conn_above: bool,     // True if connected to barline above
    pub filler1: u8,          // (unused)
    pub conn_staff: i8,       // Staff to connect to (valid if >0 and !connAbove)
}

/// Pseudomeasure object (Type 23)
/// Source: NObjTypes.h lines 821-824
#[derive(Debug, Clone)]
pub struct PsMeas {
    pub header: ObjectHeader,
    pub filler: i8,
}

/// Pseudomeasure types (codes follow those for MEASUREs)
/// Source: NObjTypes.h lines 826-830
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsMeasType {
    PsmDotted = 8, // BAR_LAST+1
    PsmDouble = 9,
    PsmFinalDbl = 10, // unused
}

// =============================================================================
// Type 24: SUPEROBJECT (union of all object types)
// =============================================================================

/// Union of all object structs - maximum size of any record in object heap
/// Source: NObjTypes.h lines 833-863
///
/// In Rust, we don't use unions the same way. This is primarily for documentation
/// and to establish the maximum object size. The actual storage will be type-specific.
#[derive(Debug, Clone)]
pub enum SuperObject {
    Header(Header),
    Tail(Tail),
    Sync(Sync),
    RptEnd(RptEnd),
    Page(Page),
    System(System),
    Staff(Staff),
    Measure(Measure),
    Clef(Clef),
    KeySig(KeySig),
    TimeSig(TimeSig),
    BeamSet(BeamSet),
    Connect(Connect),
    Dynamic(Dynamic),
    Graphic(Graphic),
    Ottava(Ottava),
    Slur(Slur),
    Tuplet(Tuplet),
    GrSync(GrSync),
    Tempo(Tempo),
    Spacer(Spacer),
    Ending(Ending),
    PsMeas(PsMeas),
}

// =============================================================================
// Miscellaneous Structures (not actual object types)
// =============================================================================

/// Context structure (rendering/layout context)
/// Source: NObjTypes.h lines 871-899
#[derive(Debug, Clone)]
pub struct Context {
    pub visible: bool,            // True if (staffVisible && measureVisible)
    pub staff_visible: bool,      // True if staff is visible
    pub measure_visible: bool,    // True if measure is visible
    pub in_measure: bool,         // True if currently in measure
    pub paper: Rect,              // SHEET: paper rect in window coords
    pub sheet_num: i16,           // PAGE: sheet number
    pub system_num: i16,          // SYSTEM: number (unused)
    pub system_top: Ddist,        // Page relative top
    pub system_left: Ddist,       // Page relative left edge
    pub system_bottom: Ddist,     // Page relative bottom
    pub staff_top: Ddist,         // STAFF: page relative top
    pub staff_left: Ddist,        // Page relative left edge
    pub staff_right: Ddist,       // Page relative right edge
    pub staff_height: Ddist,      // Height
    pub staff_half_height: Ddist, // Height divided by 2
    pub staff_lines: i8,          // Number of lines
    pub show_lines: i8,           // 0=show no lines, 1=only middle line, or SHOW_ALL_LINES=show all
    pub show_ledgers: bool,       // True=show ledger lines for notes on this staff
    pub font_size: i16,           // Preferred font size
    pub measure_top: Ddist,       // MEASURE: page relative top
    pub measure_left: Ddist,      // Page relative left
    pub clef_type: i8,            // MISC: current clef type
    pub dynamic_type: i8,         // Dynamic marking
    pub ks_info: KsInfo,          // Key signature (WHOLE_KSINFO)
    pub prev_ks_info: KsInfo,     // Previous key signature (for cancellation naturals)
    pub time_sig_type: i8,        // Current time signature
    pub numerator: i8,
    pub denominator: i8,
}

/// Staff range structure
/// Source: NObjTypes.h lines 904-907
#[derive(Debug, Clone)]
pub struct StfRange {
    pub top_staff: i16,
    pub bottom_staff: i16,
}

/// Clipboard copy type enum
/// Source: NObjTypes.h lines 912-916
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyType {
    CopyTypeContent = 0,
    CopyTypeSystem = 1,
    CopyTypePage = 2,
}

/// Copy map structure
/// Source: NObjTypes.h lines 918-921
#[derive(Debug, Clone)]
pub struct CopyMap {
    pub src_l: Link,
    pub dst_l: Link,
}

/// Voice info structure (for Merge operations)
/// Source: NObjTypes.h lines 926-936
#[derive(Debug, Clone)]
pub struct VInfo {
    pub start_time: i32, // Start time in this voice
    pub first_stf: i16,  // First staff occupied by objs in this voice
    pub last_stf: i16,   // Last staff occupied by objs in this voice
    pub single_stf: u16, // Whether voice in this sys is on more than 1 stf
    pub has_v: u16,
    pub v_ok: u16,  // True if there is enough space in voice to merge
    pub v_bad: u16, // True if check of this voice caused abort
    pub overlap: u16,
}

/// Clipboard voice info structure
/// Source: NObjTypes.h lines 938-947
#[derive(Debug, Clone)]
pub struct ClipVInfo {
    pub start_time: i32,    // Start time in this voice
    pub clip_end_time: i32, // End time of clipboard in this voice
    pub first_stf: i16,     // First staff occupied by objs in this voice
    pub last_stf: i16,      // Last staff occupied by objs in this voice
    pub single_stf: i16,    // Whether voice in this sys on more than 1 stf
    pub has_v: i16,
    pub v_bad: i16,
}

/// Chord note structure
/// Source: NObjTypes.h lines 952-956
#[derive(Debug, Clone)]
pub struct ChordNote {
    pub yqpit: ShortQd,
    pub note_num: u8,
    pub note_l: Link,
}

/// Symbol table data for a CMN symbol
/// Source: NObjTypes.h lines 961-968
#[derive(Debug, Clone)]
pub struct SymData {
    pub cursor_id: i16, // Resource ID of cursor
    pub objtype: i8,    // Object type for symbol's opcode
    pub subtype: i8,    // Subtype
    pub symcode: u8,    // Input char. code for symbol (0=none)
    pub durcode: i8,    // Duration code for notes and rests
}

/// Symbol table data for an object-list object
/// Source: NObjTypes.h lines 970-977
#[derive(Debug, Clone)]
pub struct ObjData {
    pub obj_type: i8, // mEvent type for symbol's opcode
    pub just_type: i16,
    pub min_entries: i16,
    pub max_entries: i16,
    pub obj_rect_ordered: bool, // True=objRect meaningful & its .left should be in order
}

/// Generic object header structure
/// Source: NObjTypes.h lines 982-984
#[derive(Debug, Clone)]
pub struct ObjHdr {
    pub header: ObjectHeader,
}

/// Extended object structure
/// Source: NObjTypes.h lines 989-992
#[derive(Debug, Clone)]
pub struct Extend {
    pub header: ObjectHeader,
    pub ext_header: ExtObjHeader,
}
