//! MIDI export implementation
//!
//! Converts NGL/Notelist scores to Standard MIDI Files (SMF).
//!
//! References:
//! - OG NightingaleMIDI.cp (MIDI event generation and timing)
//! - OG CoreMIDIUtils.cp (duration conversion, PDUR calculations)
//! - OG MIDIPlay.cp (playback integration)
//! - SMF specification: https://www.midi.org/specifications/file-format-specifications/standard-midi-file-smf

use crate::ngl::interpret::InterpretedScore;
use crate::obj_types::{ANote, Sync};
use std::collections::BTreeMap;

// =============================================================================
// PDUR Constants and Duration Lookup
// =============================================================================

/// PDUR tick resolution: 480 ticks per quarter note
/// This allows exact representation of all standard note durations and
/// triplets, quintuplets, etc. via timing offset/duration pairs.
///
/// Reference: OG Precomps/defs.h line 333 (DFLT_BEATDUR = 480)
const DFLT_BEATDUR: u32 = 480;

/// PDUR unit = shortest representable duration (128th note)
/// = DFLT_BEATDUR / 32 = 15 ticks
///
/// Reference: OG Precomps/defs.h line 259 (PDURUNIT = 15)
const PDURUNIT: u32 = 15;

/// Lookup table: duration code (1-9) -> PDUR ticks
/// Generated at runtime by OG InitNightingale.cp lines 222-224:
///
/// ```
/// for (i = 1; i <= MAXDURDUR; i++)
///     l2p_durs[i] = l2p_durs[i-1] * 2;
/// ```
///
/// Starting from l2p_durs[9] = PDURUNIT = 15:
/// - l2p_durs[9] = 15     (128th note)
/// - l2p_durs[8] = 30     (64th note)
/// - l2p_durs[7] = 60     (32nd note)
/// - l2p_durs[6] = 120    (16th note)
/// - l2p_durs[5] = 240    (eighth note)
/// - l2p_durs[4] = 480    (quarter note)
/// - l2p_durs[3] = 960    (half note)
/// - l2p_durs[2] = 1920   (whole note)
/// - l2p_durs[1] = 3840   (breve)
///
/// Index 0 is unused; valid duration codes are 1-9.
fn get_l2p_durs() -> [u32; 10] {
    [0, 3840, 1920, 960, 480, 240, 120, 60, 30, 15]
}

// =============================================================================
// Duration Calculation
// =============================================================================

/// Convert logical duration code + dots to PDUR ticks.
///
/// # Arguments
/// * `dur_code` - Logical duration code (1-9): breve..128th note
/// * `n_dots` - Number of augmentation dots (0-3 typical)
///
/// # Returns
/// PDUR ticks (1/480 of a quarter note)
///
/// # Example
/// - Code 4 (quarter), 0 dots = 480 ticks
/// - Code 4 (quarter), 1 dot = 480 + 240 = 720 ticks
/// - Code 5 (eighth), 0 dots = 240 ticks
///
/// Reference: OG SpaceTime.cp lines 956-964 (Code2LDur function)
pub fn code_to_ldur(dur_code: u8, n_dots: u8) -> u32 {
    let l2p_durs = get_l2p_durs();

    // Ensure duration code is in valid range
    let code = dur_code.max(1).min(9) as usize;

    let mut note_dur = l2p_durs[code];
    for j in 1..=n_dots {
        let dot_code = (code + j as usize).min(9);
        note_dur += l2p_durs[dot_code];
    }

    note_dur
}

// =============================================================================
// SMF Encoding Helpers
// =============================================================================

/// Encode a value as variable-length quantity (VLQ) for MIDI
///
/// VLQ format: 0..0x7F encodes to single byte
/// Higher values use continuation bits (high bit set) in preceding bytes
///
/// Reference: SMF spec section on variable-length quantities
fn encode_vlq(value: u32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut v = value;

    // Collect bytes in reverse (LSB first)
    let mut bytes = Vec::new();
    loop {
        bytes.push((v & 0x7F) as u8);
        v >>= 7;
        if v == 0 {
            break;
        }
    }

    // Reverse and set continuation bits
    for (i, &byte) in bytes.iter().rev().enumerate() {
        if i < bytes.len() - 1 {
            result.push(byte | 0x80); // Set continuation bit
        } else {
            result.push(byte); // Last byte has no continuation bit
        }
    }

    result
}

