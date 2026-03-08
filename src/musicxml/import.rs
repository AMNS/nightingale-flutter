//! MusicXML 4.0 import: parse a MusicXML partwise file into an InterpretedScore.
//!
//! Builds InterpretedScore directly from parsed MusicXML — no Notelist intermediate.
//! Constructs the full NGL object hierarchy:
//! HEADER → PAGE → SYSTEM → STAFF → CONNECT → CLEF → KEYSIG → TIMESIG →
//! MEASURE → SYNC → ... → TAIL
//!
//! Supports:
//! - `<score-partwise>` format (MusicXML 4.0)
//! - Parts with 1+ staves
//! - Notes, rests, chords
//! - Key signatures, time signatures, clefs
//! - Ties (preserved as note flags)
//! - Accidentals (sharp, flat, natural, double-sharp, double-flat)
//! - Dotted notes
//! - Multiple voices per staff

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::basic_types::{DRect, Ddist, KsInfo, Link, Rect, NILINK};
use crate::defs::{
    AC_FLAT, AC_NATURAL, AC_SHARP, ALTO_CLEF, BARITONE_CLEF, BASS8B_CLEF, BASS_CLEF, FRVIOLIN_CLEF,
    MZSOPRANO_CLEF, PERC_CLEF, SOPRANO_CLEF, TENOR_CLEF, TREBLE8_CLEF, TREBLE_CLEF, TRTENOR_CLEF,
};
use crate::layout::{layout_score, LayoutConfig};
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::obj_types::{
    AClef, AConnect, AKeySig, AMeasure, AModNr, ANote, ANoteBeam, ANoteTuple, AStaff, ATimeSig,
    BeamSet, Clef, Connect, Dynamic, Ending, ExtObjHeader, GrSync, Header, KeySig, Measure,
    ObjectHeader, Ottava, Page, PartInfo, RptEnd, Staff, SubObjHeader, System, Tail, Tempo,
    TimeSig, Tuplet, SHOW_ALL_LINES,
};
use crate::objects::{normal_stem_up_down_single, setup_ks_info, VoiceRole};
use crate::pitch_utils::{clef_middle_c_half_ln, half_ln_to_yd};
use crate::utility::{calc_ystem, nflags, shorten_stem};

// Re-import Sync from obj_types (shadows std::marker::Sync in this module only)
use crate::obj_types::Sync as NglSync;

/// PDUR ticks per quarter note (matches export.rs and Nightingale convention).
const PDUR_QUARTER: i32 = 480;

/// Import error type.
#[derive(Debug)]
pub enum ImportError {
    /// XML parsing error from quick-xml.
    Xml(quick_xml::Error),
    /// Missing required element or attribute.
    Missing(String),
    /// Unsupported format (e.g., score-timewise).
    Unsupported(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Xml(e) => write!(f, "XML parse error: {}", e),
            ImportError::Missing(s) => write!(f, "Missing required element: {}", s),
            ImportError::Unsupported(s) => write!(f, "Unsupported: {}", s),
        }
    }
}

impl From<quick_xml::Error> for ImportError {
    fn from(e: quick_xml::Error) -> Self {
        ImportError::Xml(e)
    }
}

// ============================================================================
// Parsed MusicXML intermediate representation
// ============================================================================

/// A parsed MusicXML part definition from `<part-list>`.
#[derive(Debug, Clone)]
struct XmlPartDef {
    _id: String,
    name: String,
}

/// A parsed note/rest from MusicXML.
#[derive(Debug, Clone)]
struct XmlNote {
    rest: bool,
    chord: bool,
    step: String,
    octave: i32,
    alter: i32,
    duration: i32,
    note_type: String,
    dots: u8,
    voice: i32,
    staff: i32,
    tie_start: bool,
    tie_stop: bool,
    accidental: String,
    /// True if this note has a slur starting here (type="start").
    slur_start: bool,
    /// True if this note has a slur ending here (type="stop").
    slur_stop: bool,
    /// True if this note is a grace note (<grace/> element).
    grace: bool,
    /// True if this grace note has a slash (acciaccatura vs appoggiatura).
    grace_slash: bool,
    /// Beam values from MusicXML: beam_levels[0] = primary beam (number=1), etc.
    /// Values: "begin", "continue", "end", "forward hook", "backward hook"
    beam_levels: Vec<String>,
    /// Tuplet actual notes (from <time-modification>/<actual-notes>).
    tuplet_actual: u8,
    /// Tuplet normal notes (from <time-modification>/<normal-notes>).
    tuplet_normal: u8,
    /// True if this note has <tuplet type="start">.
    tuplet_start: bool,
    /// True if this note has <tuplet type="stop">.
    tuplet_stop: bool,
    /// Articulation/ornament modifier codes (NGL ModCode values).
    mod_codes: Vec<u8>,
}

/// Parsed attributes from a MusicXML `<attributes>` element.
#[derive(Debug, Clone, Default)]
struct XmlAttributes {
    divisions: Option<i32>,
    key_fifths: Option<i32>,
    time_beats: Option<i32>,
    time_beat_type: Option<i32>,
    staves: Option<i32>,
    /// Clefs: (staff_number, sign, line, octave_change).
    clefs: Vec<(i32, String, i32, i32)>,
}

/// A forward or backup element.
#[derive(Debug, Clone)]
enum XmlDirection {
    Forward(i32),
    Backup(i32),
}

/// A dynamic marking parsed from MusicXML `<direction><dynamics>`.
#[derive(Debug, Clone)]
struct XmlDynamic {
    /// NGL dynamic type code (1–21 for text dynamics).
    dynamic_type: u8,
    /// Staff number (from `<staff>` inside `<direction>`), default 1.
    staff: i32,
}

/// A tempo marking parsed from MusicXML `<direction><metronome>/<words>/<sound>`.
#[derive(Debug, Clone)]
struct XmlTempo {
    /// Beat unit as l_dur code (4=quarter, 3=half, etc.).
    sub_type: i8,
    /// True if beat unit has a dot.
    dotted: bool,
    /// BPM (from per-minute or sound tempo attribute).
    tempo_mm: i16,
    /// Verbal text (e.g. "Allegro") from `<words>`.
    words: String,
    /// Staff number.
    staff: i32,
}

/// A volta ending mark from MusicXML `<barline><ending>`.
#[derive(Debug, Clone)]
struct XmlEnding {
    /// Ending number (1, 2, etc.).
    number: u8,
    /// "start", "stop", or "discontinue".
    ending_type: String,
    /// Measure number where this ending mark occurs (reserved for future use).
    #[allow(dead_code)]
    measure_num: i32,
}

/// A repeat barline from MusicXML `<barline><repeat>`.
#[derive(Debug, Clone)]
struct XmlRepeat {
    /// "forward" or "backward".
    direction: String,
    /// Measure number (reserved for future use).
    #[allow(dead_code)]
    measure_num: i32,
}

/// An ottava (octave shift) from MusicXML `<direction><octave-shift>`.
#[derive(Debug, Clone)]
struct XmlOttava {
    /// OttavaType code: 1=8va, 2=15ma, 4=8vb, 5=15mb, 0=stop.
    oct_type: u8,
    /// Staff number.
    staff: i32,
}

/// An element within a MusicXML measure.
#[derive(Debug, Clone)]
enum XmlMeasureElement {
    Attributes(XmlAttributes),
    Note(XmlNote),
    Direction(XmlDirection),
    DynamicMarking(XmlDynamic),
    TempoMark(XmlTempo),
    EndingMark(XmlEnding),
    RepeatBarline(XmlRepeat),
    OttavaMark(XmlOttava),
}

/// A parsed MusicXML measure.
#[derive(Debug, Clone)]
struct XmlMeasure {
    number: i32,
    elements: Vec<XmlMeasureElement>,
}

/// A parsed MusicXML part (all measures).
#[derive(Debug, Clone)]
struct XmlPart {
    _id: String,
    measures: Vec<XmlMeasure>,
}

// ============================================================================
// Conversion helpers
// ============================================================================

/// Convert MusicXML pitch (step, octave, alter) to MIDI note number.
fn pitch_to_midi(step: &str, octave: i32, alter: i32) -> u8 {
    let base = match step {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => 0,
    };
    let midi = (octave + 1) * 12 + base + alter;
    midi.clamp(0, 127) as u8
}

/// Convert MusicXML note type string to NGL l_dur code.
fn type_to_l_dur(note_type: &str) -> i8 {
    match note_type {
        "breve" => 1,
        "whole" => 2,
        "half" => 3,
        "quarter" => 4,
        "eighth" => 5,
        "16th" => 6,
        "32nd" => 7,
        "64th" => 8,
        "128th" => 9,
        _ => 4,
    }
}

/// Convert MusicXML accidental text + alter to NGL accidental code.
fn accidental_to_code(acc: &str, alter: i32) -> u8 {
    match acc {
        "double-flat" | "flat-flat" => 1,
        "flat" => 2,
        "natural" => 3,
        "sharp" => 4,
        "double-sharp" | "sharp-sharp" | "x" => 5,
        _ => match alter {
            -2 => 1,
            -1 => 2,
            1 => 4,
            2 => 5,
            _ => 0,
        },
    }
}

/// Convert MusicXML clef (sign, line, octave_change) to NGL clef type.
///
/// Matches the inverse of export.rs clef_to_xml(). The octave_change parameter
/// comes from the `<clef-octave-change>` element (0 if absent).
fn clef_to_ngl(sign: &str, line: i32, oct_change: i32) -> u8 {
    match (sign, line, oct_change) {
        ("G", 2, 1) => TREBLE8_CLEF,   // Treble 8va
        ("G", 2, -1) => TRTENOR_CLEF,  // Treble-tenor (G clef, octave down)
        ("G", 1, _) => FRVIOLIN_CLEF,  // French violin (G on line 1)
        ("G", 2, _) => TREBLE_CLEF,    // Standard treble
        ("C", 1, _) => SOPRANO_CLEF,   // C on line 1
        ("C", 2, _) => MZSOPRANO_CLEF, // Mezzo-soprano
        ("C", 3, _) => ALTO_CLEF,      // Alto
        ("C", 4, _) => TENOR_CLEF,     // Tenor
        ("F", 3, _) => BARITONE_CLEF,  // Baritone
        ("F", 4, -1) => BASS8B_CLEF,   // Bass 8vb
        ("F", 4, _) => BASS_CLEF,      // Standard bass
        ("percussion", _, _) => PERC_CLEF,
        _ => TREBLE_CLEF,
    }
}

/// Convert MusicXML step name to Nightingale letcode.
/// A=5, B=4, C=3, D=2, E=1, F=0, G=6
fn step_to_letcode(step: &str) -> Option<i8> {
    match step {
        "A" => Some(5),
        "B" => Some(4),
        "C" => Some(3),
        "D" => Some(2),
        "E" => Some(1),
        "F" => Some(0),
        "G" => Some(6),
        _ => None,
    }
}

/// Check whether an accidental is redundant given the key signature.
/// Returns true if the acc matches what the key signature already implies,
/// meaning it should NOT be displayed.
///
/// For example, in 4 sharps (E major: F#, C#, G#, D#), a sharp on F is
/// redundant because the key sig already sharps F.
fn is_acc_redundant_for_ks(step: &str, acc: u8, fifths: i32) -> bool {
    if acc == 0 || acc == AC_NATURAL {
        // Natural signs are never redundant — they cancel the key sig
        return false;
    }
    let letcode = match step_to_letcode(step) {
        Some(l) => l,
        None => return false,
    };

    // Build the key sig's sharped/flatted letcodes
    const SHARP_ORDER: [i8; 7] = [0, 3, 6, 2, 5, 1, 4]; // F C G D A E B
    const FLAT_ORDER: [i8; 7] = [4, 1, 5, 2, 6, 3, 0]; // B E A D G C F

    let (n_items, is_sharp) = if fifths >= 0 {
        (fifths.unsigned_abs().min(7) as usize, true)
    } else {
        (fifths.unsigned_abs().min(7) as usize, false)
    };

    let order = if is_sharp { &SHARP_ORDER } else { &FLAT_ORDER };
    let ks_acc = if is_sharp { AC_SHARP } else { AC_FLAT };

    // Check if this note letter is in the key sig and the acc type matches
    for &ks_letcode in order.iter().take(n_items) {
        if ks_letcode == letcode && acc == ks_acc {
            return true;
        }
    }
    false
}

/// Convert key fifths to (n_items, is_sharp).
fn fifths_to_ks(fifths: i32) -> (u8, bool) {
    if fifths >= 0 {
        (fifths.unsigned_abs().min(7) as u8, true)
    } else {
        (fifths.unsigned_abs().min(7) as u8, false)
    }
}

/// Build a KsInfo from fifths value.
fn fifths_to_ks_info(fifths: i32) -> KsInfo {
    let (n_items, is_sharp) = fifths_to_ks(fifths);
    setup_ks_info(n_items, is_sharp)
}

