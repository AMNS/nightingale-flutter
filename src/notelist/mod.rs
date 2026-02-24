//! Notelist (.nl) parser module.
//!
//! Parses text-based Notelist files (V1 and V2 format) that serve as a
//! human-readable representation of Nightingale scores. Used as a test oracle
//! to validate binary .ngl file parsing.
//!
//! ## Format
//!
//! Notelist V2 format is line-based with key=value pairs:
//! - Header: `%%Notelist-V2 file='<name>' partstaves=<n> <staves...> startmeas=<n>`
//! - Records: N (note), R (rest), G (grace note), / (barline), C (clef),
//!   K (key sig), T (time sig), D (dynamic), A (text), M (tempo), P (tuplet),
//!   B (beam), % (comment)

pub mod parser;
pub mod to_score;

pub use parser::{parse_notelist, Notelist, NotelistRecord, ParseError};
pub use to_score::{
    clef_middle_c_half_ln, nl_midi_to_half_ln, notelist_to_score, notelist_to_score_with_config,
    NotelistLayoutConfig, VoiceRole, AC_DBLFLAT, AC_DBLSHARP, AC_FLAT, AC_NATURAL, AC_SHARP,
};
