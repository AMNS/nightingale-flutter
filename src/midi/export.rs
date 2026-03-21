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
#[allow(dead_code)]
const PDURUNIT: u32 = 15;

/// Lookup table: duration code (1-9) -> PDUR ticks
/// Generated at runtime by OG InitNightingale.cp lines 222-224:
///
/// ```text
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
// Tempo Conversion
// =============================================================================

/// Convert BPM (beats per minute) to microseconds per quarter note
///
/// # Arguments
/// * `bpm` - Tempo in beats per minute
///
/// # Returns
/// Microseconds per quarter note for MIDI SetTempo meta-event
///
/// # Formula
/// microseconds_per_quarter = 60,000,000 / bpm
///
/// # Examples
/// - 120 BPM → 500,000 μs/quarter
/// - 60 BPM → 1,000,000 μs/quarter
/// - 240 BPM → 250,000 μs/quarter
///
/// Reference: SMF specification, Meta-Event 0x51 (Set Tempo)
fn bpm_to_microseconds(bpm: u32) -> u32 {
    if bpm == 0 {
        return 500000; // Default to 120 BPM if invalid
    }
    60_000_000 / bpm
}

// =============================================================================
// Key Signature Conversion
// =============================================================================

/// Convert KsInfo to MIDI key signature format (sharps/flats count)
///
/// # Arguments
/// * `ks_info` - Key signature information from KeySig subobject
///
/// # Returns
/// Sharps/flats count: -7 to +7 (negative = flats, positive = sharps)
///
/// # Algorithm
/// - If ks_item[0].sharp == 1: positive count = sharps
/// - If ks_item[0].sharp == 0: negative count = flats
/// - n_ks_items gives the count
///
/// Reference: MIDI specification, Meta-Event 0x59 (Key Signature)
fn ks_info_to_midi_sf(ks_info: &crate::basic_types::KsInfo) -> i8 {
    if ks_info.n_ks_items == 0 {
        return 0; // C major / A minor (no sharps or flats)
    }

    // Check if first item is sharp or flat
    let is_sharp = ks_info.ks_item[0].sharp != 0;

    if is_sharp {
        ks_info.n_ks_items // Positive for sharps
    } else {
        -ks_info.n_ks_items // Negative for flats
    }
}

// =============================================================================
// Dynamics to Velocity Mapping
// =============================================================================

/// Dynamic mark to MIDI velocity lookup table
///
/// Indices 1-23 correspond to DynamicType enum values:
/// - 1-21: Text dynamics (pppp, ppp, pp, p, mp, mf, f, ff, fff, ffff, sf, etc.)
/// - 22-23: Hairpins (dim, cresc) - not used for velocity lookup
///
/// Values based on standard MIDI velocity conventions for musical dynamics.
/// Reference: OG InitNightingale.cp:241 (dynam2velo table initialization)
fn get_dynam2velo() -> [u8; 24] {
    [
        64,  // Index 0: unused
        20,  // 1:  pppp
        32,  // 2:  ppp
        45,  // 3:  pp
        64,  // 4:  p
        80,  // 5:  mp
        96,  // 6:  mf (mezzoforte - default moderate level)
        112, // 7:  f
        120, // 8:  ff
        126, // 9:  fff
        127, // 10: ffff
        90,  // 11: più (somewhat louder - FIRSTREL_DYNAM)
        70,  // 12: meno (somewhat softer)
        72,  // 13: meno (softer variant)
        92,  // 14: più (louder variant)
        115, // 15: sf (sforzando - FIRSTSF_DYNAM)
        116, // 16: fz (forzando)
        118, // 17: sfz (sforzando)
        117, // 18: rf (rinforzando)
        119, // 19: rfz (rinforzando)
        100, // 20: fp (forte-piano)
        110, // 21: sfp (sforzando-piano)
        64,  // 22: dim hairpin (not used for velocity)
        64,  // 23: cresc hairpin (not used for velocity)
    ]
}