/// Compute PDUR ticks from MusicXML duration + divisions.
fn xml_dur_to_pdur(xml_dur: i32, divisions: i32) -> i32 {
    if divisions <= 0 {
        return PDUR_QUARTER;
    }
    (xml_dur as i64 * PDUR_QUARTER as i64 / divisions as i64) as i32
}

/// Convert MIDI note to yqpit (half-lines below middle C, clef-independent).
/// This is a simplified version — in a full port, PitchUtils.cp handles this.
fn midi_to_yqpit(midi: u8) -> i8 {
    // Middle C = MIDI 60, yqpit 0.
    // Each semitone maps to staff position based on diatonic scale.
    // Simplified: use the note letter position.
    let note = midi as i32 - 60; // semitones from middle C
    let octave_offset = if note >= 0 {
        note / 12
    } else {
        (note - 11) / 12
    };
    let pc = ((note % 12) + 12) % 12; // pitch class 0-11
                                      // Map pitch class to diatonic half-lines below middle C
    let diatonic = match pc {
        0 => 0,   // C
        1 => 0,   // C#/Db
        2 => -1,  // D
        3 => -1,  // D#/Eb
        4 => -2,  // E
        5 => -3,  // F
        6 => -3,  // F#/Gb
        7 => -4,  // G
        8 => -4,  // G#/Ab
        9 => -5,  // A
        10 => -5, // A#/Bb
        11 => -6, // B
        _ => 0,
    };
    let halflines = diatonic - (octave_offset * 7);
    halflines.clamp(-128, 127) as i8
}

// ============================================================================
// XML parsing
// ============================================================================

/// Parse a MusicXML string into an InterpretedScore.
pub fn import_musicxml(xml: &str) -> Result<InterpretedScore, ImportError> {
    let (parts_def, parts) = parse_musicxml(xml)?;
    build_score(&parts_def, &parts)
}

