//! MusicXML 4.0 import/export for Nightingale scores.
//!
//! Export: walks an InterpretedScore and produces a valid MusicXML file.
//! Import: parses a MusicXML file and produces an InterpretedScore.

pub mod export;
pub mod import;

pub use export::export_musicxml;
pub use import::import_musicxml;