/// Convert dynamic mark type to MIDI velocity
///
/// # Arguments
/// * `dynamic_type` - Dynamic type code (1-23 from DynamicType enum)
///
/// # Returns
/// MIDI velocity (1-127), or default (mf = 96) for hairpins or invalid types
///
/// # Reference
/// - OG InitNightingale.cp:241-242 (dynam2velo table)
/// - OG Context.cp:905 (newVelocity = dynam2velo[newDynamic])
/// - OG Objects.cp:847 (aNote->onVelocity = dynam2velo[context.dynamicType])
pub fn dynamic_to_velocity(dynamic_type: i8) -> u8 {
    let dynam2velo = get_dynam2velo();

    // Hairpins (22-23) and invalid types default to mf (96)
    if !(1..22).contains(&dynamic_type) {
        return 96; // mf default
    }

    dynam2velo[dynamic_type as usize]
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
    let code = dur_code.clamp(1, 9) as usize;

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
    NoteOn { channel: u8, note: u8, velocity: u8 },
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
    /// Key signature: (sharps_flats, is_minor)
    /// sharps_flats: -7 to +7 (negative = flats, positive = sharps)
    /// is_minor: 0 = major, 1 = minor
    KeySignature { sharps_flats: i8, is_minor: u8 },
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
#[allow(dead_code)]
const C_MIDI_NOTES: [u8; 10] = [12, 24, 36, 48, 60, 72, 84, 96, 108, 120];

/// Clef offsets in half-lines from Middle C
/// Determines the "Middle C half-line" position for each clef
/// Reference: OG Utilities/PitchUtils.cp lines 204-226 (Pitch2MIDI)
#[derive(Debug, Clone, Copy)]
pub enum ClefType {
    Treble = 10, // G clef: Middle C is 10 half-lines below staff
    Bass = -2,   // F clef: Middle C is 2 half-lines below staff
    Alto = 4,    // C clef (C4): Middle C is 4 half-lines above middle
    Tenor = 6,   // C clef (C3): Middle C is 6 half-lines above middle
}

// Note: ClefType offsets are NOT used for MIDI pitch conversion.
// yqpit is already in quarter-lines relative to middle C (clef-independent).
// No offset_half_lines() method is needed.

/// Convert yqpit (clef-independent quarter-line pitch) to MIDI note number
///
/// # Arguments
/// * `yqpit` - Clef-independent pitch in quarter-line units (ShortQd = i8)
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
pub fn yqpit_to_midi_note(yqpit: i8, accidental: i8) -> Option<u8> {
    // OG: PitchUtils.cp lines 206-226 (Pitch2MIDI function)
    // CRITICAL: yqpit is in QUARTER-LINES, must divide by 2 to get half-lines
    // OG uses qd2halfLn macro: #define qd2halfLn(qd) ((qd)/(STD_LINEHT/4))
    // where STD_LINEHT=8, so qd2halfLn(qd) = qd/2

    // yqpit is ALREADY clef-independent (relative to middle C)
    // Clef offsets are only used when drawing on staff, not for MIDI conversion

    const HALFLINE_TO_SEMI: [i32; 7] = [
        0,  // C
        2,  // D (2 semitones above C)
        4,  // E (4 semitones above C)
        5,  // F (5 semitones above C - only 1 semitone from E)
        7,  // G (7 semitones above C)
        9,  // A (9 semitones above C)
        11, // B (11 semitones above C)
    ];

    // Convert yqpit (quarter-lines) to half-lines: qd2halfLn macro = yqpit / 2
    let half_lines = (yqpit as i32) / 2;

    // Extract letter name (C=0, D=1, ..., B=6) and octave
    let mut letter_name = half_lines % 7;
    if letter_name < 0 {
        letter_name += 7; // Fix negative modulo
    }

    let mut octave = half_lines / 7;
    if half_lines < 0 && letter_name != 0 {
        octave -= 1; // Truncate down for negative values
    }

    // Map letter to semitone offset within octave, then add accidental
    let mut semitones = HALFLINE_TO_SEMI[letter_name as usize];

    // Accidental encoding: OG uses enum AC_DBLFLAT=1, AC_FLAT=2, AC_NATURAL=3, AC_SHARP=4, AC_DBLSHARP=5
    // Formula: if (acc!=0) halfSteps += acc-AC_NATURAL (where AC_NATURAL=3)
    // So: 0=none (treat as natural), 1=-2 (dbl flat), 2=-1 (flat), 3=0 (natural), 4=+1 (sharp), 5=+2 (dbl sharp)
    let ac_offset = if accidental == 0 {
        0
    } else {
        accidental as i32 - 3
    };
    semitones += ac_offset;

    // MIDI note = Middle C (60) + (octave * 12) + semitones
    let midi_note = 60 + (12 * octave + semitones);

    // Validate MIDI range
    if !(0..=127).contains(&midi_note) {
        return None;
    }

    Some(midi_note as u8)
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
pub fn calculate_velocity(
    on_velocity: u8,
    doc_velocity_offset: i8,
    part_velocity: Option<u8>,
) -> u8 {
    let mut velocity = (on_velocity as i32) + (doc_velocity_offset as i32);

    if let Some(pv) = part_velocity {
        velocity += pv as i32;
    }

    // Clamp to valid MIDI velocity range [1, 127]
    // (0 is used for note-off events, so minimum playback velocity is 1)
    velocity.clamp(1, 127) as u8
}

// =============================================================================
// MIDI Exporter
// =============================================================================

/// MIDI export engine
#[derive(Debug)]
pub struct MidiExporter {
    /// All timed events (sorted after export() completes)
    timed_events: Vec<TimedEvent>,
    /// Default tempo in BPM
    default_tempo: u32,
}

impl MidiExporter {
    /// Create a new MIDI exporter
    pub fn new() -> Self {
        MidiExporter {
            timed_events: Vec::new(),
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

        // Build mapping from staff_num to current clef (updated as score is walked)
        // Default to treble clef for all staves, then update from score objects
        let mut staff_to_clef: BTreeMap<i32, ClefType> = BTreeMap::new();

        // Build mapping from staff_num to current dynamic (updated as score is walked)
        // Default to mf (mezzo-forte) for all staves, then update from Dynamic objects
        let mut staff_to_dynamic: BTreeMap<i32, i8> = BTreeMap::new();

        // Emit initial SetTempo event (default 120 BPM = 500000 microseconds per quarter)
        // Will be overridden if score contains Tempo objects
        let initial_tempo_us = bpm_to_microseconds(self.default_tempo);
        self.timed_events.push(TimedEvent {
            time: 0,
            event: MidiEvent::SetTempo {
                tempo_us: initial_tempo_us,
            },
        });

        // Emit ProgramChange + BankSelect for each part/channel at score start
        for part_info in score.part_infos.iter() {
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
                event: MidiEvent::ProgramChange { channel, program },
            });
        }

        // Walk score to extract note events
        // Track current measure's accumulated time from score start
        // OG: MIDIFSave.cp lines 888-891 uses MeasureTIME(measL) + SyncTIME(pL)
        let mut current_measure_time: u32 = 0;

        for obj in score.walk() {
            // Update measure time when we encounter Measure objects
            // Each measure's l_time_stamp is the cumulative time from the start of the score
            if let crate::ngl::interpret::ObjData::Measure(measure) = &obj.data {
                current_measure_time = measure.l_time_stamp as u32;
            }

            // Update clef tracking when we encounter Clef objects
            if let crate::ngl::interpret::ObjData::Clef(_clef) = &obj.data {
                // Update staff_to_clef mapping from clef subobjects
                if let Some(clefs) = score.clefs.get(&obj.header.first_sub_obj) {
                    for clef_sub in clefs {
                        let staff_num = clef_sub.header.staffn as i32;
                        let clef_type = clef_sub.header.sub_type as u8;

                        // Map clef_type to ClefType enum
                        let clef = match clef_type {
                            2 => ClefType::Treble, // TREBLE_CLEF = 2
                            3 => ClefType::Bass,   // BASS_CLEF = 3
                            4 => ClefType::Alto,   // ALTO_CLEF = 4
                            5 => ClefType::Tenor,  // TENOR_CLEF = 5
                            _ => ClefType::Treble, // Default to treble for unknown types
                        };

                        staff_to_clef.insert(staff_num, clef);
                    }
                }
            }

            // Emit tempo change events when we encounter Tempo objects
            if let crate::ngl::interpret::ObjData::Tempo(tempo) = &obj.data {
                // Only emit tempo change if no_mm flag is false (meaning: DO use this tempo)
                // Reference: OG obj_types.rs line 1098: no_mm = False means play at tempo_mm BPM
                if !tempo.no_mm && tempo.tempo_mm > 0 {
                    let new_bpm = tempo.tempo_mm as u32;
                    let tempo_us = bpm_to_microseconds(new_bpm);

                    // Emit SetTempo event at current measure time
                    self.timed_events.push(TimedEvent {
                        time: current_measure_time,
                        event: MidiEvent::SetTempo { tempo_us },
                    });
                }
            }

            // Emit time signature events when we encounter TimeSig objects
            if let crate::ngl::interpret::ObjData::TimeSig(_timesig) = &obj.data {
                // Get ATimeSig subobjects for this TimeSig
                if let Some(timesigs) = score.timesigs.get(&obj.header.first_sub_obj) {
                    // Use first timesig subobject (typically only one per TimeSig object)
                    if let Some(ts) = timesigs.first() {
                        // MIDI denominator is encoded as power of 2:
                        // 2 (half note) = 1, 4 (quarter) = 2, 8 (eighth) = 3, etc.
                        let midi_denominator = match ts.denominator {
                            1 => 0,  // whole note (2^0 = 1)
                            2 => 1,  // half note (2^1 = 2)
                            4 => 2,  // quarter note (2^2 = 4)
                            8 => 3,  // eighth note (2^3 = 8)
                            16 => 4, // sixteenth note (2^4 = 16)
                            32 => 5, // thirty-second note (2^5 = 32)
                            _ => {
                                // For invalid values, default to quarter note
                                eprintln!(
                                    "Warning: invalid time signature denominator {}, defaulting to 4",
                                    ts.denominator
                                );
                                2
                            }
                        };

                        self.timed_events.push(TimedEvent {
                            time: current_measure_time,
                            event: MidiEvent::TimeSignature {
                                numerator: ts.numerator as u8,
                                denominator: midi_denominator,
                                clocks_per_quarter: 24, // MIDI standard: 24 MIDI clocks per quarter note
                            },
                        });
                    }
                }
            }

            // Emit key signature events when we encounter KeySig objects
            if let crate::ngl::interpret::ObjData::KeySig(_keysig) = &obj.data {
                // Get AKeySig subobjects for this KeySig
                if let Some(keysigs) = score.keysigs.get(&obj.header.first_sub_obj) {
                    // Use first keysig subobject (typically only one per KeySig object)
                    if let Some(ks) = keysigs.first() {
                        let sharps_flats = ks_info_to_midi_sf(&ks.ks_info);
                        self.timed_events.push(TimedEvent {
                            time: current_measure_time,
                            event: MidiEvent::KeySignature {
                                sharps_flats,
                                is_minor: 0, // Assume major (no way to determine from KsInfo alone)
                            },
                        });
                    }
                }
            }

            // Update dynamic tracking when we encounter Dynamic objects
            // Dynamic marks (pp, mf, ff, etc.) affect velocity of subsequent notes
            if let crate::ngl::interpret::ObjData::Dynamic(dynamic) = &obj.data {
                // Dynamic objects apply to all staves they appear on
                // Need to determine which staff(s) this dynamic affects
                // Reference: OG Context.cp:905 (newVelocity = dynam2velo[newDynamic])

                // Dynamics are attached to specific staves via first_sync_l
                // For simplicity, we'll update all staves in the current part
                // (More precise would be to track which staff the dynamic is visually on)
                for (_staff_num, dynamic_val) in staff_to_dynamic.iter_mut() {
                    *dynamic_val = dynamic.dynamic_type;
                }
            }

            // Only process Syncs (which contain Notes)
            if let crate::ngl::interpret::ObjData::Sync(sync) = &obj.data {
                // Convert Sync's timestamp (relative to measure) to absolute PDUR time
                // OG: plStartTime = MeasureTIME(measL) + SyncTIME(pL)
                let sync_start_time = current_measure_time + (sync.time_stamp as u32);

                // Process each Note/Rest subobject in this Sync
                if let Some(notes) = score.notes.get(&obj.header.first_sub_obj) {
                    for note in notes {
                        // Skip rests
                        if note.rest {
                            continue;
                        }

                        // Determine staff and channel for this note
                        let staff_num = note.header.staffn as i32;
                        let part_idx = match staff_to_part.get(&staff_num) {
                            Some(&idx) => idx,
                            None => continue,
                        };

                        // Use staff number as MIDI channel (staff 1 → channel 0, etc.)
                        // MIDI channels 0-15, clamp staff numbers to this range
                        let channel = ((staff_num - 1).max(0) as u8).min(15);

                        // Use note_num directly - it already contains the MIDI note number
                        // Apply transposition (in semitones)
                        let transpose = score.part_infos[part_idx].transpose;
                        let midi_note =
                            ((note.note_num as i32) + (transpose as i32)).clamp(0, 127) as u8;

                        // Calculate MIDI velocity from current dynamic + part velocity
                        // Get current dynamic for this staff (default to mf = 6 if not found)
                        let dynamic_type = staff_to_dynamic.get(&staff_num).copied().unwrap_or(6);
                        let base_velocity = dynamic_to_velocity(dynamic_type);

                        // Apply part velocity offset
                        let velocity = calculate_velocity(
                            base_velocity,
                            0, // Document velocity offset (not stored in InterpretedScore)
                            Some(score.part_infos[part_idx].part_velocity as u8),
                        );

                        // Emit NoteOn at play_time_delta offset from sync start
                        let note_on_time =
                            sync_start_time + ((note.play_time_delta as i32).max(0) as u32);

                        self.timed_events.push(TimedEvent {
                            time: note_on_time,
                            event: MidiEvent::NoteOn {
                                channel,
                                note: midi_note,
                                velocity,
                            },
                        });

                        // Emit NoteOff at play_dur after NoteOn
                        let note_off_time = note_on_time + (note.play_dur as u32).max(1);

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
        }

        // Sort all events by time for correct playback order
        self.timed_events.sort_by_key(|ev| ev.time);

        // NOTE: We do NOT normalize/shift timing to remove pickup measures.
        // Notation software (Finale, Sibelius, etc.) needs the original timing
        // with pickup offsets preserved to correctly display anacrusis/pickup bars.
        // The "intro silence" is actually the musical space before the pickup notes,
        // which is essential for proper measure display.
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

        // Build track data by grouping events by track number
        // Track 0 = meta events only (tempo, time sig, key sig)
        // Tracks 1-N = channel events (notes, program changes, etc.) grouped by channel
        let mut tracks: BTreeMap<u8, Vec<(u32, &TimedEvent)>> = BTreeMap::new();
        for event in &self.timed_events {
            let track_num = match &event.event {
                MidiEvent::NoteOn { channel, .. }
                | MidiEvent::NoteOff { channel, .. }
                | MidiEvent::ProgramChange { channel, .. }
                | MidiEvent::BankSelect { channel, .. } => channel + 1, // Channels 0-15 → tracks 1-16
                // Meta events go on track 0
                MidiEvent::SetTempo { .. }
                | MidiEvent::TimeSignature { .. }
                | MidiEvent::KeySignature { .. } => 0,
            };
            tracks
                .entry(track_num)
                .or_default()
                .push((event.time, event));
        }

        // Ensure track 0 exists (for meta events)
        tracks.entry(0).or_default();

        let num_tracks = tracks.len() as u16;

        // Write MThd header
        output.extend_from_slice(b"MThd");
        output.extend_from_slice(&write_be32(6)); // Header length (always 6)
        output.extend_from_slice(&write_be16(1)); // Format type 1 (multiple tracks)
        output.extend_from_slice(&write_be16(num_tracks)); // Number of tracks
        output.extend_from_slice(&write_be16(DFLT_BEATDUR as u16)); // Division (480 ticks per quarter)

        // Write MTrk chunks for each track
        for track_events in tracks.values() {
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
                    MidiEvent::NoteOff { channel: ch, note } => {
                        track_data.push(0x80 | (ch & 0x0F)); // Note-off status
                        track_data.push(*note);
                        track_data.push(0); // Velocity (always 0 for note-off)
                    }
                    MidiEvent::ProgramChange {
                        channel: ch,
                        program,
                    } => {
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
                        // Denominator is already encoded as log2 exponent
                        track_data.push(*denominator);
                        track_data.push(*clocks_per_quarter); // Clocks per quarter note
                        track_data.push(8); // Eighth notes per quarter (standard)
                    }
                    MidiEvent::KeySignature {
                        sharps_flats,
                        is_minor,
                    } => {
                        // Meta event: FF 59 02 sf mi
                        track_data.push(0xFF);
                        track_data.push(0x59); // Key signature
                        track_data.push(0x02); // Length (2 bytes)
                        track_data.push(*sharps_flats as u8); // Sharps/flats count (signed byte)
                        track_data.push(*is_minor); // Major (0) or minor (1)
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
        assert_eq!(table[3], 960); // half
        assert_eq!(table[4], 480); // quarter
        assert_eq!(table[5], 240); // eighth
        assert_eq!(table[6], 120); // sixteenth
        assert_eq!(table[7], 60); // 32nd
        assert_eq!(table[8], 30); // 64th
        assert_eq!(table[9], 15); // 128th
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
        assert!(exporter.timed_events.is_empty());
    }

    #[test]
    fn test_midi_exporter_set_tempo() {
        let mut exporter = MidiExporter::new();
        exporter.set_tempo(140);
        assert_eq!(exporter.default_tempo, 140);
    }

    #[test]
    fn test_bpm_to_microseconds() {
        // 120 BPM is the standard tempo (500,000 μs per quarter)
        assert_eq!(bpm_to_microseconds(120), 500_000);

        // 60 BPM = 1,000,000 μs per quarter (1 second per beat)
        assert_eq!(bpm_to_microseconds(60), 1_000_000);

        // 240 BPM = 250,000 μs per quarter
        assert_eq!(bpm_to_microseconds(240), 250_000);

        // Edge case: 0 BPM should default to 120 BPM
        assert_eq!(bpm_to_microseconds(0), 500_000);

        // Fast tempo: 200 BPM
        assert_eq!(bpm_to_microseconds(200), 300_000);
    }

    #[test]
    fn test_yqpit_to_midi_note_middle_c() {
        // yqpit is in QUARTER-LINES relative to middle C
        // yqpit=0 → 0 quarter-lines from middle C → middle C itself
        // 0 / 2 = 0 half-lines
        // letter_name = 0%7 = 0 (C), octave = 0/7 = 0
        // MIDI = 60 + (12*0) + 0 = 60 (Middle C)
        let middle_c = yqpit_to_midi_note(0, 0);
        assert_eq!(middle_c, Some(60));
    }

    #[test]
    fn test_yqpit_to_midi_note_scale() {
        // C major scale from middle C
        // yqpit values are in quarter-lines; divide by 2 for half-lines (integer division)
        // yqpit=0 → 0/2=0 half-lines → C4 (MIDI 60)
        assert_eq!(yqpit_to_midi_note(0, 0), Some(60));
        // yqpit=2 → 2/2=1 half-line → D4 (MIDI 62)
        assert_eq!(yqpit_to_midi_note(2, 0), Some(62));
        // yqpit=4 → 4/2=2 half-lines → E4 (MIDI 64)
        assert_eq!(yqpit_to_midi_note(4, 0), Some(64));
        // yqpit=6 → 6/2=3 half-lines → F4 (MIDI 65)
        assert_eq!(yqpit_to_midi_note(6, 0), Some(65));
        // yqpit=8 → 8/2=4 half-lines → G4 (MIDI 67)
        assert_eq!(yqpit_to_midi_note(8, 0), Some(67));
        // yqpit=10 → 10/2=5 half-lines → A4 (MIDI 69)
        assert_eq!(yqpit_to_midi_note(10, 0), Some(69));
        // yqpit=12 → 12/2=6 half-lines → B4 (MIDI 71)
        assert_eq!(yqpit_to_midi_note(12, 0), Some(71));
        // yqpit=14 → 14/2=7 half-lines → C5 (MIDI 72)
        assert_eq!(yqpit_to_midi_note(14, 0), Some(72));
    }

    #[test]
    fn test_yqpit_to_midi_note_with_accidental() {
        // Accidental encoding: AC_DBLFLAT=1, AC_FLAT=2, AC_NATURAL=3, AC_SHARP=4, AC_DBLSHARP=5
        // Formula: if acc != 0 then halfSteps += acc - AC_NATURAL (where AC_NATURAL=3)

        // Middle C (yqpit=0) = MIDI 60
        // C# (accidental=4): offset = 4-3 = +1 → MIDI 61
        let c_sharp = yqpit_to_midi_note(0, 4);
        assert_eq!(c_sharp, Some(61));

        // Cb (accidental=2): offset = 2-3 = -1 → MIDI 59
        let c_flat = yqpit_to_midi_note(0, 2);
        assert_eq!(c_flat, Some(59));

        // C double-sharp (accidental=5): offset = 5-3 = +2 → MIDI 62
        let c_double_sharp = yqpit_to_midi_note(0, 5);
        assert_eq!(c_double_sharp, Some(62));

        // C double-flat (accidental=1): offset = 1-3 = -2 → MIDI 58
        let c_double_flat = yqpit_to_midi_note(0, 1);
        assert_eq!(c_double_flat, Some(58));
    }

    #[test]
    fn test_yqpit_to_midi_note_octaves() {
        // Test octave handling (positive and negative yqpit values)

        // C3 (one octave below middle C): yqpit=-14 (7 half-lines down * 2)
        // -14 / 2 = -7 half-lines
        // letter_name = -7%7 = 0 (C), octave = -7/7 = -1
        // MIDI = 60 + (-12) + 0 = 48 (C3)
        assert_eq!(yqpit_to_midi_note(-14, 0), Some(48));

        // C5 (one octave above middle C): yqpit=14
        // 14 / 2 = 7 half-lines
        // letter_name = 7%7 = 0 (C), octave = 7/7 = 1
        // MIDI = 60 + 12 + 0 = 72 (C5)
        assert_eq!(yqpit_to_midi_note(14, 0), Some(72));

        // C2 (two octaves below): yqpit=-28
        assert_eq!(yqpit_to_midi_note(-28, 0), Some(36));
    }

    #[test]
    fn test_yqpit_to_midi_note_out_of_range() {
        // Very high note that exceeds MIDI 127
        // yqpit=127 (max i8) / 2 = 63 half-lines
        // This would be many octaves above middle C, should exceed MIDI 127
        let out_of_range = yqpit_to_midi_note(127, 0);
        assert_eq!(out_of_range, None);

        // Very low note that goes below MIDI 0
        // yqpit=-128 (min i8) / 2 = -64 half-lines
        // This would be many octaves below middle C, should be below MIDI 0
        let too_low = yqpit_to_midi_note(-128, 0);
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

    #[test]
    fn test_dynamic_to_velocity_common_dynamics() {
        // Test common dynamic markings
        assert_eq!(dynamic_to_velocity(1), 20); // pppp
        assert_eq!(dynamic_to_velocity(2), 32); // ppp
        assert_eq!(dynamic_to_velocity(3), 45); // pp
        assert_eq!(dynamic_to_velocity(4), 64); // p
        assert_eq!(dynamic_to_velocity(5), 80); // mp
        assert_eq!(dynamic_to_velocity(6), 96); // mf (default moderate)
        assert_eq!(dynamic_to_velocity(7), 112); // f
        assert_eq!(dynamic_to_velocity(8), 120); // ff
        assert_eq!(dynamic_to_velocity(9), 126); // fff
        assert_eq!(dynamic_to_velocity(10), 127); // ffff (max)
    }

    #[test]
    fn test_dynamic_to_velocity_special_marks() {
        // Sforzando and related marks
        assert_eq!(dynamic_to_velocity(15), 115); // sf
        assert_eq!(dynamic_to_velocity(16), 116); // fz
        assert_eq!(dynamic_to_velocity(17), 118); // sfz
        assert_eq!(dynamic_to_velocity(20), 100); // fp (forte-piano)
    }

    #[test]
    fn test_dynamic_to_velocity_hairpins() {
        // Hairpins should default to mf (96)
        assert_eq!(dynamic_to_velocity(22), 96); // dim hairpin
        assert_eq!(dynamic_to_velocity(23), 96); // cresc hairpin
    }

    #[test]
    fn test_dynamic_to_velocity_invalid() {
        // Invalid dynamic types should default to mf (96)
        assert_eq!(dynamic_to_velocity(0), 96); // Index 0 (unused)
        assert_eq!(dynamic_to_velocity(-1), 96); // Negative
        assert_eq!(dynamic_to_velocity(24), 96); // Beyond range
        assert_eq!(dynamic_to_velocity(127), 96); // Way out of range
    }
}
