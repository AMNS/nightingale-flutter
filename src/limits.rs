//! Constants defining limits for various Nightingale data structures.
//!
//! Ported from `Nightingale/src/Precomps/NLimits.h`
//!
//! CAUTION: Many of these limits are baked into the file format. Changing them
//! (especially MAXSTAVES, MAX_SCOREFONTS, MAX_COMMENT_LEN) will break file compatibility.

// Filenames
/// Maximum filename length (Carbon/legacy MacOS limit)
/// Source: NLimits.h:9
pub const FILENAME_MAXLEN: usize = 31;

// Staff and voice limits
/// Maximum number of staves attached to a system
/// Source: NLimits.h:13
/// WARNING: Changing this breaks file compatibility!
pub const MAXSTAVES: usize = 64;

/// Maximum number of staves in one part
/// Source: NLimits.h:14
pub const MAXSTPART: usize = 16;

/// Maximum number of staff lines/spaces (MIDI covers <11 octaves at 7/octave)
/// Source: NLimits.h:16
pub const MAX_STAFFPOS: usize = 76;

/// Maximum voice number in score
/// Source: NLimits.h:17
pub const MAXVOICES: usize = 100;

// Note/chord/beam limits
/// Maximum number of notes in a chord
/// Source: NLimits.h:18
pub const MAXCHORD: usize = 88;

/// Maximum number of notes/chords in a beamset
/// Source: NLimits.h:19
pub const MAXINBEAM: usize = 127;

/// Maximum number of notes/chords in an ottava
/// Source: NLimits.h:20
pub const MAXINOTTAVA: usize = 127;

/// Maximum number of notes/chords in a tuplet
/// Source: NLimits.h:21
pub const MAXINTUPLET: usize = 80;

/// Maximum tuplet numerator and denominator
/// Source: NLimits.h:22
pub const MAX_TUPLENUM: u8 = 255;

/// Maximum number of nodes (objects) in a measure
/// Source: NLimits.h:23
pub const MAX_MEASNODES: usize = 250;

/// Maximum legal SYNC l_dur code; shortest legal duration = 1/2^(MAX_L_DUR-WHOLE_L_DUR)
/// Source: NLimits.h:24-25
pub const MAX_L_DUR: i8 = 9;

/// Maximum number of items in key signature
/// Source: NLimits.h:26
pub const MAX_KSITEMS: usize = 7;

// Font limits
/// Maximum value of index into font style table
/// Source: NLimits.h:28
pub const MAX_FONTSTYLENUM: usize = 15; // FONT_R9 = 15 from defs.h

/// Maximum number of font families in one score
/// Source: NLimits.h:32
/// WARNING: Changing this breaks file compatibility!
pub const MAX_SCOREFONTS: usize = 20;

/// Maximum number of cursors used by symtable
/// Source: NLimits.h:33
pub const MAX_CURSORS: usize = 100;

// Time signature limits
/// Maximum time signature numerator
/// Source: NLimits.h:35
pub const MAX_TSNUM: i8 = 99;

/// Maximum time signature denominator
/// Source: NLimits.h:36
pub const MAX_TSDENOM: i8 = 64;

// Measure limits
/// Largest number for the first measure
/// Source: NLimits.h:38
pub const MAX_FIRSTMEASNUM: i32 = 4000;

/// Maximum number of measures in entire score
/// Source: NLimits.h:39
pub const MAX_SCORE_MEASURES: usize = 5000;

/// Maximum number of measures we can respace per call
/// Source: NLimits.h:40
pub const MAX_RSP_MEASURES: usize = 5000;

/// Maximum number of changes in one call to Master Page
/// Source: NLimits.h:42
pub const MAX_MPCHANGES: usize = 50;

/// Maximum pieces one note/rest can be "clarified" into
/// Source: NLimits.h:44
pub const MF_MAXPIECES: usize = 300;

// Magnification limits
/// Maximum reduction = 2^(MIN_MAGNIFY/2)
/// Source: NLimits.h:48
pub const MIN_MAGNIFY: i8 = -4;

/// Maximum magnification = 2^(MAX_MAGNIFY/2)
/// Source: NLimits.h:49
pub const MAX_MAGNIFY: i8 = 5;

/// Maximum legal staff rastral size number
/// Source: NLimits.h:51
pub const MAXRASTRAL: usize = 8;

/// Maximum number of ledger lines (22 reaches MIDI note 127 in bass clef)
/// Source: NLimits.h:53-54
pub const MAX_LEDGERS: usize = 22;

// Spacing limits
/// Minimum legal spacePercent for respacing
/// Source: NLimits.h:59
/// Note: If (MAXSPACE*RESFACTOR) exceeds SHRT_MAX, justification routines may fail
pub const MINSPACE: i16 = 10;

/// Maximum legal spacePercent for respacing
/// Source: NLimits.h:60
/// Note: If (MAXSPACE*RESFACTOR) exceeds SHRT_MAX, justification routines may fail
pub const MAXSPACE: i16 = 500;

// Text size limits
/// Minimum text size in points
/// Source: NLimits.h:62
pub const MIN_TEXT_SIZE: u8 = 4;

/// Maximum text size in points
/// Source: NLimits.h:63
pub const MAX_TEXT_SIZE: u8 = 127;

// Ending limits
/// Maximum length of any ending label
/// Source: NLimits.h:65
pub const MAX_ENDING_STRLEN: usize = 16;

/// Maximum number of ending labels
/// Source: NLimits.h:66
pub const MAX_ENDING_STRINGS: usize = 31;

// Comment length
/// One less than length of comment header field
/// Source: NLimits.h:70
/// WARNING: Changing this breaks file compatibility!
pub const MAX_COMMENT_LEN: usize = 255;

// Tempo limits
/// Minimum legal tempo in beats per minute
/// Source: NLimits.h:72
pub const MIN_BPM: i16 = 10;

/// Maximum legal tempo in beats per minute
/// Source: NLimits.h:73
pub const MAX_BPM: i16 = 1200;

// MIDI/playback limits
/// Maximum number of simultaneous notes for Play commands
/// Source: NLimits.h:75
pub const MAXEVENTLIST: usize = 128;

/// Maximum number of simultaneous notes in MIDI files
/// Source: NLimits.h:76
pub const MAXMFEVENTLIST: usize = 128;

/// Maximum safe measure duration in PDURticks (cf. ANOTE timeStamp field)
/// Source: NLimits.h:77
pub const MAX_SAFE_MEASDUR: i32 = 65500;
