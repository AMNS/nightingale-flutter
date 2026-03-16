//! MIDI export and playback support
//!
//! This module provides:
//! - MIDI event generation from score data (notes, channels, timing)
//! - Standard MIDI File (SMF) format generation
//! - Playback integration via flutter_rust_bridge
//!
//! Reference: OG Nightingale MIDI/CoreMIDIUtils.cp (41KB)

pub mod export;

pub use export::{export_to_midi, MidiExporter};