/// Write a 4-byte big-endian value
fn write_be32(value: u32) -> [u8; 4] {
    [
        ((value >> 24) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    ]
}

/// Write a 2-byte big-endian value
fn write_be16(value: u16) -> [u8; 2] {
    [((value >> 8) & 0xFF) as u8, (value & 0xFF) as u8]
}

// =============================================================================
// MIDI Event Types
// =============================================================================

/// MIDI events emitted during score traversal
#[derive(Debug, Clone)]
pub enum MidiEvent {
    /// Program change: (channel, program)
    ProgramChange { channel: u8, program: u8 },
    /// Bank select (CC 0): (channel, bank)
    BankSelect { channel: u8, bank: u8 },
    /// Note on: (channel, note, velocity)
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    /// Note off: (channel, note)
    NoteOff { channel: u8, note: u8 },
    /// Set tempo in microseconds per quarter note
    SetTempo { tempo_us: u32 },
    /// Time signature: (numerator, denominator, clocks_per_quarter_note)
    TimeSignature {
        numerator: u8,
        denominator: u8,
        clocks_per_quarter: u8,
    },
}

/// Timed MIDI event (ready for SMF track generation)
#[derive(Debug, Clone)]
pub struct TimedEvent {
    pub time: u32, // PDUR ticks since start of measure/score
    pub event: MidiEvent,
}

// =============================================================================
// Pitch Conversion
// =============================================================================

/// MIDI note numbers for C in each octave (C-1 to C8)
/// Used as reference points for pitch conversion
const C_MIDI_NOTES: [u8; 10] = [12, 24, 36, 48, 60, 72, 84, 96, 108, 120];

/// Clef offsets in half-lines from Middle C
/// Determines the "Middle C half-line" position for each clef
/// Reference: OG Utilities/PitchUtils.cp lines 204-226 (Pitch2MIDI)
#[derive(Debug, Clone, Copy)]
pub enum ClefType {
    Treble = 10,      // G clef: Middle C is 10 half-lines below staff
    Bass = -2,        // F clef: Middle C is 2 half-lines below staff
    Alto = 4,         // C clef (C4): Middle C is 4 half-lines above middle
    Tenor = 6,        // C clef (C3): Middle C is 6 half-lines above middle
}

impl ClefType {
    /// Get clef offset in half-lines
    fn offset_half_lines(&self) -> i8 {
        match self {
            ClefType::Treble => 10,
            ClefType::Bass => -2,
            ClefType::Alto => 4,
            ClefType::Tenor => 6,
        }
    }
}

/// Convert yqpit (clef-independent quarter-line pitch) to MIDI note number
///
/// # Arguments
/// * `yqpit` - Clef-independent pitch in quarter-line units (signed short)
/// * `accidental` - Accidental code: 0=natural, 1=sharp, -1=flat, ±2=double, etc.
/// * `clef_offset` - Clef-specific offset in half-lines
///
/// # Returns
/// MIDI note number (0-127), or None if pitch is out of range
///
/// # Algorithm
/// 1. Convert yqpit (quarter-lines) → half-lines: yqpit * 2
/// 2. Add clef offset to get staff position in half-lines
/// 3. Convert half-lines to letter name + octave
/// 4. Add accidental (semitone offset)
/// 5. Clamp to MIDI range [0, 127]
///
/// Reference: OG Utilities/PitchUtils.cp lines 204-226 (Pitch2MIDI function)
pub fn yqpit_to_midi_note(yqpit: i16, accidental: i8, clef_offset: i8) -> Option<u8> {
    // Convert yqpit (quarter-lines) to half-lines
    let half_lines = (yqpit as i32) * 2 + (clef_offset as i32);

    // Convert half-lines to semitones from C0
    // Each staff line spans 2 half-lines = 1 whole step = 2 semitones
    // Middle C (MIDI 60) = octave 4, letter C
    // Half-line 0 corresponds to Middle C (MIDI 60)
    // Half-line +1 = C# (61), Half-line +2 = D (62), etc.
    // Half-line -1 = B (59), Half-line -2 = Bb (58), etc.

    let semitones_from_c = half_lines; // 1 half-line = 1 semitone
    let midi_note_i32 = 60 + semitones_from_c + (accidental as i32);

    // Clamp to valid MIDI range
    if midi_note_i32 < 0 || midi_note_i32 > 127 {
        return None;
    }

    Some(midi_note_i32 as u8)
}

/// Calculate MIDI velocity from note velocity and document velocity offset
///
/// # Arguments
/// * `on_velocity` - Note-specific velocity (0-127)
/// * `doc_velocity_offset` - Document-wide velocity offset (-127 to +127)
/// * `part_velocity` - Optional per-part velocity offset (0-127)
///
/// # Returns
/// Final velocity clamped to [1, 127] (0 is reserved for note-off)
///
/// Reference: OG Utilities/MIDIUtils.cp lines 119-152, 634-655
pub fn calculate_velocity(on_velocity: u8, doc_velocity_offset: i8, part_velocity: Option<u8>) -> u8 {
    let mut velocity = (on_velocity as i32) + (doc_velocity_offset as i32);

    if let Some(pv) = part_velocity {
        velocity += pv as i32;
    }

    // Clamp to valid MIDI velocity range [1, 127]
    // (0 is used for note-off events, so minimum playback velocity is 1)
    velocity.max(1).min(127) as u8
}

// =============================================================================
// MIDI Exporter
// =============================================================================

/// MIDI export engine
#[derive(Debug)]
pub struct MidiExporter {
    /// Events grouped by track (channel)
    tracks: BTreeMap<u8, Vec<TimedEvent>>,
    /// Default tempo in BPM
    default_tempo: u32,
}

impl MidiExporter {
    /// Create a new MIDI exporter
    pub fn new() -> Self {
        MidiExporter {
            tracks: BTreeMap::new(),
            default_tempo: 120, // Quarter = 120 BPM by default
        }
    }

    /// Set default tempo in BPM
    pub fn set_tempo(&mut self, bpm: u32) {
        self.default_tempo = bpm;
    }

    /// Convert a score to MIDI events
    ///
    /// # Implementation Notes
    /// - Walks the score object list (Syncs contain Notes)
    /// - Extracts: MIDI note number, velocity, duration, channel
    /// - Groups events by MIDI channel for multi-track output
    /// - Handles tuplets via play_dur field (not logical dur_code)
    /// - Skips rests (rest=true)
    ///
    /// Reference: OG NightingaleMIDI.cp Export functions
    pub fn export(&mut self, score: &InterpretedScore) {
        // Build mapping from staff_num to PartInfo for quick channel lookup
        let mut staff_to_part: BTreeMap<i32, usize> = BTreeMap::new();
        for (part_idx, part_info) in score.part_infos.iter().enumerate() {
            for staff in part_info.first_staff..=part_info.last_staff {
                staff_to_part.insert(staff as i32, part_idx);
            }
        }

        // Emit SetTempo events at start of score
        // OG uses defaultQuarterDur = 500000 microseconds = 120 BPM
        self.timed_events.push(TimedEvent {
            time: 0,
            event: MidiEvent::SetTempo {
                tempo_us: 500000,
            },
        });

        // Emit ProgramChange + BankSelect for each part/channel at score start
        for (part_idx, part_info) in score.part_infos.iter().enumerate() {
            let channel = part_info.channel;
            let program = part_info.patch_num;

            // Bank select (CC 0 if available)
            if part_info.bank_number0 > 0 {
                self.timed_events.push(TimedEvent {
                    time: 0,
                    event: MidiEvent::BankSelect {
                        channel,
                        bank: part_info.bank_number0,
                    },
                });
            }
            if part_info.bank_number32 > 0 {
                self.timed_events.push(TimedEvent {
                    time: 0,
                    event: MidiEvent::BankSelect {
                        channel,
                        bank: part_info.bank_number32,
                    },
                });
            }

            // Program change to set instrument
            self.timed_events.push(TimedEvent {
                time: 0,
                event: MidiEvent::ProgramChange {
                    channel,
                    program,
                },
            });
        }

        // Emit TimeSignature event (once per score)
        if let Some(time_sig) = &score.time_sig {
            self.timed_events.push(TimedEvent {
                time: 0,
                event: MidiEvent::TimeSignature {
                    numerator: time_sig.numerator,
                    denominator: time_sig.denominator,
                    clocks_per_quarter: 24, // Standard: 24 MIDI clocks per quarter note
                },
            });
        }

        // Walk score to extract note events
        // Tracks absolute PDUR ticks since start of score
        let mut abs_time: u32 = 0;

        for obj in score.walk() {
            // Only process Syncs (which contain Notes)
            if let crate::ngl::interpret::ObjData::Sync(sync) = &obj.data {
                // Convert Sync's timestamp (relative to measure) to absolute PDUR time
                let sync_start_time = abs_time + (sync.time_stamp as u32);

                // Process each Note/Rest subobject in this Sync
                if let Some(notes) = score.notes.get(&obj.header.first_sub_obj) {
                    for note in notes {
                        // Skip rests
                        if note.rest {
                            continue;
                        }

                        // Determine staff and channel for this note
                        let staff_num = note.header.staff_num as i32;
                        let part_idx = match staff_to_part.get(&staff_num) {
                            Some(&idx) => idx,
                            None => continue,
                        };
                        let channel = score.part_infos[part_idx].channel;

                        // Calculate absolute MIDI note from pitch + accidental + transposition
                        let transpose = score.part_infos[part_idx].transpose as i8;
                        let midi_note_result = yqpit_to_midi_note(
                            note.yqpit,
                            note.accident as i8,
                            ClefType::Treble as i8, // TODO: Use actual clef per staff
                        );

                        if let Some(mut midi_note) = midi_note_result {
                            // Apply transposition (in semitones)
                            midi_note = ((midi_note as i32) + (transpose as i32))
                                .max(0)
                                .min(127) as u8;

                            // Calculate MIDI velocity
                            let velocity = calculate_velocity(
                                note.on_velocity,
                                0, // TODO: Extract document velocity offset
                                Some(score.part_infos[part_idx].part_velocity as u8),
                            );

                            // Emit NoteOn at play_time_delta offset from sync start
                            let note_on_time = sync_start_time
                                + ((note.play_time_delta as i32).max(0) as u32);

                            self.timed_events.push(TimedEvent {
                                time: note_on_time,
                                event: MidiEvent::NoteOn {
                                    channel,
                                    note: midi_note,
                                    velocity,
                                },
                            });

                            // Emit NoteOff at play_dur after NoteOn
                            let note_off_time =
                                note_on_time + (note.play_dur as u32).max(1);

                            self.timed_events.push(TimedEvent {
                                time: note_off_time,
                                event: MidiEvent::NoteOff {
                                    channel,
                                    note: midi_note,
                                },
                            });
                        }
                    }
                }

                // Update absolute time for next section
                abs_time = sync_start_time;
            }
        }

        // Sort all events by time for correct playback order
        self.timed_events.sort_by_key(|ev| ev.time);
    }

    /// Serialize to Standard MIDI File bytes
    ///
    /// Returns SMF Format 1 (multiple tracks):
    /// - Header chunk (6 bytes ID + 8 bytes size + 14 bytes data)
    /// - Track chunks (one per channel)
    ///
    /// Reference: https://www.midi.org/specifications/file-format-specifications/standard-midi-file-smf
    pub fn to_smf(&self) -> Vec<u8> {
        let mut output = Vec::new();

        // Build track data by grouping events by channel
        let mut tracks: BTreeMap<u8, Vec<(u32, &TimedEvent)>> = BTreeMap::new();
        for event in &self.timed_events {
            let channel = match &event.event {
                MidiEvent::NoteOn { channel, .. }
                | MidiEvent::NoteOff { channel, .. }
                | MidiEvent::ProgramChange { channel, .. }
                | MidiEvent::BankSelect { channel, .. } => *channel,
                // Meta/tempo events go on track 0
                MidiEvent::SetTempo { .. } | MidiEvent::TimeSignature { .. } => 0,
            };
            tracks.entry(channel).or_insert_with(Vec::new).push((event.time, event));
        }

        // Ensure track 0 exists (for global events)
        if !tracks.contains_key(&0) {
            tracks.insert(0, Vec::new());
        }

        let num_tracks = tracks.len() as u16;

        // Write MThd header
        output.extend_from_slice(b"MThd");
        output.extend_from_slice(&write_be32(6)); // Header length (always 6)
        output.extend_from_slice(&write_be16(1)); // Format type 1 (multiple tracks)
        output.extend_from_slice(&write_be16(num_tracks)); // Number of tracks
        output.extend_from_slice(&write_be16(DFLT_BEATDUR as u16)); // Division (480 ticks per quarter)

        // Write MTrk chunks for each track
        for (channel, track_events) in &tracks {
            let mut track_data = Vec::new();
            let mut last_time = 0u32;

            for (time, event) in track_events {
                // Encode delta time (relative to last event)
                let delta = *time - last_time;
                track_data.extend(encode_vlq(delta));
                last_time = *time;

                // Encode MIDI event
                match &event.event {
                    MidiEvent::NoteOn {
                        channel: ch,
                        note,
                        velocity,
                    } => {
                        track_data.push(0x90 | (ch & 0x0F)); // Note-on status
                        track_data.push(*note);
                        track_data.push(*velocity);
                    }
                    MidiEvent::NoteOff {
                        channel: ch,
                        note,
                    } => {
                        track_data.push(0x80 | (ch & 0x0F)); // Note-off status
                        track_data.push(*note);
                        track_data.push(0); // Velocity (always 0 for note-off)
                    }
                    MidiEvent::ProgramChange { channel: ch, program } => {
                        track_data.push(0xC0 | (ch & 0x0F)); // Program change status
                        track_data.push(*program);
                    }
                    MidiEvent::BankSelect { channel: ch, bank } => {
                        // Bank select uses CC 0: BnH 00H vvH
                        track_data.push(0xB0 | (ch & 0x0F)); // Control change status
                        track_data.push(0x00); // Bank select CC
                        track_data.push(*bank);
                    }
                    MidiEvent::SetTempo { tempo_us } => {
                        // Meta event: FF 51 03 [tempo in microseconds, big-endian]
                        track_data.push(0xFF);
                        track_data.push(0x51); // Set tempo
                        track_data.push(0x03); // Length (3 bytes)
                        track_data.push(((*tempo_us >> 16) & 0xFF) as u8);
                        track_data.push(((*tempo_us >> 8) & 0xFF) as u8);
                        track_data.push((*tempo_us & 0xFF) as u8);
                    }
                    MidiEvent::TimeSignature {
                        numerator,
                        denominator,
                        clocks_per_quarter,
                    } => {
                        // Meta event: FF 58 04 nn dd cc bb
                        track_data.push(0xFF);
                        track_data.push(0x58); // Time signature
                        track_data.push(0x04); // Length (4 bytes)
                        track_data.push(*numerator);
                        // Denominator as power of 2 (4 = quarter note)
                        let denom_exp = (*denominator as u32).trailing_zeros() as u8;
                        track_data.push(denom_exp);
                        track_data.push(*clocks_per_quarter); // Clocks per quarter note
                        track_data.push(8); // Eighth notes per quarter (standard)
                    }
                }
            }

            // Add End of Track meta event
            track_data.push(0x00); // Delta time = 0
            track_data.push(0xFF);
            track_data.push(0x2F); // End of track
            track_data.push(0x00); // Length = 0

            // Write MTrk header and data
            output.extend_from_slice(b"MTrk");
            output.extend_from_slice(&write_be32(track_data.len() as u32));
            output.extend_from_slice(&track_data);
        }

        output
    }
}

impl Default for MidiExporter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Export a score to Standard MIDI File
pub fn export_to_midi(score: &InterpretedScore) -> Vec<u8> {
    let mut exporter = MidiExporter::new();
    exporter.export(score);
    exporter.to_smf()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2p_durs_table() {
        let table = get_l2p_durs();
        assert_eq!(table[1], 3840); // breve
        assert_eq!(table[2], 1920); // whole
        assert_eq!(table[3], 960);  // half
        assert_eq!(table[4], 480);  // quarter
        assert_eq!(table[5], 240);  // eighth
        assert_eq!(table[6], 120);  // sixteenth
        assert_eq!(table[7], 60);   // 32nd
        assert_eq!(table[8], 30);   // 64th
        assert_eq!(table[9], 15);   // 128th
    }

    #[test]
    fn test_code_to_ldur_simple() {
        // Quarter note, no dots
        assert_eq!(code_to_ldur(4, 0), 480);
        // Eighth note, no dots
        assert_eq!(code_to_ldur(5, 0), 240);
        // Half note, no dots
        assert_eq!(code_to_ldur(3, 0), 960);
    }

    #[test]
    fn test_code_to_ldur_dotted() {
        // Dotted quarter: 480 + 240 = 720
        assert_eq!(code_to_ldur(4, 1), 720);
        // Dotted half: 960 + 480 = 1440
        assert_eq!(code_to_ldur(3, 1), 1440);
        // Double-dotted quarter: 480 + 240 + 120 = 840
        assert_eq!(code_to_ldur(4, 2), 840);
    }

    #[test]
    fn test_code_to_ldur_edge_cases() {
        // Code 0 should clamp to 1 (breve)
        assert_eq!(code_to_ldur(0, 0), 3840);
        // Code 10 should clamp to 9 (128th)
        assert_eq!(code_to_ldur(10, 0), 15);
        // Dots beyond valid range should not panic
        let result = code_to_ldur(9, 5);
        assert!(result > 0);
    }

    #[test]
    fn test_midi_exporter_new() {
        let exporter = MidiExporter::new();
        assert_eq!(exporter.default_tempo, 120);
        assert!(exporter.tracks.is_empty());
    }

    #[test]
    fn test_midi_exporter_set_tempo() {
        let mut exporter = MidiExporter::new();
        exporter.set_tempo(140);
        assert_eq!(exporter.default_tempo, 140);
    }

    #[test]
    fn test_yqpit_to_midi_note_middle_c() {
        // Middle C on treble clef: yqpit=0 (Middle C reference), treble offset=10
        // yqpit → 0 * 2 = 0 half-lines
        // 0 + 10 (treble offset) = 10 half-lines from C0 = MIDI 60 (Middle C)
        let note = yqpit_to_midi_note(0, 0, 10);
        assert_eq!(note, Some(60)); // Middle C
    }

    #[test]
    fn test_yqpit_to_midi_note_treble_clef() {
        // Treble clef (offset 10 half-lines from Middle C)
        // E above Middle C: yqpit would place E naturally
        // In treble: E = 4 half-lines above Middle C, so yqpit needs -4 to get to 0 + 10 + 4 = 14
        // yqpit=-4 * 2 = -8 half-lines, -8 + 10 = 2 half-lines = D (MIDI 62)

        // Test various notes on treble clef
        // G (top line of treble staff): 8 half-lines above C0
        let g_treble = yqpit_to_midi_note(-1, 0, 10);
        assert!(g_treble.is_some() && g_treble.unwrap() == 67); // G = MIDI 67
    }

    #[test]
    fn test_yqpit_to_midi_note_with_accidental() {
        // Middle C sharp
        let c_sharp = yqpit_to_midi_note(0, 1, 10);
        assert_eq!(c_sharp, Some(61));

        // Middle C flat
        let c_flat = yqpit_to_midi_note(0, -1, 10);
        assert_eq!(c_flat, Some(59));

        // Middle C double sharp
        let c_double_sharp = yqpit_to_midi_note(0, 2, 10);
        assert_eq!(c_double_sharp, Some(62));
    }

    #[test]
    fn test_yqpit_to_midi_note_bass_clef() {
        // Bass clef (offset -2 half-lines from Middle C)
        // Middle C in bass: yqpit=0, clef_offset=-2
        // 0 * 2 + (-2) = -2 half-lines = Bb (MIDI 58)
        let bass_c = yqpit_to_midi_note(0, 0, -2);
        assert_eq!(bass_c, Some(58));
    }

    #[test]
    fn test_yqpit_to_midi_note_out_of_range() {
        // Very high note that exceeds MIDI 127
        let out_of_range = yqpit_to_midi_note(200, 0, 10);
        assert_eq!(out_of_range, None);

        // Very low note that goes below MIDI 0
        let too_low = yqpit_to_midi_note(-200, 0, 10);
        assert_eq!(too_low, None);
    }

    #[test]
    fn test_calculate_velocity_simple() {
        // Note velocity 100, no offsets
        let vel = calculate_velocity(100, 0, None);
        assert_eq!(vel, 100);
    }

    #[test]
    fn test_calculate_velocity_with_document_offset() {
        // Note velocity 100, document offset +20
        let vel = calculate_velocity(100, 20, None);
        assert_eq!(vel, 120);

        // Note velocity 100, document offset -30
        let vel = calculate_velocity(100, -30, None);
        assert_eq!(vel, 70);
    }

    #[test]
    fn test_calculate_velocity_with_part_velocity() {
        // Note velocity 100, no doc offset, part velocity 15
        let vel = calculate_velocity(100, 0, Some(15));
        assert_eq!(vel, 115);

        // Note velocity 100, doc offset +10, part velocity +15
        let vel = calculate_velocity(100, 10, Some(15));
        assert_eq!(vel, 125);
    }

    #[test]
    fn test_calculate_velocity_clamping() {
        // Velocity too high should clamp to 127
        let vel = calculate_velocity(127, 50, Some(50));
        assert_eq!(vel, 127);

        // Velocity too low should clamp to 1 (not 0, which is note-off)
        let vel = calculate_velocity(5, -100, None);
        assert_eq!(vel, 1);

        // Zero velocity should clamp to 1
        let vel = calculate_velocity(0, 0, None);
        assert_eq!(vel, 1);
    }
}
