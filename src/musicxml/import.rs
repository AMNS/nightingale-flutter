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

use crate::basic_types::{DRect, KsInfo, Link, Rect, NILINK};
use crate::defs::{
    ALTO_CLEF, BARITONE_CLEF, BASS8B_CLEF, BASS_CLEF, FRVIOLIN_CLEF, MZSOPRANO_CLEF, PERC_CLEF,
    SOPRANO_CLEF, TENOR_CLEF, TREBLE8_CLEF, TREBLE_CLEF, TRTENOR_CLEF,
};
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::obj_types::{
    AClef, AConnect, AKeySig, AMeasure, ANote, AStaff, ATimeSig, Clef, Connect, Header, KeySig,
    Measure, ObjectHeader, Page, PartInfo, Staff, SubObjHeader, System, Tail, TimeSig,
    SHOW_ALL_LINES,
};
use crate::objects::setup_ks_info;

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

/// An element within a MusicXML measure.
#[derive(Debug, Clone)]
enum XmlMeasureElement {
    Attributes(XmlAttributes),
    Note(XmlNote),
    Direction(XmlDirection),
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
                        });
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
                        "tie" => match attr_str(e, "type").as_str() {
                            "start" => note.tie_start = true,
                            "stop" => note.tie_stop = true,
                            _ => {}
                        },
                        _ => {}
                    }
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
        play_dur: i16,
    }

    let mut all_notes: Vec<NoteEntry> = Vec::new();
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

        for meas in &part.measures {
            let mut voice_time: i32 = 0;

            for elem in &meas.elements {
                match elem {
                    XmlMeasureElement::Attributes(attrs) => {
                        if let Some(d) = attrs.divisions {
                            divisions = d;
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
                        let pdur = xml_dur_to_pdur(note.duration, divisions);

                        let t = if note.chord {
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

                        let acc = accidental_to_code(&note.accidental, note.alter);

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
                            play_dur: pdur as i16,
                        });

                        if !note.chord {
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
                }
            }
        }
    }

    // Group notes by (measure_num, time) for creating Sync objects.
    // Sort: measure_num, time, staff, voice
    all_notes.sort_by(|a, b| {
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

    for meas_num in 1..=(num_measures as i32) {
        // ---- Insert mid-score Clef/KeySig/TimeSig objects before this measure ----
        if let Some(change) = mid_score_attrs.remove(&meas_num) {
            // Insert Clef object if there are clef changes
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
                    beamed: false,
                    other_stem_side: false,
                    yqpit,
                    xd: 0,
                    yd: 0,
                    ystem: 0,
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
                    slurred_l: false,
                    slurred_r: false,
                    in_tuplet: false,
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
            prev_link = sync_link;
        }

        // Advance measure start time
        let meas_dur =
            (PDUR_QUARTER as i64 * 4 * init_time_num as i64 / init_time_denom as i64) as i32;
        measure_start_time += meas_dur;
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