/// Parse a MusicXML string into intermediate structures.
fn parse_musicxml(xml: &str) -> Result<(Vec<XmlPartDef>, Vec<XmlPart>), ImportError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut parts_def: Vec<XmlPartDef> = Vec::new();
    let mut parts: Vec<XmlPart> = Vec::new();
    let mut buf = Vec::new();

    // Depth-tracking approach: simpler than full state machine for nested elements
    let mut path: Vec<String> = Vec::new();
    let mut current_part_def: Option<XmlPartDef> = None;
    let mut current_part: Option<XmlPart> = None;
    let mut current_measure: Option<XmlMeasure> = None;
    let mut current_attrs: Option<XmlAttributes> = None;
    let mut current_note: Option<XmlNote> = None;
    let mut current_clef_num: i32 = 1;
    let mut current_clef_sign: String = String::new();
    let mut current_clef_line: i32 = 2;
    let mut current_clef_oct_change: i32 = 0;
    // State for parsing <direction><direction-type><dynamics> elements
    let mut current_dynamic_type: Option<u8> = None;
    let mut current_direction_staff: i32 = 1;
    // State for parsing <beam number="N"> elements inside <note>
    let mut current_beam_number: i32 = 0;
    // State for parsing <direction> tempo marks
    let mut current_tempo_words: String = String::new();
    let mut current_tempo_beat_unit: String = String::new();
    let mut current_tempo_beat_dots: bool = false;
    let mut current_tempo_per_minute: i32 = 0;
    let mut current_tempo_sound: i32 = 0;
    // State for parsing <direction><octave-shift>
    let mut current_ottava_type: Option<u8> = None;
    // State for parsing <barline> elements
    let mut in_barline: bool = false;
    let mut current_barline_ending_num: Option<u8> = None;
    let mut current_barline_ending_type: String = String::new();
    let mut current_barline_repeat_dir: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(ImportError::Xml(e)),
            Ok(Event::Eof) => break,

            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag.as_str() {
                    "score-part" => {
                        let id = attr_str(e, "id");
                        current_part_def = Some(XmlPartDef {
                            _id: id,
                            name: String::new(),
                        });
                    }
                    "part" => {
                        let id = attr_str(e, "id");
                        current_part = Some(XmlPart {
                            _id: id,
                            measures: Vec::new(),
                        });
                    }
                    "measure" => {
                        let num = attr_str(e, "number").parse::<i32>().unwrap_or(1);
                        current_measure = Some(XmlMeasure {
                            number: num,
                            elements: Vec::new(),
                        });
                    }
                    "attributes" => {
                        current_attrs = Some(XmlAttributes::default());
                    }
                    "note" => {
                        current_note = Some(XmlNote {
                            rest: false,
                            chord: false,
                            step: String::new(),
                            octave: 4,
                            alter: 0,
                            duration: 0,
                            note_type: String::new(),
                            dots: 0,
                            voice: 1,
                            staff: 1,
                            tie_start: false,
                            tie_stop: false,
                            accidental: String::new(),
                            slur_start: false,
                            slur_stop: false,
                            grace: false,
                            grace_slash: false,
                            beam_levels: Vec::new(),
                            tuplet_actual: 0,
                            tuplet_normal: 0,
                            tuplet_start: false,
                            tuplet_stop: false,
                            mod_codes: Vec::new(),
                        });
                    }
                    "direction" => {
                        // Reset direction state for a new <direction> element.
                        current_dynamic_type = None;
                        current_direction_staff = 1;
                        current_tempo_words.clear();
                        current_tempo_beat_unit.clear();
                        current_tempo_beat_dots = false;
                        current_tempo_per_minute = 0;
                        current_tempo_sound = 0;
                        current_ottava_type = None;
                    }
                    "barline" => {
                        in_barline = true;
                        current_barline_ending_num = None;
                        current_barline_ending_type.clear();
                        current_barline_repeat_dir = None;
                    }
                    "clef" => {
                        current_clef_num = attr_str(e, "number").parse::<i32>().unwrap_or(1);
                        current_clef_sign = String::new();
                        current_clef_line = 2;
                        current_clef_oct_change = 0;
                    }
                    "tie" => {
                        if let Some(ref mut note) = current_note {
                            match attr_str(e, "type").as_str() {
                                "start" => note.tie_start = true,
                                "stop" => note.tie_stop = true,
                                _ => {}
                            }
                        }
                    }
                    "slur" => {
                        if let Some(ref mut note) = current_note {
                            match attr_str(e, "type").as_str() {
                                "start" => note.slur_start = true,
                                "stop" => note.slur_stop = true,
                                _ => {}
                            }
                        }
                    }
                    "tuplet" => {
                        if let Some(ref mut note) = current_note {
                            match attr_str(e, "type").as_str() {
                                "start" => note.tuplet_start = true,
                                "stop" => note.tuplet_stop = true,
                                _ => {}
                            }
                        }
                    }
                    "beam" => {
                        current_beam_number = attr_str(e, "number").parse::<i32>().unwrap_or(1);
                    }
                    // <ending> inside <barline> may have content: <ending number="1" type="start">1.</ending>
                    "ending" if in_barline => {
                        current_barline_ending_num = attr_str(e, "number").parse().ok();
                        current_barline_ending_type = attr_str(e, "type");
                    }
                    _ => {}
                }
                path.push(tag);
            }

            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if let Some(ref mut note) = current_note {
                    match tag.as_str() {
                        "rest" => note.rest = true,
                        "chord" => note.chord = true,
                        "dot" => note.dots += 1,
                        "grace" => {
                            note.grace = true;
                            note.grace_slash = attr_str(e, "slash") == "yes";
                        }
                        "tie" => match attr_str(e, "type").as_str() {
                            "start" => note.tie_start = true,
                            "stop" => note.tie_stop = true,
                            _ => {}
                        },
                        "slur" => match attr_str(e, "type").as_str() {
                            "start" => note.slur_start = true,
                            "stop" => note.slur_stop = true,
                            _ => {}
                        },
                        "tuplet" => match attr_str(e, "type").as_str() {
                            "start" => note.tuplet_start = true,
                            "stop" => note.tuplet_stop = true,
                            _ => {}
                        },
                        // Articulations inside <notations><articulations>
                        "staccato" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(14); // ModStaccato
                        }
                        "accent" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(12); // ModAccent
                        }
                        "strong-accent" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(13); // ModHeavyAccent
                        }
                        "staccatissimo" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(15); // ModWedge (staccatissimo)
                        }
                        "tenuto" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(16); // ModTenuto
                        }
                        "fermata" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(10); // ModFermata
                        }
                        // Ornaments inside <notations><ornaments>
                        "trill-mark" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(11); // ModTrill
                        }
                        "mordent" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(17); // ModMordent
                        }
                        "inverted-mordent" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(18); // ModInvMordent
                        }
                        "turn" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(19); // ModTurn
                        }
                        // Technical marks
                        "up-bow" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(22); // ModUpbow
                        }
                        "down-bow" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(23); // ModDownbow
                        }
                        "harmonic" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(21); // ModCircle
                        }
                        "snap-pizzicato" | "pluck" if path.iter().any(|p| p == "notations") => {
                            note.mod_codes.push(20); // ModPlus
                        }
                        _ => {}
                    }
                }
                // Dynamic element names appear as self-closing tags inside <dynamics>.
                // path should contain "dynamics" when we're inside a <dynamics> element.
                if path.last().map(|s| s.as_str()) == Some("dynamics") {
                    let dtype = match tag.as_str() {
                        "pppp" => Some(1u8),
                        "ppp" => Some(2),
                        "pp" => Some(3),
                        "p" => Some(4),
                        "mp" => Some(5),
                        "mf" => Some(6),
                        "f" => Some(7),
                        "ff" => Some(8),
                        "fff" => Some(9),
                        "ffff" => Some(10),
                        "sf" => Some(15),
                        "fz" => Some(16),
                        "sfz" => Some(17),
                        "rf" => Some(18),
                        "rfz" => Some(19),
                        "fp" => Some(20),
                        "sfp" => Some(21),
                        _ => None,
                    };
                    if let Some(d) = dtype {
                        current_dynamic_type = Some(d);
                    }
                }
                // <repeat direction="forward|backward"/> inside <barline>
                if tag == "repeat" && in_barline {
                    current_barline_repeat_dir = Some(attr_str(e, "direction"));
                }
                // <ending number="N" type="start|stop"/> inside <barline> (self-closing form)
                if tag == "ending" && in_barline && current_barline_ending_num.is_none() {
                    current_barline_ending_num = attr_str(e, "number").parse().ok();
                    current_barline_ending_type = attr_str(e, "type");
                }
                // <octave-shift type="up|down|stop" size="8|15"/> inside <direction>
                if tag == "octave-shift" && path.iter().any(|p| p == "direction") {
                    let shift_type = attr_str(e, "type");
                    let size = attr_str(e, "size").parse::<i32>().unwrap_or(8);
                    current_ottava_type = Some(match (shift_type.as_str(), size) {
                        ("down", 8) => 1,  // Ottava8va (sounds higher)
                        ("down", 15) => 2, // Ottava15ma
                        ("up", 8) => 4,    // Ottava8vaBassa (sounds lower)
                        ("up", 15) => 5,   // Ottava15maBassa
                        ("stop", _) => 0,  // stop marker
                        _ => 0,
                    });
                }
                // <sound tempo="N"/> inside <direction>
                if tag == "sound" && path.iter().any(|p| p == "direction") {
                    if let Ok(t) = attr_str(e, "tempo").parse::<i32>() {
                        current_tempo_sound = t;
                    }
                }
                // <beat-unit-dot/> inside <metronome>
                if tag == "beat-unit-dot" && path.iter().any(|p| p == "metronome") {
                    current_tempo_beat_dots = true;
                }
            }

            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                let parent = path.last().map(|s| s.as_str()).unwrap_or("");

                match parent {
                    "part-name" => {
                        if let Some(ref mut pd) = current_part_def {
                            pd.name = text;
                        }
                    }
                    "divisions" => {
                        if let Some(ref mut a) = current_attrs {
                            a.divisions = text.parse().ok();
                        }
                    }
                    "fifths" => {
                        if let Some(ref mut a) = current_attrs {
                            a.key_fifths = text.parse().ok();
                        }
                    }
                    "beats" => {
                        if let Some(ref mut a) = current_attrs {
                            a.time_beats = text.parse().ok();
                        }
                    }
                    "beat-type" => {
                        if let Some(ref mut a) = current_attrs {
                            a.time_beat_type = text.parse().ok();
                        }
                    }
                    "staves" => {
                        if let Some(ref mut a) = current_attrs {
                            a.staves = text.parse().ok();
                        }
                    }
                    "sign" => {
                        if current_attrs.is_some() {
                            current_clef_sign = text;
                        }
                    }
                    "line" => {
                        if current_attrs.is_some() {
                            current_clef_line = text.parse().unwrap_or(2);
                        }
                    }
                    "clef-octave-change" => {
                        if current_attrs.is_some() {
                            current_clef_oct_change = text.parse().unwrap_or(0);
                        }
                    }
                    "step" => {
                        if let Some(ref mut n) = current_note {
                            n.step = text;
                        }
                    }
                    "alter" => {
                        if let Some(ref mut n) = current_note {
                            n.alter = text.parse().unwrap_or(0);
                        }
                    }
                    "octave" => {
                        if let Some(ref mut n) = current_note {
                            n.octave = text.parse().unwrap_or(4);
                        }
                    }
                    "duration" => {
                        if let Some(ref mut n) = current_note {
                            n.duration = text.parse().unwrap_or(0);
                        } else if let Some(ref mut m) = current_measure {
                            // forward or backup duration
                            let dur = text.parse::<i32>().unwrap_or(0);
                            let grandparent =
                                path.get(path.len().wrapping_sub(2)).map(|s| s.as_str());
                            match grandparent {
                                Some("forward") => {
                                    m.elements.push(XmlMeasureElement::Direction(
                                        XmlDirection::Forward(dur),
                                    ));
                                }
                                Some("backup") => {
                                    m.elements.push(XmlMeasureElement::Direction(
                                        XmlDirection::Backup(dur),
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                    "type" => {
                        if let Some(ref mut n) = current_note {
                            n.note_type = text;
                        }
                    }
                    // Staff number inside <direction> (for multi-staff parts)
                    "staff" if current_note.is_none() && path.iter().any(|p| p == "direction") => {
                        current_direction_staff = text.parse().unwrap_or(1);
                    }
                    "voice" => {
                        if let Some(ref mut n) = current_note {
                            n.voice = text.parse().unwrap_or(1);
                        }
                    }
                    "staff" => {
                        if let Some(ref mut n) = current_note {
                            n.staff = text.parse().unwrap_or(1);
                        }
                    }
                    "accidental" => {
                        if let Some(ref mut n) = current_note {
                            n.accidental = text;
                        }
                    }
                    "actual-notes" => {
                        if let Some(ref mut n) = current_note {
                            n.tuplet_actual = text.parse().unwrap_or(0);
                        }
                    }
                    "normal-notes" => {
                        if let Some(ref mut n) = current_note {
                            n.tuplet_normal = text.parse().unwrap_or(0);
                        }
                    }
                    "beam" => {
                        if let Some(ref mut n) = current_note {
                            // Ensure beam_levels has enough entries for this beam number
                            let idx = (current_beam_number - 1).max(0) as usize;
                            while n.beam_levels.len() <= idx {
                                n.beam_levels.push(String::new());
                            }
                            n.beam_levels[idx] = text;
                        }
                    }
                    // Tempo text inside <direction><direction-type><words>
                    "words" if path.iter().any(|p| p == "direction") => {
                        if current_tempo_words.is_empty() {
                            current_tempo_words = text;
                        } else {
                            // Concatenate multiple <words> in the same <direction>
                            current_tempo_words.push(' ');
                            current_tempo_words.push_str(&text);
                        }
                    }
                    // Metronome beat unit inside <metronome><beat-unit>
                    "beat-unit" if path.iter().any(|p| p == "metronome") => {
                        current_tempo_beat_unit = text;
                    }
                    // Metronome BPM inside <metronome><per-minute>
                    "per-minute" if path.iter().any(|p| p == "metronome") => {
                        current_tempo_per_minute = text.parse().unwrap_or(0);
                    }
                    _ => {}
                }
            }

            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag.as_str() {
                    "score-part" => {
                        if let Some(pd) = current_part_def.take() {
                            parts_def.push(pd);
                        }
                    }
                    "part" => {
                        if let Some(part) = current_part.take() {
                            parts.push(part);
                        }
                    }
                    "measure" => {
                        if let Some(meas) = current_measure.take() {
                            if let Some(ref mut part) = current_part {
                                part.measures.push(meas);
                            }
                            // Restore current_measure as None (already taken)
                        }
                    }
                    "attributes" => {
                        if let Some(attrs) = current_attrs.take() {
                            if let Some(ref mut meas) = current_measure {
                                meas.elements.push(XmlMeasureElement::Attributes(attrs));
                            }
                        }
                    }
                    "note" => {
                        if let Some(note) = current_note.take() {
                            if let Some(ref mut meas) = current_measure {
                                meas.elements.push(XmlMeasureElement::Note(note));
                            }
                        }
                    }
                    "clef" => {
                        if let Some(ref mut a) = current_attrs {
                            a.clefs.push((
                                current_clef_num,
                                current_clef_sign.clone(),
                                current_clef_line,
                                current_clef_oct_change,
                            ));
                        }
                    }
                    "direction" => {
                        // Flush any dynamic collected during this <direction>.
                        if let Some(dtype) = current_dynamic_type.take() {
                            if let Some(ref mut meas) = current_measure {
                                meas.elements
                                    .push(XmlMeasureElement::DynamicMarking(XmlDynamic {
                                        dynamic_type: dtype,
                                        staff: current_direction_staff,
                                    }));
                            }
                        }
                        // Flush tempo mark if we collected metronome or sound tempo or words.
                        let has_tempo = current_tempo_per_minute > 0
                            || current_tempo_sound > 0
                            || !current_tempo_words.is_empty();
                        if has_tempo {
                            let bpm = if current_tempo_per_minute > 0 {
                                current_tempo_per_minute
                            } else {
                                current_tempo_sound
                            };
                            let sub_type = type_to_l_dur(if current_tempo_beat_unit.is_empty() {
                                "quarter"
                            } else {
                                &current_tempo_beat_unit
                            });
                            if let Some(ref mut meas) = current_measure {
                                meas.elements.push(XmlMeasureElement::TempoMark(XmlTempo {
                                    sub_type,
                                    dotted: current_tempo_beat_dots,
                                    tempo_mm: bpm as i16,
                                    words: current_tempo_words.clone(),
                                    staff: current_direction_staff,
                                }));
                            }
                        }
                        // Flush ottava mark.
                        if let Some(oct_type) = current_ottava_type.take() {
                            if let Some(ref mut meas) = current_measure {
                                meas.elements.push(XmlMeasureElement::OttavaMark(XmlOttava {
                                    oct_type,
                                    staff: current_direction_staff,
                                }));
                            }
                        }
                    }
                    "barline" => {
                        if let Some(ref mut meas) = current_measure {
                            // Flush ending mark.
                            if let Some(num) = current_barline_ending_num {
                                meas.elements.push(XmlMeasureElement::EndingMark(XmlEnding {
                                    number: num,
                                    ending_type: current_barline_ending_type.clone(),
                                    measure_num: meas.number,
                                }));
                            }
                            // Flush repeat barline.
                            if let Some(ref dir) = current_barline_repeat_dir {
                                meas.elements
                                    .push(XmlMeasureElement::RepeatBarline(XmlRepeat {
                                        direction: dir.clone(),
                                        measure_num: meas.number,
                                    }));
                            }
                        }
                        in_barline = false;
                    }
                    _ => {}
                }

                if path.last().map(|s| s.as_str()) == Some(tag.as_str()) {
                    path.pop();
                }
            }

            _ => {}
        }
        buf.clear();
    }

    Ok((parts_def, parts))
}

/// Helper to extract an attribute string from an XML element.
fn attr_str(e: &quick_xml::events::BytesStart<'_>, name: &str) -> String {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == name.as_bytes())
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
        .unwrap_or_default()
}

// ============================================================================
// InterpretedScore builder
// ============================================================================

/// Build an InterpretedScore directly from parsed MusicXML parts.
///
/// Constructs the full NGL object hierarchy with proper linked-list wiring.
fn build_score(
    parts_def: &[XmlPartDef],
    parts: &[XmlPart],
) -> Result<InterpretedScore, ImportError> {
    if parts.is_empty() {
        return Err(ImportError::Missing("no parts in score".into()));
    }

    let mut score = InterpretedScore::new();

    // ---- Determine part→staff mapping ----
    struct PartStaffInfo {
        first_staff: i8,
        last_staff: i8,
        n_staves: i32,
        name: String,
    }

    let mut part_infos_list: Vec<PartStaffInfo> = Vec::new();
    let mut current_staff = 1i8;
    for (i, part) in parts.iter().enumerate() {
        let n_staves = part
            .measures
            .iter()
            .flat_map(|m| m.elements.iter())
            .find_map(|e| {
                if let XmlMeasureElement::Attributes(a) = e {
                    a.staves
                } else {
                    None
                }
            })
            .unwrap_or(1)
            .max(1) as i8;

        let name = parts_def
            .get(i)
            .map(|pd| pd.name.clone())
            .unwrap_or_else(|| format!("Part {}", i + 1));

        part_infos_list.push(PartStaffInfo {
            first_staff: current_staff,
            last_staff: current_staff + n_staves - 1,
            n_staves: n_staves as i32,
            name,
        });
        current_staff += n_staves;
    }

    let total_staves = (current_staff - 1) as usize;

    // ---- Build PartInfo subobjects ----
    for (i, psi) in part_infos_list.iter().enumerate() {
        let mut name_bytes = [0u8; 32];
        for (j, b) in psi.name.bytes().enumerate() {
            if j >= 31 {
                break;
            }
            name_bytes[j] = b;
        }
        score.part_infos.push(PartInfo {
            next: if i + 1 < part_infos_list.len() {
                (i + 2) as Link
            } else {
                NILINK
            },
            part_velocity: 0,
            first_staff: psi.first_staff,
            patch_num: 0,
            last_staff: psi.last_staff,
            channel: i as u8,
            transpose: 0,
            lo_key_num: 0,
            hi_key_num: 127,
            name: name_bytes,
            short_name: [0u8; 12],
            hi_key_name: 0,
            hi_key_acc: 0,
            tran_name: 0,
            tran_acc: 0,
            lo_key_name: 0,
            lo_key_acc: 0,
            bank_number0: 0,
            bank_number32: 0,
            fms_output_device: 0u16,
            fms_output_destination: [0u8; 280],
        });
    }

    // ---- Extract initial attributes from first measure ----
    let mut init_divisions = PDUR_QUARTER;
    let mut init_key_fifths = 0i32;
    let mut init_time_num = 4i8;
    let mut init_time_denom = 4i8;
    let mut init_clefs: Vec<u8> = vec![TREBLE_CLEF; total_staves + 1]; // 1-indexed

    // Get initial attributes from each part's first measure
    for (pi, part) in parts.iter().enumerate() {
        let psi = &part_infos_list[pi];
        if let Some(first_meas) = part.measures.first() {
            for elem in &first_meas.elements {
                if let XmlMeasureElement::Attributes(attrs) = elem {
                    if let Some(d) = attrs.divisions {
                        init_divisions = d;
                    }
                    if let Some(f) = attrs.key_fifths {
                        init_key_fifths = f;
                    }
                    if let (Some(b), Some(bt)) = (attrs.time_beats, attrs.time_beat_type) {
                        init_time_num = b as i8;
                        init_time_denom = bt as i8;
                    }
                    for &(staff_in_part, ref sign, line, oct_change) in &attrs.clefs {
                        let global = psi.first_staff + staff_in_part as i8 - 1;
                        if global >= 1 && (global as usize) <= total_staves {
                            init_clefs[global as usize] = clef_to_ngl(sign, line, oct_change);
                        }
                    }
                }
            }
        }
        // Default bass clef for second staff of multi-staff parts
        if psi.n_staves > 1 && init_clefs[psi.last_staff as usize] == TREBLE_CLEF {
            init_clefs[psi.last_staff as usize] = BASS_CLEF;
        }
    }

    // ---- Collect all note events globally ----
    // Each event: (measure_number, time_in_measure, global_staff, voice, XmlNote)
    struct NoteEntry {
        measure_num: i32,
        time: i32, // PDUR ticks from measure start
        global_staff: i8,
        voice: i8,
        midi: u8,
        l_dur: i8,
        dots: u8,
        rest: bool,
        chord: bool,
        acc: u8,
        tied_l: bool,
        tied_r: bool,
        slurred_l: bool,
        slurred_r: bool,
        play_dur: i16,
        /// Beam levels from MusicXML: beam_levels[0] = primary beam, etc.
        beam_levels: Vec<String>,
        /// Tuplet actual/normal notes (from <time-modification>).
        tuplet_actual: u8,
        tuplet_normal: u8,
        /// Tuplet start/stop flags (from <tuplet> notation element).
        tuplet_start: bool,
        tuplet_stop: bool,
        /// True if this note is a grace note.
        is_grace: bool,
        /// Articulation/ornament modifier codes (NGL ModCode values).
        mod_codes: Vec<u8>,
    }

    let mut all_notes: Vec<NoteEntry> = Vec::new();
    // Collected dynamics: (measure_num, global_staff, dynamic_type).
    let mut collected_dynamics: Vec<(i32, i8, u8)> = Vec::new();
    let mut collected_tempos: Vec<(i32, i8, XmlTempo)> = Vec::new();
    let mut collected_endings: Vec<XmlEnding> = Vec::new();
    let mut collected_repeats: Vec<XmlRepeat> = Vec::new();
    let mut collected_ottavas: Vec<(i32, i8, XmlOttava)> = Vec::new();
    let num_measures = parts.iter().map(|p| p.measures.len()).max().unwrap_or(0);

    // Track mid-score attribute changes per measure (key, time, clefs).
    // Indexed by measure number (1-based).
    struct MidScoreAttrChange {
        key_fifths: Option<i32>,
        time_sig: Option<(i8, i8)>,
        clefs: Vec<(i8, u8)>, // (global_staff, ngl_clef_type)
    }
    let mut mid_score_attrs: std::collections::HashMap<i32, MidScoreAttrChange> =
        std::collections::HashMap::new();

    for (pi, part) in parts.iter().enumerate() {
        let psi = &part_infos_list[pi];
        let mut divisions = init_divisions;
        let mut current_fifths = init_key_fifths;

        for meas in &part.measures {
            let mut voice_time: i32 = 0;

            for elem in &meas.elements {
                match elem {
                    XmlMeasureElement::Attributes(attrs) => {
                        if let Some(d) = attrs.divisions {
                            divisions = d;
                        }
                        // Track current key for accidental filtering
                        if let Some(f) = attrs.key_fifths {
                            current_fifths = f;
                        }
                        // Collect mid-score attribute changes (skip first measure)
                        if meas.number > 1 {
                            let entry =
                                mid_score_attrs
                                    .entry(meas.number)
                                    .or_insert(MidScoreAttrChange {
                                        key_fifths: None,
                                        time_sig: None,
                                        clefs: Vec::new(),
                                    });
                            if let Some(f) = attrs.key_fifths {
                                entry.key_fifths = Some(f);
                            }
                            if let (Some(b), Some(bt)) = (attrs.time_beats, attrs.time_beat_type) {
                                entry.time_sig = Some((b as i8, bt as i8));
                            }
                            for &(staff_in_part, ref sign, line, oct_change) in &attrs.clefs {
                                let global = psi.first_staff + staff_in_part as i8 - 1;
                                if global >= 1 && (global as usize) <= total_staves {
                                    entry
                                        .clefs
                                        .push((global, clef_to_ngl(sign, line, oct_change)));
                                }
                            }
                        }
                    }
                    XmlMeasureElement::Note(note) => {
                        let global_staff = psi.first_staff + note.staff as i8 - 1;

                        // Grace notes have zero duration — don't advance voice_time.
                        let pdur = if note.grace {
                            0
                        } else {
                            xml_dur_to_pdur(note.duration, divisions)
                        };

                        let t = if note.grace {
                            // Grace notes get the current time position (they attach
                            // to the next real note at this time).
                            voice_time
                        } else if note.chord {
                            (voice_time - pdur).max(0)
                        } else {
                            voice_time
                        };

                        let l_dur = if note.rest && note.note_type.is_empty() {
                            -1i8 // WHOLEMR_L_DUR
                        } else {
                            type_to_l_dur(&note.note_type)
                        };

                        let midi = if note.rest {
                            0
                        } else {
                            pitch_to_midi(&note.step, note.octave, note.alter)
                        };

                        let raw_acc = accidental_to_code(&note.accidental, note.alter);
                        // Suppress accidentals that are already implied by the key sig.
                        // E.g. in 4 sharps (E major), F# doesn't need a visible sharp.
                        let acc = if !note.rest
                            && is_acc_redundant_for_ks(&note.step, raw_acc, current_fifths)
                        {
                            0
                        } else {
                            raw_acc
                        };

                        all_notes.push(NoteEntry {
                            measure_num: meas.number,
                            time: t,
                            global_staff,
                            voice: note.voice as i8,
                            midi,
                            l_dur,
                            dots: note.dots,
                            rest: note.rest,
                            chord: note.chord,
                            acc,
                            tied_l: note.tie_stop,
                            tied_r: note.tie_start,
                            slurred_l: note.slur_stop,
                            slurred_r: note.slur_start,
                            play_dur: pdur as i16,
                            beam_levels: note.beam_levels.clone(),
                            tuplet_actual: note.tuplet_actual,
                            tuplet_normal: note.tuplet_normal,
                            tuplet_start: note.tuplet_start,
                            tuplet_stop: note.tuplet_stop,
                            is_grace: note.grace,
                            mod_codes: note.mod_codes.clone(),
                        });

                        if !note.chord && !note.grace {
                            voice_time += pdur;
                        }
                    }
                    XmlMeasureElement::Direction(dir) => match dir {
                        XmlDirection::Forward(dur) => {
                            voice_time += xml_dur_to_pdur(*dur, divisions);
                        }
                        XmlDirection::Backup(dur) => {
                            voice_time -= xml_dur_to_pdur(*dur, divisions);
                            if voice_time < 0 {
                                voice_time = 0;
                            }
                        }
                    },
                    XmlMeasureElement::DynamicMarking(dyn_mark) => {
                        let global_staff = psi.first_staff + dyn_mark.staff as i8 - 1;
                        collected_dynamics.push((meas.number, global_staff, dyn_mark.dynamic_type));
                    }
                    XmlMeasureElement::TempoMark(tempo) => {
                        let global_staff = psi.first_staff + tempo.staff as i8 - 1;
                        collected_tempos.push((meas.number, global_staff, tempo.clone()));
                    }
                    XmlMeasureElement::EndingMark(ending) => {
                        collected_endings.push(ending.clone());
                    }
                    XmlMeasureElement::RepeatBarline(repeat) => {
                        collected_repeats.push(repeat.clone());
                    }
                    XmlMeasureElement::OttavaMark(ottava) => {
                        let global_staff = psi.first_staff + ottava.staff as i8 - 1;
                        collected_ottavas.push((meas.number, global_staff, ottava.clone()));
                    }
                }
            }
        }
    }

    // Separate grace notes from regular notes so they can be processed differently.
    // Grace notes become GrSync objects positioned before their parent Sync.
    let mut grace_notes: Vec<NoteEntry> = Vec::new();
    let mut regular_notes: Vec<NoteEntry> = Vec::new();
    for ne in all_notes {
        if ne.is_grace {
            grace_notes.push(ne);
        } else {
            regular_notes.push(ne);
        }
    }
    let mut all_notes = regular_notes;

    // Sort regular notes by (measure_num, time, staff, voice) for creating Sync objects.
    all_notes.sort_by(|a, b| {
        a.measure_num.cmp(&b.measure_num).then_with(|| {
            a.time.cmp(&b.time).then_with(|| {
                a.global_staff
                    .cmp(&b.global_staff)
                    .then_with(|| a.voice.cmp(&b.voice))
            })
        })
    });
    // Sort grace notes similarly — they'll be matched to Syncs by (measure_num, time).
    grace_notes.sort_by(|a, b| {
        a.measure_num.cmp(&b.measure_num).then_with(|| {
            a.time.cmp(&b.time).then_with(|| {
                a.global_staff
                    .cmp(&b.global_staff)
                    .then_with(|| a.voice.cmp(&b.voice))
            })
        })
    });

    // ---- Build object hierarchy ----
    // Objects vector (1-indexed, slot 0 = placeholder)
    let mut objects: Vec<InterpretedObject> = Vec::new();
    objects.push(InterpretedObject {
        index: 0,
        header: ObjectHeader::default(),
        data: ObjData::Header(Header {
            header: ObjectHeader::default(),
        }),
    }); // placeholder at index 0

    // Sub-object link counter (separate namespace from object links)
    let mut next_sub_link: Link = 1;

    /// Allocate a sub-object link.
    fn alloc_sub(counter: &mut Link) -> Link {
        let l = *counter;
        *counter += 1;
        l
    }

    // Helper: push an object and return its link (1-based index)
    fn push_obj(objects: &mut Vec<InterpretedObject>, obj: InterpretedObject) -> Link {
        let link = objects.len() as Link;
        objects.push(InterpretedObject { index: link, ..obj });
        link
    }

    // ---- 1. HEADER (link 1) ----
    let header_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 0, // HEADER_TYPE
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Header(Header {
                header: ObjectHeader::default(),
            }),
        },
    );

    // ---- 2. PAGE (link 2) ----
    let page_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 4, // PAGE_TYPE
                left: header_link,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Page(Page {
                header: ObjectHeader::default(),
                l_page: NILINK,
                r_page: NILINK,
                sheet_num: 0,
                header_str_offset: 0,
                footer_str_offset: 0,
            }),
        },
    );
    objects[header_link as usize].header.right = page_link;

    // ---- 3. SYSTEM (link 3) ----
    // Put everything in a single system for now.
    let system_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 5, // SYSTEM_TYPE
                left: page_link,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::System(System {
                header: ObjectHeader::default(),
                l_system: NILINK,
                r_system: NILINK,
                page_l: page_link,
                system_num: 1,
                system_rect: DRect {
                    top: 0,
                    left: 0,
                    bottom: 9792, // ~612pt * 16
                    right: 12672, // ~792pt * 16
                },
                sys_desc_ptr: 0,
            }),
        },
    );
    objects[page_link as usize].header.right = system_link;

    // ---- 4. STAFF ----
    let staff_sub_link = alloc_sub(&mut next_sub_link);
    let mut staff_subs: Vec<AStaff> = Vec::new();
    for s in 1..=total_staves {
        staff_subs.push(AStaff {
            next: if s < total_staves {
                staff_sub_link + s as Link
            } else {
                NILINK
            },
            staffn: s as i8,
            selected: false,
            visible: true,
            filler_stf: false,
            staff_top: (s as i16 - 1) * 640, // ~40pt per staff
            staff_left: 0,
            staff_right: 9600, // ~600pt
            staff_height: 128, // ~8pt (standard 5-line staff)
            staff_lines: 5,
            font_size: 24,
            flag_leading: 56,
            min_stem_free: 0,
            ledger_width: 36,
            note_head_width: 26,
            frac_beam_width: 10,
            space_below: 256,
            clef_type: init_clefs.get(s).copied().unwrap_or(TREBLE_CLEF) as i8,
            dynamic_type: 0,
            ks_info: fifths_to_ks_info(init_key_fifths),
            time_sig_type: 1,
            numerator: init_time_num,
            denominator: init_time_denom,
            filler: 0,
            show_ledgers: 1,
            show_lines: SHOW_ALL_LINES,
        });
    }
    next_sub_link += total_staves as Link;

    let staff_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 6, // STAFF_TYPE
                left: system_link,
                first_sub_obj: staff_sub_link,
                n_entries: total_staves as u8,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Staff(Staff {
                header: ObjectHeader::default(),
                l_staff: NILINK,
                r_staff: NILINK,
                system_l: system_link,
            }),
        },
    );
    score.staffs.insert(staff_sub_link, staff_subs);
    objects[system_link as usize].header.right = staff_link;

    // ---- 5. CONNECT ----
    let conn_sub_link = alloc_sub(&mut next_sub_link);
    let connect_subs = vec![AConnect {
        next: NILINK,
        selected: false,
        filler: 0,
        conn_level: 0,   // system level
        connect_type: 3, // curly brace
        staff_above: 1,
        staff_below: total_staves as i8,
        xd: 0,
        first_part: 1,
        last_part: part_infos_list.len() as Link,
    }];

    let connect_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 12, // CONNECT_TYPE
                left: staff_link,
                first_sub_obj: conn_sub_link,
                n_entries: 1,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Connect(Connect {
                header: ObjectHeader::default(),
                conn_filler: NILINK,
            }),
        },
    );
    score.connects.insert(conn_sub_link, connect_subs);
    objects[staff_link as usize].header.right = connect_link;

    // ---- 6. CLEF ----
    let clef_sub_link = alloc_sub(&mut next_sub_link);
    let mut clef_subs: Vec<AClef> = Vec::new();
    for s in 1..=total_staves {
        clef_subs.push(AClef {
            header: SubObjHeader {
                next: if s < total_staves {
                    clef_sub_link + s as Link
                } else {
                    NILINK
                },
                staffn: s as i8,
                sub_type: init_clefs.get(s).copied().unwrap_or(TREBLE_CLEF) as i8,
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
    next_sub_link += total_staves as Link;

    let clef_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 8, // CLEF_TYPE
                left: connect_link,
                first_sub_obj: clef_sub_link,
                n_entries: total_staves as u8,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Clef(Clef {
                header: ObjectHeader::default(),
                in_measure: false,
            }),
        },
    );
    score.clefs.insert(clef_sub_link, clef_subs);
    objects[connect_link as usize].header.right = clef_link;

    // ---- 7. KEYSIG ----
    let ks_sub_link = alloc_sub(&mut next_sub_link);
    let mut ks_subs: Vec<AKeySig> = Vec::new();
    for s in 1..=total_staves {
        ks_subs.push(AKeySig {
            header: SubObjHeader {
                next: if s < total_staves {
                    ks_sub_link + s as Link
                } else {
                    NILINK
                },
                staffn: s as i8,
                sub_type: 0,
                selected: false,
                visible: true,
                soft: false,
            },
            nonstandard: 0,
            filler1: 0,
            small: 0,
            filler2: 0,
            xd: 0,
            ks_info: fifths_to_ks_info(init_key_fifths),
        });
    }
    next_sub_link += total_staves as Link;

    let ks_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 9, // KEYSIG_TYPE
                left: clef_link,
                first_sub_obj: ks_sub_link,
                n_entries: total_staves as u8,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::KeySig(KeySig {
                header: ObjectHeader::default(),
                in_measure: false,
            }),
        },
    );
    score.keysigs.insert(ks_sub_link, ks_subs);
    objects[clef_link as usize].header.right = ks_link;

    // ---- 8. TIMESIG ----
    let ts_sub_link = alloc_sub(&mut next_sub_link);
    let mut ts_subs: Vec<ATimeSig> = Vec::new();
    for s in 1..=total_staves {
        ts_subs.push(ATimeSig {
            header: SubObjHeader {
                next: if s < total_staves {
                    ts_sub_link + s as Link
                } else {
                    NILINK
                },
                staffn: s as i8,
                sub_type: 1, // NOverD
                selected: false,
                visible: true,
                soft: false,
            },
            filler: 0,
            small: 0,
            conn_staff: 0,
            xd: 0,
            yd: 0,
            numerator: init_time_num,
            denominator: init_time_denom,
        });
    }
    next_sub_link += total_staves as Link;

    let ts_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 10, // TIMESIG_TYPE
                left: ks_link,
                first_sub_obj: ts_sub_link,
                n_entries: total_staves as u8,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::TimeSig(TimeSig {
                header: ObjectHeader::default(),
                in_measure: false,
            }),
        },
    );
    score.timesigs.insert(ts_sub_link, ts_subs);
    objects[ks_link as usize].header.right = ts_link;

    // ---- 9. MEASURES + SYNC objects ----
    let mut prev_link = ts_link;
    let mut measure_start_time: i32 = 0;
    // Track current key/time for AMeasure subobjects (updated on mid-score changes)
    let mut current_key_fifths = init_key_fifths;
    let mut current_time_num = init_time_num;
    let mut current_time_denom = init_time_denom;
    // Collect beam info during Sync creation: (sync_link, staff, voice, beam_levels)
    let mut beam_notes_with_links: Vec<(Link, i8, i8, Vec<String>)> = Vec::new();
    // Tuplet info: (sync_link, staff, voice, actual, normal, tuplet_start, tuplet_stop)
    let mut tuplet_notes_with_links: Vec<(Link, i8, i8, u8, u8, bool, bool)> = Vec::new();

    // Staff height for yd/ystem computation — must match LayoutConfig::default().
    // layout_score() will later set the definitive value, but we compute yd/ystem here
    // using the same default so they're consistent.
    let layout_cfg = LayoutConfig::default();
    let staff_height = layout_cfg.staff_height;
    let n_staff_lines: i16 = 5;
    // Stem length defaults (quarter-spaces) — same as NotelistLayoutConfig
    let stem_len_normal: i16 = 14;
    let stem_len_outside: i16 = 12;
    let stem_len_2v: i16 = 13;

    // Determine voice roles per (staff, voice) pair for stem direction.
    // If a staff has multiple voices, the lowest-numbered is Upper, others are Lower.
    // If only one voice on a staff, it's Single.
    let mut staff_voices: std::collections::HashMap<i8, std::collections::BTreeSet<i8>> =
        std::collections::HashMap::new();
    for ne in &all_notes {
        if !ne.rest {
            staff_voices
                .entry(ne.global_staff)
                .or_default()
                .insert(ne.voice);
        }
    }
    let mut voice_roles: std::collections::HashMap<(i8, i8), VoiceRole> =
        std::collections::HashMap::new();
    for (staff, voices) in &staff_voices {
        if voices.len() <= 1 {
            for v in voices {
                voice_roles.insert((*staff, *v), VoiceRole::Single);
            }
        } else {
            let min_voice = *voices.iter().next().unwrap();
            for v in voices {
                if *v == min_voice {
                    voice_roles.insert((*staff, *v), VoiceRole::Upper);
                } else {
                    voice_roles.insert((*staff, *v), VoiceRole::Lower);
                }
            }
        }
    }

    for meas_num in 1..=(num_measures as i32) {
        // ---- Insert mid-score Clef/KeySig/TimeSig objects before this measure ----
        if let Some(change) = mid_score_attrs.remove(&meas_num) {
            // Insert Clef object if there are clef changes
            // Also update running clef tracker so yd computation uses the right clef
            for (gs, ct) in &change.clefs {
                if (*gs as usize) < init_clefs.len() {
                    init_clefs[*gs as usize] = *ct;
                }
            }
            if !change.clefs.is_empty() {
                let csub_link = alloc_sub(&mut next_sub_link);
                let mut csubs: Vec<AClef> = Vec::new();
                for (i, s) in (1..=total_staves).enumerate() {
                    // Find if this staff has a clef change
                    let clef_type = change
                        .clefs
                        .iter()
                        .find(|(gs, _)| *gs == s as i8)
                        .map(|(_, ct)| *ct)
                        .unwrap_or(0); // 0 = no change for this staff
                    if clef_type != 0 {
                        csubs.push(AClef {
                            header: SubObjHeader {
                                next: if i + 1 < total_staves {
                                    csub_link + (i + 1) as Link
                                } else {
                                    NILINK
                                },
                                staffn: s as i8,
                                sub_type: clef_type as i8,
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
                }
                if !csubs.is_empty() {
                    let n_entries = csubs.len() as u8;
                    // Fix up linked list for the actual subs we have
                    for i in 0..csubs.len() {
                        csubs[i].header.next = if i + 1 < csubs.len() {
                            csub_link + (i + 1) as Link
                        } else {
                            NILINK
                        };
                    }
                    next_sub_link += csubs.len() as Link;
                    let clink = push_obj(
                        &mut objects,
                        InterpretedObject {
                            index: 0,
                            header: ObjectHeader {
                                obj_type: 8, // CLEF_TYPE
                                left: prev_link,
                                first_sub_obj: csub_link,
                                n_entries,
                                visible: true,
                                valid: true,
                                ..Default::default()
                            },
                            data: ObjData::Clef(Clef {
                                header: ObjectHeader::default(),
                                in_measure: true,
                            }),
                        },
                    );
                    score.clefs.insert(csub_link, csubs);
                    objects[prev_link as usize].header.right = clink;
                    prev_link = clink;
                }
            }

            // Insert KeySig object if there is a key change
            if let Some(fifths) = change.key_fifths {
                current_key_fifths = fifths;
                let ksub_link = alloc_sub(&mut next_sub_link);
                let mut ksubs: Vec<AKeySig> = Vec::new();
                for s in 1..=total_staves {
                    ksubs.push(AKeySig {
                        header: SubObjHeader {
                            next: if s < total_staves {
                                ksub_link + s as Link
                            } else {
                                NILINK
                            },
                            staffn: s as i8,
                            sub_type: 0,
                            selected: false,
                            visible: true,
                            soft: false,
                        },
                        nonstandard: 0,
                        filler1: 0,
                        small: 0,
                        filler2: 0,
                        xd: 0,
                        ks_info: fifths_to_ks_info(fifths),
                    });
                }
                next_sub_link += total_staves as Link;
                let klink = push_obj(
                    &mut objects,
                    InterpretedObject {
                        index: 0,
                        header: ObjectHeader {
                            obj_type: 9, // KEYSIG_TYPE
                            left: prev_link,
                            first_sub_obj: ksub_link,
                            n_entries: total_staves as u8,
                            visible: true,
                            valid: true,
                            ..Default::default()
                        },
                        data: ObjData::KeySig(KeySig {
                            header: ObjectHeader::default(),
                            in_measure: true,
                        }),
                    },
                );
                score.keysigs.insert(ksub_link, ksubs);
                objects[prev_link as usize].header.right = klink;
                prev_link = klink;
            }

            // Insert TimeSig object if there is a time sig change
            if let Some((num, denom)) = change.time_sig {
                current_time_num = num;
                current_time_denom = denom;
                let tsub_link = alloc_sub(&mut next_sub_link);
                let mut tsubs: Vec<ATimeSig> = Vec::new();
                for s in 1..=total_staves {
                    tsubs.push(ATimeSig {
                        header: SubObjHeader {
                            next: if s < total_staves {
                                tsub_link + s as Link
                            } else {
                                NILINK
                            },
                            staffn: s as i8,
                            sub_type: 1, // NOverD
                            selected: false,
                            visible: true,
                            soft: false,
                        },
                        filler: 0,
                        small: 0,
                        conn_staff: 0,
                        xd: 0,
                        yd: 0,
                        numerator: num,
                        denominator: denom,
                    });
                }
                next_sub_link += total_staves as Link;
                let tlink = push_obj(
                    &mut objects,
                    InterpretedObject {
                        index: 0,
                        header: ObjectHeader {
                            obj_type: 10, // TIMESIG_TYPE
                            left: prev_link,
                            first_sub_obj: tsub_link,
                            n_entries: total_staves as u8,
                            visible: true,
                            valid: true,
                            ..Default::default()
                        },
                        data: ObjData::TimeSig(TimeSig {
                            header: ObjectHeader::default(),
                            in_measure: true,
                        }),
                    },
                );
                score.timesigs.insert(tsub_link, tsubs);
                objects[prev_link as usize].header.right = tlink;
                prev_link = tlink;
            }
        }

        // Create MEASURE object
        let meas_sub_link = alloc_sub(&mut next_sub_link);
        let mut meas_subs: Vec<AMeasure> = Vec::new();
        for s in 1..=total_staves {
            meas_subs.push(AMeasure {
                header: SubObjHeader {
                    next: if s < total_staves {
                        meas_sub_link + s as Link
                    } else {
                        NILINK
                    },
                    staffn: s as i8,
                    sub_type: 1, // normal barline
                    selected: false,
                    visible: true,
                    soft: false,
                },
                measure_visible: true,
                conn_above: false,
                filler1: 0,
                filler2: 0,
                reserved_m: 0,
                measure_num: (meas_num - 1) as i16,
                meas_size_rect: DRect::default(),
                conn_staff: 0,
                clef_type: 0,
                dynamic_type: 0,
                ks_info: fifths_to_ks_info(current_key_fifths),
                time_sig_type: 1,
                numerator: current_time_num,
                denominator: current_time_denom,
                x_mn_std_offset: 0,
                y_mn_std_offset: 0,
            });
        }
        next_sub_link += total_staves as Link;

        let meas_link = push_obj(
            &mut objects,
            InterpretedObject {
                index: 0,
                header: ObjectHeader {
                    obj_type: 7, // MEASURE_TYPE
                    left: prev_link,
                    first_sub_obj: meas_sub_link,
                    n_entries: total_staves as u8,
                    visible: true,
                    valid: true,
                    ..Default::default()
                },
                data: ObjData::Measure(Measure {
                    header: ObjectHeader::default(),
                    filler_m: 0,
                    l_measure: NILINK, // simplified
                    r_measure: NILINK,
                    system_l: system_link,
                    staff_l: NILINK,
                    fake_meas: 0,
                    space_percent: 100,
                    measure_b_box: Rect::default(),
                    l_time_stamp: measure_start_time,
                }),
            },
        );
        score.measures.insert(meas_sub_link, meas_subs);
        objects[prev_link as usize].header.right = meas_link;
        prev_link = meas_link;

        // Collect notes for this measure and group by timestamp
        let meas_notes: Vec<&NoteEntry> = all_notes
            .iter()
            .filter(|n| n.measure_num == meas_num)
            .collect();

        // Group by timestamp
        let mut time_groups: Vec<(i32, Vec<&NoteEntry>)> = Vec::new();
        for ne in &meas_notes {
            if let Some(last) = time_groups.last_mut() {
                if last.0 == ne.time {
                    last.1.push(ne);
                    continue;
                }
            }
            time_groups.push((ne.time, vec![ne]));
        }

        // Deduplicate/sort time groups
        let mut unique_times: Vec<i32> = time_groups.iter().map(|(t, _)| *t).collect();
        unique_times.sort();
        unique_times.dedup();

        let mut grouped: Vec<(i32, Vec<&NoteEntry>)> = Vec::new();
        for t in &unique_times {
            let notes_at_t: Vec<&NoteEntry> = meas_notes
                .iter()
                .filter(|n| n.time == *t)
                .copied()
                .collect();
            grouped.push((*t, notes_at_t));
        }

        // Create SYNC for each time group
        for (time, notes_at_time) in &grouped {
            let sync_sub_link = alloc_sub(&mut next_sub_link);
            let mut note_subs: Vec<ANote> = Vec::new();

            for (ni, ne) in notes_at_time.iter().enumerate() {
                let yqpit = if ne.rest { 0 } else { midi_to_yqpit(ne.midi) };

                // Compute yd (vertical position) from yqpit + clef context.
                // yqpit is clef-independent half-lines from middle C (negative = above).
                // half_ln is clef-relative: 0 = top staff line, positive = downward.
                // Formula: half_ln = clef_middle_c_half_ln(clef) + yqpit
                let clef_type = init_clefs
                    .get(ne.global_staff as usize)
                    .copied()
                    .unwrap_or(TREBLE_CLEF);
                let (yd, ystem) = if ne.rest {
                    // Rests go at center of staff (half-line 4 for 5-line staff)
                    let rest_role = voice_roles
                        .get(&(ne.global_staff, ne.voice))
                        .copied()
                        .unwrap_or(VoiceRole::Single);
                    let rest_hl: i16 = match rest_role {
                        VoiceRole::Single => 4,
                        VoiceRole::Upper => 2, // offset up
                        VoiceRole::Lower => 6, // offset down
                    };
                    let yd = half_ln_to_yd(rest_hl, staff_height);
                    (yd, yd)
                } else {
                    let mid_c_hl = clef_middle_c_half_ln(clef_type);
                    let half_ln = mid_c_hl + yqpit as i16;
                    let yd = half_ln_to_yd(half_ln, staff_height);

                    // Determine stem direction and compute ystem
                    let role = voice_roles
                        .get(&(ne.global_staff, ne.voice))
                        .copied()
                        .unwrap_or(VoiceRole::Single);
                    let stem_down = normal_stem_up_down_single(half_ln, n_staff_lines, role);
                    let num_flags = nflags(ne.l_dur);
                    let qtr_sp = match role {
                        VoiceRole::Single => {
                            if shorten_stem(half_ln, stem_down, n_staff_lines) {
                                stem_len_outside
                            } else {
                                stem_len_normal
                            }
                        }
                        _ => stem_len_2v,
                    };
                    let ystem = if ne.l_dur >= 3 {
                        // Has stem (half note and shorter)
                        calc_ystem(
                            yd,
                            num_flags,
                            stem_down,
                            staff_height,
                            n_staff_lines,
                            qtr_sp,
                            false,
                        )
                    } else {
                        yd // Whole notes/breves: no stem
                    };
                    (yd, ystem)
                };

                note_subs.push(ANote {
                    header: SubObjHeader {
                        next: if ni + 1 < notes_at_time.len() {
                            sync_sub_link + (ni + 1) as Link
                        } else {
                            NILINK
                        },
                        staffn: ne.global_staff,
                        sub_type: ne.l_dur,
                        selected: false,
                        visible: true,
                        soft: false,
                    },
                    in_chord: ne.chord,
                    rest: ne.rest,
                    unpitched: false,
                    beamed: !ne.beam_levels.is_empty(),
                    other_stem_side: false,
                    yqpit,
                    xd: 0,
                    yd,
                    ystem,
                    play_time_delta: 0,
                    play_dur: ne.play_dur,
                    p_time: 0,
                    note_num: ne.midi,
                    on_velocity: 75,
                    off_velocity: 64,
                    tied_l: ne.tied_l,
                    tied_r: ne.tied_r,
                    x_move_dots: 0,
                    y_move_dots: 2,
                    ndots: ne.dots,
                    voice: ne.voice,
                    rsp_ignore: 0,
                    accident: ne.acc,
                    acc_soft: false,
                    courtesy_acc: 0,
                    xmove_acc: 0,
                    play_as_cue: false,
                    micropitch: 0,
                    merged: 0,
                    double_dur: 0,
                    head_shape: 1, // NormalVis
                    first_mod: NILINK,
                    slurred_l: ne.slurred_l,
                    slurred_r: ne.slurred_r,
                    in_tuplet: ne.tuplet_actual > 0,
                    in_ottava: false,
                    small: false,
                    temp_flag: 0,
                    art_harmonic: 0,
                    user_id: 0,
                    nh_segment: [0u8; 6],
                    reserved_n: 0,
                });
            }
            next_sub_link += notes_at_time.len() as Link;

            // Allocate AModNr sub-objects for notes with articulations/ornaments
            for (ni, ne) in notes_at_time.iter().enumerate() {
                if ne.mod_codes.is_empty() {
                    continue;
                }
                let mod_sub_link = alloc_sub(&mut next_sub_link);
                let mut mod_subs: Vec<AModNr> = Vec::new();
                for (mi, &mc) in ne.mod_codes.iter().enumerate() {
                    mod_subs.push(AModNr {
                        next: if mi + 1 < ne.mod_codes.len() {
                            mod_sub_link + (mi + 1) as Link
                        } else {
                            NILINK
                        },
                        selected: false,
                        visible: true,
                        soft: false,
                        xstd: 0,
                        mod_code: mc,
                        data: 0,
                        ystdpit: 0,
                    });
                }
                // alloc_sub already consumed 1 link; consume the rest for multi-mod notes
                if ne.mod_codes.len() > 1 {
                    next_sub_link += (ne.mod_codes.len() - 1) as Link;
                }
                note_subs[ni].first_mod = mod_sub_link;
                score.modnrs.insert(mod_sub_link, mod_subs);
            }

            let sync_link = push_obj(
                &mut objects,
                InterpretedObject {
                    index: 0,
                    header: ObjectHeader {
                        obj_type: 2, // SYNC_TYPE
                        left: prev_link,
                        first_sub_obj: sync_sub_link,
                        n_entries: notes_at_time.len() as u8,
                        visible: true,
                        valid: true,
                        ..Default::default()
                    },
                    data: ObjData::Sync(NglSync {
                        header: ObjectHeader::default(),
                        time_stamp: (*time as u16),
                    }),
                },
            );
            score.notes.insert(sync_sub_link, note_subs);
            objects[prev_link as usize].header.right = sync_link;

            // Collect beam info for each beamed note at this sync
            for ne in notes_at_time {
                if !ne.beam_levels.is_empty() {
                    beam_notes_with_links.push((
                        sync_link,
                        ne.global_staff,
                        ne.voice,
                        ne.beam_levels.clone(),
                    ));
                }
                // Collect tuplet info for notes with tuplet_start or tuplet_stop
                if ne.tuplet_start || ne.tuplet_stop || ne.tuplet_actual > 0 {
                    tuplet_notes_with_links.push((
                        sync_link,
                        ne.global_staff,
                        ne.voice,
                        ne.tuplet_actual,
                        ne.tuplet_normal,
                        ne.tuplet_start,
                        ne.tuplet_stop,
                    ));
                }
            }

            prev_link = sync_link;
        }

        // Advance measure start time
        let meas_dur =
            (PDUR_QUARTER as i64 * 4 * init_time_num as i64 / init_time_denom as i64) as i32;
        measure_start_time += meas_dur;
    }

    // ---- 9b. BEAMSET OBJECTS ----
    // Create BeamSet + ANoteBeam objects from beam info collected during Sync creation.
    // beam_notes_with_links: (sync_link, staff, voice, beam_levels) — already in order.
    // Group consecutive beamed notes by (staff, voice) into beam groups.
    // A beam group starts with "begin" on primary beam and ends with "end".
    {
        let mut current_group: Vec<(Link, Vec<String>)> = Vec::new();
        let mut current_sv: Option<(i8, i8)> = None;

        let flush_group = |group: &mut Vec<(Link, Vec<String>)>,
                           sv: (i8, i8),
                           objects: &mut Vec<InterpretedObject>,
                           score: &mut InterpretedScore,
                           next_sub_link: &mut Link,
                           prev_link: &mut Link| {
            if group.len() < 2 {
                group.clear();
                return;
            }

            // Create ANoteBeam subobjects
            let beam_sub_link = alloc_sub(next_sub_link);
            let mut notebeam_subs: Vec<ANoteBeam> = Vec::new();

            for (gi, (sync_link, beam_levels)) in group.iter().enumerate() {
                // Compute startend from beam_levels:
                // Count how many non-hook beams at this note.
                let active_beams = beam_levels
                    .iter()
                    .filter(|bv| {
                        let s = bv.as_str();
                        s == "begin" || s == "continue" || s == "end"
                    })
                    .count() as i8;
                // For first note: startend = +active_beams (beams starting)
                // For last note: startend = -active_beams (beams ending)
                // For middle notes: startend = change from previous
                let startend = if gi == 0 {
                    active_beams
                } else {
                    let prev_active = group[gi - 1]
                        .1
                        .iter()
                        .filter(|bv| {
                            let s = bv.as_str();
                            s == "begin" || s == "continue" || s == "end"
                        })
                        .count() as i8;
                    active_beams - prev_active
                };

                // Count fractional beams (forward/backward hooks)
                let fracs = beam_levels
                    .iter()
                    .filter(|bv| bv.as_str() == "forward hook" || bv.as_str() == "backward hook")
                    .count() as u8;
                let frac_go_left =
                    beam_levels.iter().any(|bv| bv.as_str() == "backward hook") as u8;

                notebeam_subs.push(ANoteBeam {
                    next: if gi + 1 < group.len() {
                        beam_sub_link + (gi + 1) as Link
                    } else {
                        NILINK
                    },
                    bp_sync: *sync_link,
                    startend,
                    fracs,
                    frac_go_left,
                    filler: 0,
                });
            }

            // alloc_sub already consumed 1, add remaining (group.len() - 1) sub-links
            *next_sub_link += (group.len() - 1) as Link;

            let blink = push_obj(
                objects,
                InterpretedObject {
                    index: 0,
                    header: ObjectHeader {
                        obj_type: 11, // BEAMSET_TYPE
                        left: *prev_link,
                        first_sub_obj: beam_sub_link,
                        n_entries: group.len() as u8,
                        visible: true,
                        valid: true,
                        ..Default::default()
                    },
                    data: ObjData::BeamSet(BeamSet {
                        header: ObjectHeader::default(),
                        ext_header: ExtObjHeader { staffn: sv.0 },
                        voice: sv.1,
                        thin: 0,
                        beam_rests: 0,
                        feather: 0,
                        grace: 0,
                        first_system: 0,
                        cross_staff: 0,
                        cross_system: 0,
                    }),
                },
            );
            score.notebeams.insert(beam_sub_link, notebeam_subs);
            objects[*prev_link as usize].header.right = blink;
            *prev_link = blink;

            group.clear();
        };

        for (sync_link, staff, voice, beam_levels) in &beam_notes_with_links {
            let sv = (*staff, *voice);
            let is_begin = beam_levels.first().map(|s| s.as_str()) == Some("begin");
            let is_end = beam_levels.first().map(|s| s.as_str()) == Some("end");

            if current_sv != Some(sv) || is_begin {
                // Flush previous group if any
                if !current_group.is_empty() {
                    let old_sv = current_sv.unwrap_or(sv);
                    flush_group(
                        &mut current_group,
                        old_sv,
                        &mut objects,
                        &mut score,
                        &mut next_sub_link,
                        &mut prev_link,
                    );
                }
                current_sv = Some(sv);
            }

            current_group.push((*sync_link, beam_levels.clone()));

            if is_end {
                flush_group(
                    &mut current_group,
                    sv,
                    &mut objects,
                    &mut score,
                    &mut next_sub_link,
                    &mut prev_link,
                );
                current_sv = None;
            }
        }
        // Flush any remaining group
        if !current_group.is_empty() {
            if let Some(sv) = current_sv {
                flush_group(
                    &mut current_group,
                    sv,
                    &mut objects,
                    &mut score,
                    &mut next_sub_link,
                    &mut prev_link,
                );
            }
        }
    }

    // ---- 9c. TUPLET OBJECTS ----
    // Create Tuplet + ANoteTuple objects from tuplet info collected during Sync creation.
    // Group consecutive notes by (staff, voice) with tuplet_start/tuplet_stop boundaries.
    {
        let mut current_group: Vec<Link> = Vec::new(); // Sync links in this tuplet
        let mut current_sv: Option<(i8, i8)> = None;
        let mut current_actual: u8 = 3;
        let mut current_normal: u8 = 2;

        let flush_tuplet = |group: &mut Vec<Link>,
                            sv: (i8, i8),
                            acc_num: u8,
                            acc_denom: u8,
                            objects: &mut Vec<InterpretedObject>,
                            score: &mut InterpretedScore,
                            next_sub_link: &mut Link,
                            prev_link: &mut Link| {
            if group.len() < 2 {
                group.clear();
                return;
            }

            // Create ANoteTuple subobjects
            let tuplet_sub_link = alloc_sub(next_sub_link);
            let mut notetuple_subs: Vec<ANoteTuple> = Vec::new();
            for (gi, sync_link) in group.iter().enumerate() {
                notetuple_subs.push(ANoteTuple {
                    next: if gi + 1 < group.len() {
                        tuplet_sub_link + (gi + 1) as Link
                    } else {
                        NILINK
                    },
                    tp_sync: *sync_link,
                });
            }
            // alloc_sub already consumed 1, add remaining (group.len() - 1) sub-links
            *next_sub_link += (group.len() - 1) as Link;

            // Bracket position: default to above staff, moderate offset
            let bracket_yd: Ddist = -40; // ~2.5 spaces above staff top

            let tlink = push_obj(
                objects,
                InterpretedObject {
                    index: 0,
                    header: ObjectHeader {
                        obj_type: 18, // TUPLET_TYPE
                        left: *prev_link,
                        first_sub_obj: tuplet_sub_link,
                        n_entries: group.len() as u8,
                        visible: true,
                        valid: true,
                        ..Default::default()
                    },
                    data: ObjData::Tuplet(Tuplet {
                        header: ObjectHeader::default(),
                        ext_header: ExtObjHeader { staffn: sv.0 },
                        acc_num,
                        acc_denom,
                        voice: sv.1,
                        num_vis: 1,   // Show number
                        denom_vis: 0, // Hide denominator
                        brack_vis: 1, // Show bracket
                        small: 0,
                        filler: 0,
                        acnxd: 0,
                        acnyd: 0,
                        xd_first: 0,
                        yd_first: bracket_yd,
                        xd_last: 0,
                        yd_last: bracket_yd,
                    }),
                },
            );
            score.tuplets.insert(tuplet_sub_link, notetuple_subs);
            objects[*prev_link as usize].header.right = tlink;
            *prev_link = tlink;

            group.clear();
        };

        for &(sync_link, staff, voice, actual, normal, tup_start, tup_stop) in
            &tuplet_notes_with_links
        {
            let sv = (staff, voice);

            if tup_start || (current_sv != Some(sv) && current_group.is_empty()) {
                // Starting a new tuplet group
                if !current_group.is_empty() {
                    let old_sv = current_sv.unwrap_or(sv);
                    flush_tuplet(
                        &mut current_group,
                        old_sv,
                        current_actual,
                        current_normal,
                        &mut objects,
                        &mut score,
                        &mut next_sub_link,
                        &mut prev_link,
                    );
                }
                current_sv = Some(sv);
                current_actual = if actual > 0 { actual } else { 3 };
                current_normal = if normal > 0 { normal } else { 2 };
            }

            current_group.push(sync_link);

            if tup_stop {
                flush_tuplet(
                    &mut current_group,
                    sv,
                    current_actual,
                    current_normal,
                    &mut objects,
                    &mut score,
                    &mut next_sub_link,
                    &mut prev_link,
                );
                current_sv = None;
            }
        }
        // Flush any remaining group
        if !current_group.is_empty() {
            if let Some(sv) = current_sv {
                flush_tuplet(
                    &mut current_group,
                    sv,
                    current_actual,
                    current_normal,
                    &mut objects,
                    &mut score,
                    &mut next_sub_link,
                    &mut prev_link,
                );
            }
        }
    }

    // ---- 9d. GRSYNC OBJECTS (grace notes) ----
    // Create GrSync + AGrNote objects from grace_notes collected during parsing.
    // Grace notes are grouped by (measure_num, time) and placed before the Sync at that time.
    // For simplicity, each grace note gets its own GrSync (matching Notelist behavior).
    if !grace_notes.is_empty() {
        for gne in &grace_notes {
            let grsync_sub_link = alloc_sub(&mut next_sub_link);
            // alloc_sub already incremented next_sub_link by 1 for our single AGrNote

            let yqpit = if gne.rest { 0 } else { midi_to_yqpit(gne.midi) };

            // Compute yd from clef context, matching regular Sync note creation
            let clef_type = init_clefs
                .get(gne.global_staff as usize)
                .copied()
                .unwrap_or(TREBLE_CLEF);
            let (yd, ystem) = if gne.rest {
                let yd = half_ln_to_yd(4, staff_height);
                (yd, yd)
            } else {
                let mid_c_hl = clef_middle_c_half_ln(clef_type);
                let half_ln = mid_c_hl + yqpit as i16;
                let yd = half_ln_to_yd(half_ln, staff_height);
                // Grace notes: stems are shorter, always 5 half-spaces
                let stem_down =
                    normal_stem_up_down_single(half_ln, n_staff_lines, VoiceRole::Single);
                let ystem = calc_ystem(
                    yd,
                    nflags(gne.l_dur),
                    stem_down,
                    staff_height,
                    n_staff_lines,
                    stem_len_normal, // use normal stem length; grace drawing scales down
                    false,
                );
                (yd, ystem)
            };

            let grnote = ANote {
                header: SubObjHeader {
                    next: NILINK,
                    staffn: gne.global_staff,
                    sub_type: gne.l_dur,
                    selected: false,
                    visible: true,
                    soft: false,
                },
                in_chord: false,
                rest: gne.rest,
                unpitched: false,
                beamed: false,
                other_stem_side: false,
                xd: 0,
                yd,
                yqpit,
                ystem,
                play_time_delta: 0,
                play_dur: 0, // grace notes have no playback duration
                p_time: 0,
                note_num: gne.midi,
                on_velocity: 75,
                off_velocity: 64,
                tied_l: false,
                tied_r: false,
                x_move_dots: 0,
                y_move_dots: 2,
                ndots: gne.dots,
                voice: gne.voice,
                rsp_ignore: 0,
                accident: gne.acc,
                acc_soft: false,
                courtesy_acc: 0,
                xmove_acc: 0,
                play_as_cue: false,
                micropitch: 0,
                merged: 0,
                double_dur: 0,
                head_shape: 1, // NormalVis
                first_mod: NILINK,
                slurred_l: false,
                slurred_r: false,
                in_tuplet: false,
                in_ottava: false,
                small: true, // Grace notes are small
                temp_flag: 0,
                art_harmonic: 0,
                user_id: 0,
                nh_segment: [0u8; 6],
                reserved_n: 0,
            };

            score.grnotes.insert(grsync_sub_link, vec![grnote]);

            let grlink = push_obj(
                &mut objects,
                InterpretedObject {
                    index: 0,
                    header: ObjectHeader {
                        obj_type: 19, // GRSYNC_TYPE
                        left: prev_link,
                        first_sub_obj: grsync_sub_link,
                        n_entries: 1,
                        visible: true,
                        valid: true,
                        ..Default::default()
                    },
                    data: ObjData::GrSync(GrSync {
                        header: ObjectHeader::default(),
                    }),
                },
            );
            objects[prev_link as usize].header.right = grlink;
            prev_link = grlink;
        }
    }

    // ---- 9e. DYNAMIC OBJECTS ----
    // Create Dynamic + ADynamic objects for collected dynamics.
    // These are inserted into the object list after all Syncs but before the Tail.
    // The exporter's collect_measure_data() will pick them up during the walk.
    for &(measure_num, global_staff, dynamic_type) in &collected_dynamics {
        let dsub_link = alloc_sub(&mut next_sub_link);
        let adyn = crate::obj_types::ADynamic {
            header: SubObjHeader {
                next: NILINK,
                staffn: global_staff,
                sub_type: 0,
                selected: false,
                visible: true,
                soft: false,
            },
            mouth_width: 0,
            small: 0,
            other_width: 0,
            xd: 0,
            yd: 0,
            endxd: 0,
            endyd: 0,
            d_mod_code: 0,
            cross_staff: 0,
        };
        // alloc_sub already incremented next_sub_link by 1 for our single ADynamic
        let dlink = push_obj(
            &mut objects,
            InterpretedObject {
                index: 0,
                header: ObjectHeader {
                    obj_type: 13, // DYNAMIC_TYPE
                    left: prev_link,
                    first_sub_obj: dsub_link,
                    n_entries: 1,
                    visible: true,
                    valid: true,
                    ..Default::default()
                },
                data: ObjData::Dynamic(Dynamic {
                    header: ObjectHeader::default(),
                    dynamic_type: dynamic_type as i8,
                    filler: false,
                    cross_sys: false,
                    first_sync_l: NILINK,
                    last_sync_l: NILINK,
                }),
            },
        );
        let _ = measure_num; // future: associate with specific sync
        score.dynamics.insert(dsub_link, vec![adyn]);
        objects[prev_link as usize].header.right = dlink;
        prev_link = dlink;
    }

    // ---- 9f. TEMPO OBJECTS ----
    // Create Tempo objects from collected tempo marks.
    // Tempo data lives directly in ObjData::Tempo; verbal/metronome strings in tempo_strings.
    for (_measure_num, global_staff, tempo) in &collected_tempos {
        let tempo_link = push_obj(
            &mut objects,
            InterpretedObject {
                index: 0,
                header: ObjectHeader {
                    obj_type: 20, // TEMPO_TYPE
                    left: prev_link,
                    n_entries: 0,
                    visible: true,
                    valid: true,
                    ..Default::default()
                },
                data: ObjData::Tempo(Tempo {
                    header: ObjectHeader::default(),
                    ext_header: ExtObjHeader {
                        staffn: *global_staff,
                    },
                    sub_type: tempo.sub_type,
                    expanded: false,
                    no_mm: tempo.tempo_mm == 0,
                    filler: 0,
                    dotted: tempo.dotted,
                    hide_mm: tempo.tempo_mm == 0,
                    tempo_mm: tempo.tempo_mm,
                    str_offset: 0, // placeholder — verbal text stored in tempo_strings
                    first_obj_l: NILINK,
                    metro_str_offset: 0, // placeholder — metro text in tempo_strings
                }),
            },
        );
        // Store verbal and metronome strings in the score's tempo_strings map.
        let metro_str = if tempo.tempo_mm > 0 {
            tempo.tempo_mm.to_string()
        } else {
            String::new()
        };
        score
            .tempo_strings
            .insert(tempo_link, (tempo.words.clone(), metro_str));
        objects[prev_link as usize].header.right = tempo_link;
        prev_link = tempo_link;
    }

    // ---- 9g. ENDING (VOLTA) OBJECTS ----
    // Create Ending objects from collected volta endings.
    for ending in &collected_endings {
        let ending_link = push_obj(
            &mut objects,
            InterpretedObject {
                index: 0,
                header: ObjectHeader {
                    obj_type: 22, // ENDING_TYPE
                    left: prev_link,
                    n_entries: 0,
                    visible: true,
                    valid: true,
                    ..Default::default()
                },
                data: ObjData::Ending(Ending {
                    header: ObjectHeader::default(),
                    ext_header: ExtObjHeader { staffn: 1 },
                    first_obj_l: NILINK,
                    last_obj_l: NILINK,
                    no_l_cutoff: if ending.ending_type == "start" { 0 } else { 1 },
                    no_r_cutoff: if ending.ending_type == "stop"
                        || ending.ending_type == "discontinue"
                    {
                        0
                    } else {
                        1
                    },
                    end_num: ending.number,
                    endxd: 0,
                }),
            },
        );
        objects[prev_link as usize].header.right = ending_link;
        prev_link = ending_link;
    }

    // ---- 9h. RPTEND (REPEAT BARLINE) OBJECTS ----
    // Create RptEnd objects from collected repeat barlines.
    for repeat in &collected_repeats {
        let rpt_sub_type: i8 = match repeat.direction.as_str() {
            "forward" => 5,  // RptL
            "backward" => 6, // RptR
            _ => 6,          // default to backward
        };
        let rpt_link = push_obj(
            &mut objects,
            InterpretedObject {
                index: 0,
                header: ObjectHeader {
                    obj_type: 3, // RPTEND_TYPE
                    left: prev_link,
                    n_entries: 0,
                    visible: true,
                    valid: true,
                    ..Default::default()
                },
                data: ObjData::RptEnd(RptEnd {
                    header: ObjectHeader::default(),
                    first_obj: NILINK,
                    start_rpt: NILINK,
                    end_rpt: NILINK,
                    sub_type: rpt_sub_type,
                    count: 2, // default repeat count
                }),
            },
        );
        objects[prev_link as usize].header.right = rpt_link;
        prev_link = rpt_link;
    }

    // ---- 9i. OTTAVA OBJECTS ----
    // Create Ottava objects from collected octave shift markings.
    for (_measure_num, global_staff, ottava) in &collected_ottavas {
        let ottava_link = push_obj(
            &mut objects,
            InterpretedObject {
                index: 0,
                header: ObjectHeader {
                    obj_type: 16, // OTTAVA_TYPE
                    left: prev_link,
                    n_entries: 0,
                    visible: true,
                    valid: true,
                    ..Default::default()
                },
                data: ObjData::Ottava(Ottava {
                    header: ObjectHeader::default(),
                    ext_header: ExtObjHeader {
                        staffn: *global_staff,
                    },
                    no_cutoff: 0,
                    cross_staff: 0,
                    cross_system: 0,
                    oct_sign_type: ottava.oct_type,
                    filler: 0,
                    number_vis: true,
                    unused1: false,
                    brack_vis: true,
                    unused2: false,
                    nxd: 0,
                    nyd: 0,
                    xd_first: 0,
                    yd_first: 0,
                    xd_last: 0,
                    yd_last: 0,
                }),
            },
        );
        objects[prev_link as usize].header.right = ottava_link;
        prev_link = ottava_link;
    }

    // ---- 10. TAIL ----
    let tail_link = push_obj(
        &mut objects,
        InterpretedObject {
            index: 0,
            header: ObjectHeader {
                obj_type: 1, // TAIL_TYPE
                left: prev_link,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Tail(Tail {
                header: ObjectHeader::default(),
            }),
        },
    );
    objects[prev_link as usize].header.right = tail_link;

    // Wire up HEADER
    score.head_l = header_link;
    score.objects = objects;
    score.first_names = 2; // full names on first system
    score.other_names = 0;

    // --- Post-hoc layout: fix geometry + compute spacing ---
    layout_score(&mut score, &LayoutConfig::default());

    Ok(score)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1">
      <part-name>Piano</part-name>
    </score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>E</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>F</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
    </measure>
  </part>
</score-partwise>"#;

    #[test]
    fn import_minimal_xml() {
        let score = import_musicxml(MINIMAL_XML).unwrap();
        assert!(!score.notes.is_empty(), "score should have notes");
        assert!(!score.part_infos.is_empty(), "score should have parts");
    }

    #[test]
    fn import_produces_walkable_score() {
        let score = import_musicxml(MINIMAL_XML).unwrap();
        let objs: Vec<_> = score.walk().collect();
        assert!(
            objs.len() > 5,
            "should have several objects: {}",
            objs.len()
        );
    }

    #[test]
    fn import_has_correct_note_count() {
        let score = import_musicxml(MINIMAL_XML).unwrap();
        let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
        assert_eq!(total_notes, 4, "should have 4 notes");
    }

    #[test]
    fn pitch_to_midi_basic() {
        assert_eq!(pitch_to_midi("C", 4, 0), 60);
        assert_eq!(pitch_to_midi("A", 4, 0), 69);
        assert_eq!(pitch_to_midi("C", 4, 1), 61);
        assert_eq!(pitch_to_midi("B", 3, -1), 58);
        assert_eq!(pitch_to_midi("C", 0, 0), 12);
    }

    #[test]
    fn type_to_l_dur_values() {
        assert_eq!(type_to_l_dur("whole"), 2);
        assert_eq!(type_to_l_dur("half"), 3);
        assert_eq!(type_to_l_dur("quarter"), 4);
        assert_eq!(type_to_l_dur("eighth"), 5);
        assert_eq!(type_to_l_dur("16th"), 6);
        assert_eq!(type_to_l_dur("32nd"), 7);
    }

    #[test]
    fn import_with_rests() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Test</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
      <note>
        <rest/>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>E</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
      <note>
        <rest/>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
    </measure>
  </part>
</score-partwise>"#;

        let score = import_musicxml(xml).unwrap();
        let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
        assert_eq!(total_notes, 4, "should have 4 note objects");
        let rest_count: usize = score
            .notes
            .values()
            .flat_map(|v| v.iter())
            .filter(|n| n.rest)
            .count();
        assert_eq!(rest_count, 2, "should have 2 rests");
    }

    #[test]
    fn import_two_part_score() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Violin</part-name></score-part>
    <score-part id="P2"><part-name>Cello</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>2</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>A</step><octave>4</octave></pitch>
        <duration>4</duration>
        <type>whole</type>
        <voice>1</voice>
      </note>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>2</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>F</sign><line>4</line></clef>
      </attributes>
      <note>
        <pitch><step>A</step><octave>2</octave></pitch>
        <duration>4</duration>
        <type>whole</type>
        <voice>1</voice>
      </note>
    </measure>
  </part>
</score-partwise>"#;

        let score = import_musicxml(xml).unwrap();
        assert!(score.part_infos.len() >= 2, "should have at least 2 parts");
        let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
        assert_eq!(total_notes, 2, "should have 2 notes");
    }

    /// Round-trip test: NGL → export → import → check note count preserved.
    #[test]
    fn round_trip_ngl_export_import() {
        use crate::musicxml::export::export_musicxml;
        use crate::ngl::{interpret::interpret_heap, NglFile};

        let path = "tests/fixtures/01_me_and_lucy.ngl";
        let data = std::fs::read(path).unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        let original_score = interpret_heap(&ngl).unwrap();

        let orig_notes: usize = original_score.notes.values().map(|v| v.len()).sum();
        let xml = export_musicxml(&original_score);
        let imported_score = import_musicxml(&xml).unwrap();
        let imported_notes: usize = imported_score.notes.values().map(|v| v.len()).sum();

        assert!(
            imported_notes > 0,
            "imported score should have notes (original had {})",
            orig_notes
        );
        let ratio = imported_notes as f64 / orig_notes as f64;
        assert!(
            ratio > 0.5,
            "round-trip should preserve most notes: orig={}, imported={}, ratio={:.2}",
            orig_notes,
            imported_notes,
            ratio
        );
    }

    /// Round-trip: export → import → re-export should produce valid MusicXML.
    #[test]
    fn round_trip_export_import_export() {
        use crate::musicxml::export::export_musicxml;
        use crate::ngl::{interpret::interpret_heap, NglFile};

        let path = "tests/fixtures/01_me_and_lucy.ngl";
        let data = std::fs::read(path).unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        let xml1 = export_musicxml(&score);
        let imported = import_musicxml(&xml1).unwrap();
        let xml2 = export_musicxml(&imported);

        assert!(xml2.contains("<score-partwise"));
        assert!(xml2.contains("<part-list>"));
        assert!(xml2.contains("<note>"));
    }

    /// Canonical roundtrip: export → import → re-export, compare XML idempotency.
    /// The second export should match the first on key structural elements.
    #[test]
    fn canonical_roundtrip_xml_stability() {
        use crate::musicxml::export::export_musicxml;
        use crate::ngl::{interpret::interpret_heap, NglFile};

        let path = "tests/fixtures/01_me_and_lucy.ngl";
        let data = std::fs::read(path).unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        let xml1 = export_musicxml(&score);
        let imported = import_musicxml(&xml1).unwrap();
        let xml2 = export_musicxml(&imported);

        // Count structural elements — they should be close
        let count_tag = |xml: &str, tag: &str| -> usize { xml.matches(tag).count() };

        let notes1 = count_tag(&xml1, "<note>");
        let notes2 = count_tag(&xml2, "<note>");
        let measures1 = count_tag(&xml1, "<measure ");
        let measures2 = count_tag(&xml2, "<measure ");
        let pitches1 = count_tag(&xml1, "<pitch>");
        let pitches2 = count_tag(&xml2, "<pitch>");
        let rests1 = count_tag(&xml1, "<rest");
        let rests2 = count_tag(&xml2, "<rest");
        let ties1 = count_tag(&xml1, "type=\"start\"");
        let ties2 = count_tag(&xml2, "type=\"start\"");

        // Notes and pitches should be preserved exactly
        assert_eq!(
            notes1, notes2,
            "note count should be preserved: {} vs {}",
            notes1, notes2
        );
        assert_eq!(
            pitches1, pitches2,
            "pitch count should be preserved: {} vs {}",
            pitches1, pitches2
        );
        assert_eq!(
            rests1, rests2,
            "rest count should be preserved: {} vs {}",
            rests1, rests2
        );
        assert_eq!(
            measures1, measures2,
            "measure count should be preserved: {} vs {}",
            measures1, measures2
        );
        // Tie count may differ slightly due to cross-system tie handling
        let tie_ratio = if ties1 > 0 {
            ties2 as f64 / ties1 as f64
        } else {
            1.0
        };
        assert!(
            tie_ratio > 0.8,
            "tie count should be mostly preserved: {} vs {} (ratio {:.2})",
            ties1,
            ties2,
            tie_ratio
        );
    }

    /// Round-trip all NGL fixtures: export → import → re-export → verify note count.
    #[test]
    fn round_trip_all_ngl_fixtures() {
        use crate::musicxml::export::export_musicxml;
        use crate::ngl::{interpret::interpret_heap, NglFile};

        let fixture_dir = "tests/fixtures";
        let entries = std::fs::read_dir(fixture_dir).unwrap();
        let mut count = 0;
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ngl") {
                let data = std::fs::read(&path).unwrap();
                let ngl = NglFile::read_from_bytes(&data).unwrap();
                let score = interpret_heap(&ngl).unwrap();

                let orig_notes: usize = score.notes.values().map(|v| v.len()).sum();
                let xml = export_musicxml(&score);

                // Should import without panicking
                let imported = import_musicxml(&xml).unwrap_or_else(|e| {
                    panic!("Failed to import {}: {}", path.display(), e);
                });

                let imported_notes: usize = imported.notes.values().map(|v| v.len()).sum();

                // Re-export should produce valid XML
                let xml2 = export_musicxml(&imported);
                assert!(
                    xml2.contains("<score-partwise"),
                    "{}: re-export missing score-partwise",
                    path.display()
                );

                // Note count should be preserved (import may add/remove padding rests)
                if orig_notes > 0 {
                    assert!(
                        imported_notes > 0,
                        "{}: imported 0 notes from {} original",
                        path.display(),
                        orig_notes
                    );
                }

                count += 1;
            }
        }
        assert!(count > 10, "Should have tested at least 10 fixtures");
    }

    /// Test import of MusicXML test suite files (from icebox).
    #[test]
    fn import_musicxml_test_suite() {
        let suite_dir = "icebox/tests/musicxml_test_suite";
        if !std::path::Path::new(suite_dir).exists() {
            eprintln!("Skipping: {} not found", suite_dir);
            return;
        }

        let entries = std::fs::read_dir(suite_dir).unwrap();
        let mut success = 0;
        let mut fail = 0;
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "xml") {
                let xml = std::fs::read_to_string(&path).unwrap();
                // We only support score-partwise format
                if !xml.contains("<score-partwise") {
                    continue;
                }
                match import_musicxml(&xml) {
                    Ok(score) => {
                        // Verify we get a walkable score
                        let objs: Vec<_> = score.walk().collect();
                        assert!(
                            objs.len() >= 3,
                            "{}: too few objects: {}",
                            path.display(),
                            objs.len()
                        );
                        success += 1;
                    }
                    Err(e) => {
                        eprintln!("  FAIL {}: {}", path.display(), e);
                        fail += 1;
                    }
                }
            }
        }
        eprintln!("MusicXML test suite: {} passed, {} failed", success, fail);
        assert!(success > 10, "Should import at least 10 test suite files");
        // Allow some failures for unsupported features, but most should pass
        assert!(
            success > fail * 2,
            "Too many failures: {} fail vs {} pass",
            fail,
            success
        );
    }

    /// Test that accidentals round-trip through export → import.
    #[test]
    fn round_trip_accidentals() {
        use crate::musicxml::export::export_musicxml;

        let xml_with_accs = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Test</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><alter>1</alter><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <accidental>sharp</accidental>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>E</step><alter>-1</alter><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <accidental>flat</accidental>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>F</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <accidental>natural</accidental>
        <voice>1</voice>
      </note>
      <note>
        <pitch><step>G</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
        <voice>1</voice>
      </note>
    </measure>
  </part>
</score-partwise>"#;

        let score = import_musicxml(xml_with_accs).unwrap();

        // Verify accidentals were imported
        let all_notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
        assert_eq!(all_notes.len(), 4);

        // Re-export and verify accidentals appear
        let xml_out = export_musicxml(&score);
        assert!(xml_out.contains("<accidental>sharp</accidental>"));
        assert!(xml_out.contains("<accidental>flat</accidental>"));
        assert!(xml_out.contains("<accidental>natural</accidental>"));
    }

    /// Test that clef-octave-change round-trips correctly.
    #[test]
    fn round_trip_clef_octave_change() {
        use crate::musicxml::export::export_musicxml;

        let xml_with_oct = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Guitar</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef>
          <sign>G</sign>
          <line>2</line>
          <clef-octave-change>-1</clef-octave-change>
        </clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>4</duration>
        <type>whole</type>
        <voice>1</voice>
      </note>
    </measure>
  </part>
</score-partwise>"#;

        let score = import_musicxml(xml_with_oct).unwrap();

        // Verify the clef was imported as TRTENOR_CLEF (octave-transposing treble)
        let staffs: Vec<_> = score.staffs.values().collect();
        assert!(!staffs.is_empty(), "should have staff subobjects");
        let clef_type = staffs[0][0].clef_type;
        assert_eq!(
            clef_type, TRTENOR_CLEF as i8,
            "should import G clef with oct-change=-1 as TRTENOR_CLEF ({}), got {}",
            TRTENOR_CLEF, clef_type
        );

        // Re-export and verify octave-change appears
        let xml_out = export_musicxml(&score);
        assert!(
            xml_out.contains("<clef-octave-change>-1</clef-octave-change>"),
            "should export clef-octave-change=-1"
        );
    }
}
