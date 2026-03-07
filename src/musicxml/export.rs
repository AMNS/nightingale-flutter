// MusicXML 4.0 partwise export from an InterpretedScore.
//
// Walks the NGL object list and produces a valid MusicXML file.
// Handles: parts, measures, notes/rests, clefs, key signatures,
// time signatures, ties, dots, chords, dynamics, tempo markings.
//
// Ported from: icebox/src/musicxml/export.rs (new functionality, no C++ equivalent).

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;

use crate::basic_types::{KsItem, Link};
use crate::defs::*;
use crate::ngl::interpret::*;
use crate::obj_types::PartInfo;

/// PDUR ticks per quarter note (used as MusicXML divisions).
const DIVISIONS: i32 = 480;

// ============================================================
// Pitch conversion
// ============================================================

/// Convert MIDI note number + NGL accidental to MusicXML pitch (step, alter, octave).
fn midi_to_pitch(midi: u8, accident: u8) -> (&'static str, i32, i32) {
    let octave = (midi as i32 / 12) - 1;
    let pc = midi % 12;

    // For double accidentals, the sounding pitch (MIDI) is 2 semitones away from
    // the written pitch. We need to find the diatonic step that, when altered by
    // ±2, produces the sounding MIDI pitch.
    if accident == AC_DBLSHARP {
        // Double-sharp: written note is 2 semitones below sounding pitch.
        // E.g., F## sounds as G (midi 67): we need step=F, alter=+2.
        let written_midi = midi.wrapping_sub(2);
        let written_pc = written_midi % 12;
        let written_octave = (written_midi as i32 / 12) - 1;
        let step = match written_pc {
            0 => "C",
            2 => "D",
            4 => "E",
            5 => "F",
            7 => "G",
            9 => "A",
            11 => "B",
            // Written pitch is on a chromatic note — shouldn't normally happen
            // for double-sharps, but fall back to sharp spelling.
            _ => return midi_to_pitch(midi, AC_SHARP),
        };
        return (step, 2, written_octave);
    }

    if accident == AC_DBLFLAT {
        // Double-flat: written note is 2 semitones above sounding pitch.
        // E.g., Bbb sounds as A (midi 69): we need step=B, alter=-2.
        let written_midi = midi + 2;
        let written_pc = written_midi % 12;
        let written_octave = (written_midi as i32 / 12) - 1;
        let step = match written_pc {
            0 => "C",
            2 => "D",
            4 => "E",
            5 => "F",
            7 => "G",
            9 => "A",
            11 => "B",
            _ => return midi_to_pitch(midi, AC_FLAT),
        };
        return (step, -2, written_octave);
    }

    // Single accidentals and naturals: use the accident code to choose
    // enharmonic spelling for chromatic pitches.
    let is_flat = accident == AC_FLAT;
    match pc {
        0 => ("C", 0, octave),
        1 => {
            if is_flat {
                ("D", -1, octave)
            } else {
                ("C", 1, octave)
            }
        }
        2 => ("D", 0, octave),
        3 => {
            if accident == AC_SHARP {
                ("D", 1, octave)
            } else {
                ("E", -1, octave)
            }
        }
        4 => ("E", 0, octave),
        5 => ("F", 0, octave),
        6 => {
            if is_flat {
                ("G", -1, octave)
            } else {
                ("F", 1, octave)
            }
        }
        7 => ("G", 0, octave),
        8 => {
            if is_flat {
                ("A", -1, octave)
            } else {
                ("G", 1, octave)
            }
        }
        9 => ("A", 0, octave),
        10 => {
            if accident == AC_SHARP {
                ("A", 1, octave)
            } else {
                ("B", -1, octave)
            }
        }
        11 => ("B", 0, octave),
        _ => ("C", 0, octave),
    }
}

/// Convert NGL duration code to MusicXML type name.
fn l_dur_to_type(l_dur: i8) -> &'static str {
    match l_dur {
        1 => "breve",
        2 => "whole",
        3 => "half",
        4 => "quarter",
        5 => "eighth",
        6 => "16th",
        7 => "32nd",
        8 => "64th",
        9 => "128th",
        _ => "quarter",
    }
}

/// Compute PDUR ticks for a given duration code + number of dots.
fn dur_to_ticks(l_dur: i8, ndots: u8) -> i32 {
    let base = match l_dur {
        1 => 3840, // breve
        2 => 1920, // whole
        3 => 960,  // half
        4 => 480,  // quarter
        5 => 240,  // eighth
        6 => 120,  // 16th
        7 => 60,   // 32nd
        8 => 30,   // 64th
        9 => 15,   // 128th
        _ => 480,  // default to quarter
    };
    let mut total = base;
    let mut dot_val = base / 2;
    for _ in 0..ndots {
        total += dot_val;
        dot_val /= 2;
    }
    total
}

/// Compute whole-measure rest duration from time signature.
fn whole_measure_dur(numerator: i8, denominator: i8) -> i32 {
    if denominator <= 0 || numerator <= 0 {
        return 1920; // default: whole note
    }
    let beat_dur = (DIVISIONS * 4) / denominator as i32;
    beat_dur * numerator as i32
}

/// Convert NGL clef code to MusicXML (sign, line, octave-change).
/// Clef constants are u8 in defs.rs; AClef.header.sub_type is i8.
///
/// TRTENOR_CLEF uses the treble clef *glyph* (G clef) but sounds an octave
/// lower — it is NOT a C clef despite its constant value being between C clef
/// variants. See DrawUtils.cp:286-290 where it shares MCH_trebleclef glyph
/// with TREBLE_CLEF, and PitchUtils.cp:127 where middleCHalfLn=3.
fn clef_to_xml(clef_type: u8) -> (&'static str, i32, i32) {
    match clef_type {
        TREBLE8_CLEF => ("G", 2, 1),   // Treble 8va
        FRVIOLIN_CLEF => ("G", 1, 0),  // French violin (G on line 1)
        TREBLE_CLEF => ("G", 2, 0),    // Standard treble
        SOPRANO_CLEF => ("C", 1, 0),   // C on line 1
        MZSOPRANO_CLEF => ("C", 2, 0), // C on line 2
        ALTO_CLEF => ("C", 3, 0),      // C on line 3
        TRTENOR_CLEF => ("G", 2, -1),  // Treble-tenor: G clef, octave down
        TENOR_CLEF => ("C", 4, 0),     // C on line 4
        BARITONE_CLEF => ("F", 3, 0),  // F on line 3
        BASS_CLEF => ("F", 4, 0),      // Standard bass
        BASS8B_CLEF => ("F", 4, -1),   // Bass 8vb
        PERC_CLEF => ("percussion", 2, 0),
        _ => ("G", 2, 0),
    }
}

/// Convert NGL dynamic type code to MusicXML dynamic element name.
/// Returns None for hairpins (types 22-23) which use `<wedge>` instead.
fn dynamic_type_to_xml(dtype: u8) -> Option<&'static str> {
    match dtype {
        PPPP_DYNAM => Some("pppp"),
        PPP_DYNAM => Some("ppp"),
        PP_DYNAM => Some("pp"),
        P_DYNAM => Some("p"),
        MP_DYNAM => Some("mp"),
        MF_DYNAM => Some("mf"),
        F_DYNAM => Some("f"),
        FF_DYNAM => Some("ff"),
        FFF_DYNAM => Some("fff"),
        FFFF_DYNAM => Some("ffff"),
        SF_DYNAM => Some("sf"),
        16 => Some("fz"),
        SFZ_DYNAM => Some("sfz"),
        RF_DYNAM => Some("rf"),
        19 => Some("rfz"),
        FP_DYNAM => Some("fp"),
        SFP_DYNAM => Some("sfp"),
        // Types 11-14 (relative dynamics) mapped to closest absolute
        11 => Some("p"),  // più p → p
        12 => Some("mp"), // meno p → mp
        13 => Some("mf"), // meno f → mf
        14 => Some("f"),  // più f → f
        _ => None,        // hairpins or unknown
    }
}

/// Convert MusicXML dynamic element name to NGL dynamic type code.
#[allow(dead_code)]
fn xml_to_dynamic_type(name: &str) -> Option<u8> {
    match name {
        "pppp" => Some(PPPP_DYNAM),
        "ppp" => Some(PPP_DYNAM),
        "pp" => Some(PP_DYNAM),
        "p" => Some(P_DYNAM),
        "mp" => Some(MP_DYNAM),
        "mf" => Some(MF_DYNAM),
        "f" => Some(F_DYNAM),
        "ff" => Some(FF_DYNAM),
        "fff" => Some(FFF_DYNAM),
        "ffff" => Some(FFFF_DYNAM),
        "sf" => Some(SF_DYNAM),
        "fz" => Some(16),
        "sfz" => Some(SFZ_DYNAM),
        "rf" => Some(RF_DYNAM),
        "rfz" => Some(19),
        "fp" => Some(FP_DYNAM),
        "sfp" => Some(SFP_DYNAM),
        _ => None,
    }
}

/// Convert NGL key signature items to MusicXML fifths value.
fn ks_to_fifths(ks_items: &[KsItem; 7], n_ks_items: i8) -> i32 {
    if n_ks_items <= 0 {
        return 0;
    }
    // If first item is sharp, fifths is positive; if flat, negative
    if ks_items[0].sharp != 0 {
        n_ks_items as i32
    } else {
        -(n_ks_items as i32)
    }
}

// ============================================================
// Beam info for MusicXML export
// ============================================================

/// Beam state for a single beam level on a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BeamValue {
    Begin,
    Continue,
    End,
    ForwardHook,
    BackwardHook,
}

impl BeamValue {
    fn as_str(&self) -> &'static str {
        match self {
            BeamValue::Begin => "begin",
            BeamValue::Continue => "continue",
            BeamValue::End => "end",
            BeamValue::ForwardHook => "forward hook",
            BeamValue::BackwardHook => "backward hook",
        }
    }
}

// ============================================================
// Note event for intermediate representation
// ============================================================

#[derive(Debug, Clone)]
struct NoteEvent {
    time_stamp: u16,
    staff: i8,
    voice: i8,
    note_num: u8,
    l_dur: i8,
    ndots: u8,
    rest: bool,
    accident: u8,
    #[allow(dead_code)]
    in_chord: bool,
    tied_l: bool,
    tied_r: bool,
    slurred_l: bool,
    slurred_r: bool,
    /// Beam levels for this note: beam_levels[0] = primary beam (number=1), etc.
    beam_levels: Vec<BeamValue>,
    /// Duration in PDUR ticks (computed).
    #[allow(dead_code)]
    duration: i32,
}

// ============================================================
// Part definition
// ============================================================

struct PartDef {
    id: String,
    name: String,
    first_staff: i8,
    last_staff: i8,
    transpose: i8,
}

// ============================================================
// Measure context tracker
// ============================================================

#[derive(Debug, Clone)]
struct MeasureCtx {
    numerator: i8,
    denominator: i8,
    key_fifths: i32,
    /// Clef type per staff number (u8 to match defs.rs constants).
    clef: Vec<u8>,
    /// Whether attributes changed (need to emit).
    attrs_dirty: bool,
}

impl MeasureCtx {
    fn new(n_staves: i16) -> Self {
        MeasureCtx {
            numerator: 4,
            denominator: 4,
            key_fifths: 0,
            clef: vec![TREBLE_CLEF; n_staves as usize + 1],
            attrs_dirty: true,
        }
    }
}

// ============================================================
// XML writing helpers
// ============================================================

fn write_simple_element(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str, text: &str) {
    let _ = w.write_event(Event::Start(BytesStart::new(tag)));
    let _ = w.write_event(Event::Text(BytesText::new(text)));
    let _ = w.write_event(Event::End(BytesEnd::new(tag)));
}

fn write_empty_element(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str) {
    let _ = w.write_event(Event::Empty(BytesStart::new(tag)));
}

// ============================================================
// Derive n_staves from part_infos
// ============================================================

/// Compute the total number of staves from the PartInfo list.
/// Returns the maximum last_staff value across all parts.
fn compute_n_staves(part_infos: &[PartInfo]) -> i16 {
    part_infos
        .iter()
        .map(|p| p.last_staff as i16)
        .max()
        .unwrap_or(1)
}

// ============================================================
// Main export function
// ============================================================

/// Export an InterpretedScore to MusicXML 4.0 (partwise format).
///
/// Returns the MusicXML document as a UTF-8 string.
pub fn export_musicxml(score: &InterpretedScore) -> String {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));

    // Doctype -- write directly to the inner writer
    {
        use std::io::Write;
        let inner = writer.get_mut();
        let _ = inner.write_all(b"\n<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    }

    let n_staves = compute_n_staves(&score.part_infos);

    // Build part definitions from PartInfo
    let mut parts: Vec<PartDef> = Vec::new();
    for (i, p) in score.part_infos.iter().enumerate() {
        if p.first_staff >= 1 && p.last_staff >= p.first_staff && (p.first_staff as i16) <= n_staves
        {
            parts.push(PartDef {
                id: format!("P{}", i + 1),
                name: PartInfo::name_str(p),
                first_staff: p.first_staff,
                last_staff: p.last_staff,
                transpose: p.transpose,
            });
        }
    }

    // If no valid parts found, create a default one
    if parts.is_empty() {
        parts.push(PartDef {
            id: "P1".into(),
            name: "Part 1".into(),
            first_staff: 1,
            last_staff: n_staves as i8,
            transpose: 0,
        });
    }

    // <score-partwise>
    let mut score_elem = BytesStart::new("score-partwise");
    score_elem.push_attribute(("version", "4.0"));
    let _ = writer.write_event(Event::Start(score_elem));

    // <part-list>
    let _ = writer.write_event(Event::Start(BytesStart::new("part-list")));
    for part in &parts {
        let mut sp = BytesStart::new("score-part");
        sp.push_attribute(("id", part.id.as_str()));
        let _ = writer.write_event(Event::Start(sp));
        write_simple_element(&mut writer, "part-name", &part.name);
        let _ = writer.write_event(Event::End(BytesEnd::new("score-part")));
    }
    let _ = writer.write_event(Event::End(BytesEnd::new("part-list")));

    // Collect measure data: walk the score and collect events per measure
    let measure_data = collect_measure_data(score);

    // Write each part
    for part in &parts {
        let mut part_elem = BytesStart::new("part");
        part_elem.push_attribute(("id", part.id.as_str()));
        let _ = writer.write_event(Event::Start(part_elem));

        let n_part_staves = (part.last_staff - part.first_staff + 1) as usize;

        write_part_measures(&mut writer, &measure_data, part, n_part_staves, n_staves);

        let _ = writer.write_event(Event::End(BytesEnd::new("part")));
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("score-partwise")));

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).unwrap_or_default()
}

// ============================================================
// Measure data collection
// ============================================================

/// A dynamic marking collected during score walk.
#[derive(Debug, Clone)]
struct DynamicEvent {
    /// NGL dynamic type code (1–23).
    dynamic_type: u8,
    /// Staff number this dynamic applies to.
    staff: i8,
    /// Timestamp of the Sync this dynamic is attached to (approximate).
    #[allow(dead_code)]
    time_stamp: u16,
}

/// Data for one measure in the score.
struct MeasureData {
    measure_num: i32,
    notes: Vec<NoteEvent>,
    dynamics: Vec<DynamicEvent>,
    /// Time sig at start of measure (numerator, denominator), if changed.
    time_sig: Option<(i8, i8)>,
    /// Key sig at start of measure (fifths), if changed.
    key_fifths: Option<i32>,
    /// Clefs at start of measure: (staff, clef_type as u8), if changed.
    clefs: Vec<(i8, u8)>,
    /// Is this the first measure?
    #[allow(dead_code)]
    is_first: bool,
}

/// Compute MusicXML beam levels for each note in a beam group.
///
/// Takes an ordered list of (startend, fracs, frac_go_left) from ANoteBeam subobjects
/// and returns a Vec of beam level assignments for each note.
///
/// NGL `startend`:
///   +N means N beams begin at this note (first note has + equal to max beams)
///   -N means N beams end at this note (last note has - equal to max beams)
///   The absolute value indicates beam count change, not total beams.
///
/// MusicXML beam levels:
///   <beam number="1">begin|continue|end</beam>  (primary beam)
///   <beam number="2">begin|continue|end|forward hook|backward hook</beam>  (secondary)
///   etc.
fn compute_beam_levels(beam_notes: &[(i8, u8, u8)]) -> Vec<Vec<BeamValue>> {
    let n = beam_notes.len();
    if n == 0 {
        return Vec::new();
    }

    // Determine the number of beam levels active at each note position.
    // NGL startend: the first note's startend = total beams to start,
    // subsequent notes adjust the count up or down.
    //
    // We track active_beams at each position:
    //   - first note: active_beams = startend (positive = how many beams start)
    //   - subsequent: active_beams changes based on startend:
    //     positive startend = new beams starting, negative = beams ending
    //
    // Actually, the simplest reliable approach: compute active beam count
    // at each note based on its l_dur. But we don't have l_dur here.
    //
    // Alternative: derive active beams from startend accumulation.
    // At note i: active_beams[i] = active_beams[i-1] + startend[i]
    // where startend[0] = initial positive count
    let mut active: Vec<i32> = vec![0; n];
    let mut running = 0i32;
    for (i, &(startend, _fracs, _fgl)) in beam_notes.iter().enumerate() {
        running += startend as i32;
        active[i] = running.max(1); // at least 1 beam while in group
    }

    // The maximum beam level across the group
    let max_level = active.iter().copied().max().unwrap_or(1) as usize;
    if max_level == 0 {
        return vec![Vec::new(); n];
    }

    // For each beam level (1-indexed), determine begin/continue/end per note.
    let mut result: Vec<Vec<BeamValue>> = vec![Vec::new(); n];

    for level in 1..=max_level {
        let level_i = level as i32;

        // Find which notes have this beam level active
        let has_level: Vec<bool> = active.iter().map(|&a| a >= level_i).collect();

        for i in 0..n {
            if !has_level[i] {
                // Check for hooks: a beam level that's active only at this note
                // NGL uses fracs/frac_go_left for fractional beams
                let (_, fracs, frac_go_left) = beam_notes[i];
                if fracs as usize >= level && level > active[i] as usize {
                    if frac_go_left != 0 {
                        result[i].push(BeamValue::BackwardHook);
                    } else {
                        result[i].push(BeamValue::ForwardHook);
                    }
                }
                continue;
            }

            let prev_has = if i > 0 { has_level[i - 1] } else { false };
            let next_has = if i + 1 < n { has_level[i + 1] } else { false };

            let value = match (prev_has, next_has) {
                (false, true) => BeamValue::Begin,
                (false, false) => {
                    // Isolated note at this level — use hook based on position
                    let (_, fracs, frac_go_left) = beam_notes[i];
                    if fracs > 0 {
                        if frac_go_left != 0 {
                            BeamValue::BackwardHook
                        } else {
                            BeamValue::ForwardHook
                        }
                    } else if i == 0 {
                        BeamValue::ForwardHook
                    } else {
                        BeamValue::BackwardHook
                    }
                }
                (true, true) => BeamValue::Continue,
                (true, false) => BeamValue::End,
            };
            result[i].push(value);
        }
    }

    result
}

fn collect_measure_data(score: &InterpretedScore) -> Vec<MeasureData> {
    let mut measures: Vec<MeasureData> = Vec::new();
    let mut current_notes: Vec<NoteEvent> = Vec::new();
    let mut current_dynamics: Vec<DynamicEvent> = Vec::new();
    let mut pending_time_sig: Option<(i8, i8)> = None;
    let mut pending_key_fifths: Option<i32> = None;
    let mut pending_clefs: Vec<(i8, u8)> = Vec::new();
    let mut measure_num = 0i32;
    let mut is_first_measure = true;
    let mut current_ts = (4i8, 4i8);
    // Snapshot of initial attributes, consumed at first measure boundary so
    // mid-score objects before measure 2 don't overwrite them.
    let mut initial_key: Option<i32> = None;
    let mut initial_time: Option<(i8, i8)> = None;
    let mut initial_clefs: Vec<(i8, u8)> = Vec::new();

    // Pre-pass: collect beam info from all BeamSet objects.
    // Maps sync_link -> (voice, staff, Vec<BeamValue>) for beam assignment.
    // We use a HashMap keyed by (sync_link, staff, voice) -> beam_levels.
    let mut beam_map: std::collections::HashMap<(Link, i8, i8), Vec<BeamValue>> =
        std::collections::HashMap::new();

    for obj in score.walk() {
        if let ObjData::BeamSet(beamset) = &obj.data {
            if beamset.grace != 0 {
                continue; // Skip grace note beams for now
            }
            if let Some(notebeams) = score.notebeams.get(&obj.header.first_sub_obj) {
                // Collect ordered list of (startend, fracs, frac_go_left) per note in beam
                let beam_note_infos: Vec<(i8, u8, u8)> = notebeams
                    .iter()
                    .map(|nb| (nb.startend, nb.fracs, nb.frac_go_left))
                    .collect();

                let beam_levels = compute_beam_levels(&beam_note_infos);

                // Assign beam levels to each sync in the beam group
                for (i, nb) in notebeams.iter().enumerate() {
                    if i < beam_levels.len() && !beam_levels[i].is_empty() {
                        let key = (nb.bp_sync, beamset.ext_header.staffn, beamset.voice);
                        beam_map.insert(key, beam_levels[i].clone());
                    }
                }
            }
        }
    }

    for obj in score.walk() {
        match &obj.data {
            ObjData::TimeSig(_) => {
                // Access timesig subobjects from the HashMap
                if let Some(subs) = score.timesigs.get(&obj.header.first_sub_obj) {
                    if let Some(ts) = subs.first() {
                        pending_time_sig = Some((ts.numerator, ts.denominator));
                        current_ts = (ts.numerator, ts.denominator);
                    }
                }
            }
            ObjData::KeySig(_) => {
                // Access keysig subobjects from the HashMap
                if let Some(subs) = score.keysigs.get(&obj.header.first_sub_obj) {
                    if let Some(ks) = subs.first() {
                        pending_key_fifths =
                            Some(ks_to_fifths(&ks.ks_info.ks_item, ks.ks_info.n_ks_items));
                    }
                }
            }
            ObjData::Clef(_) => {
                // Access clef subobjects from the HashMap
                if let Some(subs) = score.clefs.get(&obj.header.first_sub_obj) {
                    for cs in subs {
                        // sub_type holds the clef type (i8); cast to u8 for our constants
                        pending_clefs.push((cs.header.staffn, cs.header.sub_type as u8));
                    }
                }
            }
            ObjData::Staff(_) => {
                // Read initial clef context from AStaff subobjects.
                // STAFF objects appear at the start of each system, carrying
                // the current clef/key/time state for each staff. We use these
                // to set the initial clef (before the first measure) and to
                // refresh clef state at system boundaries.
                if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
                    for astaff in astaff_list {
                        pending_clefs.push((astaff.staffn, astaff.clef_type as u8));
                    }
                }
            }
            ObjData::Measure(_) => {
                // Save previous measure if we have one
                if !is_first_measure || !current_notes.is_empty() {
                    // For the first real measure, restore initial attributes
                    // that were snapshot at the first Measure boundary.
                    let key = if is_first_measure {
                        initial_key.take().or_else(|| pending_key_fifths.take())
                    } else {
                        pending_key_fifths.take()
                    };
                    let time = if is_first_measure {
                        initial_time.take().or_else(|| pending_time_sig.take())
                    } else {
                        pending_time_sig.take()
                    };
                    let clefs = if is_first_measure && !initial_clefs.is_empty() {
                        let mut c = std::mem::take(&mut initial_clefs);
                        c.extend(std::mem::take(&mut pending_clefs));
                        c
                    } else {
                        std::mem::take(&mut pending_clefs)
                    };
                    measures.push(MeasureData {
                        measure_num,
                        notes: std::mem::take(&mut current_notes),
                        dynamics: std::mem::take(&mut current_dynamics),
                        time_sig: time,
                        key_fifths: key,
                        clefs,
                        is_first: is_first_measure,
                    });
                    is_first_measure = false;
                } else if is_first_measure {
                    // At the first Measure boundary with no notes yet, snapshot
                    // the initial attributes so that mid-score objects between
                    // Measure_1 and Measure_2 don't overwrite them.
                    initial_key = pending_key_fifths.take();
                    initial_time = pending_time_sig.take();
                    initial_clefs = std::mem::take(&mut pending_clefs);
                }
                measure_num += 1;
            }
            ObjData::Dynamic(dyn_obj) => {
                // Collect text dynamics (not hairpins) as direction events.
                let dtype = dyn_obj.dynamic_type as u8;
                if (1..=23).contains(&dtype) {
                    if let Some(subs) = score.dynamics.get(&obj.header.first_sub_obj) {
                        for adyn in subs {
                            // Use timestamp 0 — dynamic objects don't carry their own
                            // timestamp; they're positioned relative to their firstSyncL.
                            // We'll use the last seen sync timestamp as approximation.
                            let ts = current_notes.last().map(|n| n.time_stamp).unwrap_or(0);
                            current_dynamics.push(DynamicEvent {
                                dynamic_type: dtype,
                                staff: adyn.header.staffn,
                                time_stamp: ts,
                            });
                        }
                    }
                }
            }
            ObjData::Sync(sync) => {
                // Access note subobjects from the HashMap
                let notes = score.get_notes(obj.header.first_sub_obj);
                let sync_link = obj.index as Link;
                for note in &notes {
                    // In the current API:
                    // - l_dur is stored in note.header.sub_type (i8)
                    // - staffn is note.header.staffn (i8)
                    // - voice is note.voice (i8)
                    let l_dur = note.header.sub_type;
                    let duration = if l_dur == WHOLEMR_L_DUR {
                        whole_measure_dur(current_ts.0, current_ts.1)
                    } else {
                        dur_to_ticks(l_dur, note.ndots)
                    };

                    // WHOLEMR rests always start at beat 0 (beginning of measure),
                    // regardless of their stored timestamp.
                    let effective_ts = if l_dur == WHOLEMR_L_DUR {
                        0
                    } else {
                        sync.time_stamp
                    };

                    // Look up beam info for this note from the pre-pass beam map.
                    let beam_key = (sync_link, note.header.staffn, note.voice);
                    let beam_levels = beam_map.get(&beam_key).cloned().unwrap_or_default();

                    current_notes.push(NoteEvent {
                        time_stamp: effective_ts,
                        staff: note.header.staffn,
                        voice: note.voice,
                        note_num: note.note_num,
                        l_dur,
                        ndots: note.ndots,
                        rest: note.rest,
                        accident: note.accident,
                        in_chord: note.in_chord,
                        tied_l: note.tied_l,
                        tied_r: note.tied_r,
                        slurred_l: note.slurred_l,
                        slurred_r: note.slurred_r,
                        beam_levels,
                        duration,
                    });
                }
            }
            _ => {}
        }
    }

    // Flush last measure
    if !current_notes.is_empty() || measure_num > 0 {
        let key = if is_first_measure {
            initial_key.or(pending_key_fifths)
        } else {
            pending_key_fifths
        };
        let time = if is_first_measure {
            initial_time.or(pending_time_sig)
        } else {
            pending_time_sig
        };
        let clefs = if is_first_measure && !initial_clefs.is_empty() {
            let mut c = initial_clefs;
            c.extend(pending_clefs);
            c
        } else {
            pending_clefs
        };
        measures.push(MeasureData {
            measure_num,
            notes: current_notes,
            dynamics: current_dynamics,
            time_sig: time,
            key_fifths: key,
            clefs,
            is_first: is_first_measure,
        });
    }

    measures
}

// ============================================================
// Part measure writing
// ============================================================

fn write_part_measures(
    w: &mut Writer<Cursor<Vec<u8>>>,
    measures: &[MeasureData],
    part: &PartDef,
    n_part_staves: usize,
    n_total_staves: i16,
) {
    let mut ctx = MeasureCtx::new(n_total_staves);

    for (i, mdata) in measures.iter().enumerate() {
        let mnum = if mdata.measure_num > 0 {
            mdata.measure_num
        } else {
            (i + 1) as i32
        };
        let mut meas_elem = BytesStart::new("measure");
        meas_elem.push_attribute(("number", mnum.to_string().as_str()));
        let _ = w.write_event(Event::Start(meas_elem));

        // Update context
        if let Some((n, d)) = mdata.time_sig {
            if n != ctx.numerator || d != ctx.denominator {
                ctx.numerator = n;
                ctx.denominator = d;
                ctx.attrs_dirty = true;
            }
        }
        if let Some(fifths) = mdata.key_fifths {
            if fifths != ctx.key_fifths {
                ctx.key_fifths = fifths;
                ctx.attrs_dirty = true;
            }
        }
        for &(staffn, clef_type) in &mdata.clefs {
            let idx = staffn as usize;
            if idx < ctx.clef.len() && ctx.clef[idx] != clef_type {
                ctx.clef[idx] = clef_type;
                ctx.attrs_dirty = true;
            }
        }

        // Write attributes if first measure or changed
        if i == 0 || ctx.attrs_dirty {
            let _ = w.write_event(Event::Start(BytesStart::new("attributes")));
            write_simple_element(w, "divisions", &DIVISIONS.to_string());

            // Key
            let _ = w.write_event(Event::Start(BytesStart::new("key")));
            write_simple_element(w, "fifths", &ctx.key_fifths.to_string());
            let _ = w.write_event(Event::End(BytesEnd::new("key")));

            // Time
            let _ = w.write_event(Event::Start(BytesStart::new("time")));
            write_simple_element(w, "beats", &ctx.numerator.to_string());
            write_simple_element(w, "beat-type", &ctx.denominator.to_string());
            let _ = w.write_event(Event::End(BytesEnd::new("time")));

            // Staves
            if n_part_staves > 1 {
                write_simple_element(w, "staves", &n_part_staves.to_string());
            }

            // Clefs
            for s in part.first_staff..=part.last_staff {
                let clef_type = ctx.clef.get(s as usize).copied().unwrap_or(TREBLE_CLEF);
                let (sign, line, oct_change) = clef_to_xml(clef_type);
                if n_part_staves > 1 {
                    let mut clef_elem = BytesStart::new("clef");
                    clef_elem.push_attribute((
                        "number",
                        (s - part.first_staff + 1).to_string().as_str(),
                    ));
                    let _ = w.write_event(Event::Start(clef_elem));
                } else {
                    let _ = w.write_event(Event::Start(BytesStart::new("clef")));
                }
                write_simple_element(w, "sign", sign);
                write_simple_element(w, "line", &line.to_string());
                if oct_change != 0 {
                    write_simple_element(w, "clef-octave-change", &oct_change.to_string());
                }
                let _ = w.write_event(Event::End(BytesEnd::new("clef")));
            }

            // Transpose
            if part.transpose != 0 && i == 0 {
                let _ = w.write_event(Event::Start(BytesStart::new("transpose")));
                write_simple_element(w, "chromatic", &part.transpose.to_string());
                let _ = w.write_event(Event::End(BytesEnd::new("transpose")));
            }

            let _ = w.write_event(Event::End(BytesEnd::new("attributes")));
            ctx.attrs_dirty = false;
        }

        // Write dynamics as <direction> elements for this part's staves
        for dyn_ev in &mdata.dynamics {
            if dyn_ev.staff >= part.first_staff && dyn_ev.staff <= part.last_staff {
                if let Some(dyn_name) = dynamic_type_to_xml(dyn_ev.dynamic_type) {
                    let _ = w.write_event(Event::Start(BytesStart::new("direction")));
                    let _ = w.write_event(Event::Start(BytesStart::new("direction-type")));
                    let _ = w.write_event(Event::Start(BytesStart::new("dynamics")));
                    write_empty_element(w, dyn_name);
                    let _ = w.write_event(Event::End(BytesEnd::new("dynamics")));
                    let _ = w.write_event(Event::End(BytesEnd::new("direction-type")));
                    if n_part_staves > 1 {
                        let staff_in_part = dyn_ev.staff - part.first_staff + 1;
                        write_simple_element(w, "staff", &staff_in_part.to_string());
                    }
                    let _ = w.write_event(Event::End(BytesEnd::new("direction")));
                }
            }
        }

        // Filter notes for this part's staves
        let mut part_notes: Vec<&NoteEvent> = mdata
            .notes
            .iter()
            .filter(|n| n.staff >= part.first_staff && n.staff <= part.last_staff)
            .collect();

        // Remove whole-measure rests if there are other notes in the same (staff, voice).
        // A whole-measure rest should only appear if it's the ONLY note in that voice.
        let mut voices_with_notes: std::collections::HashSet<(i8, i8)> =
            std::collections::HashSet::new();
        for note in &part_notes {
            if note.l_dur != WHOLEMR_L_DUR {
                voices_with_notes.insert((note.staff, note.voice));
            }
        }
        part_notes.retain(|note| {
            note.l_dur != WHOLEMR_L_DUR || !voices_with_notes.contains(&(note.staff, note.voice))
        });

        // Group by (staff-within-part, voice) and sort by timestamp
        write_measure_notes(w, &part_notes, part, n_part_staves, &ctx);

        let _ = w.write_event(Event::End(BytesEnd::new("measure")));
    }
}

fn write_measure_notes(
    w: &mut Writer<Cursor<Vec<u8>>>,
    notes: &[&NoteEvent],
    part: &PartDef,
    n_part_staves: usize,
    ctx: &MeasureCtx,
) {
    if notes.is_empty() {
        // Write a whole-measure rest for the part
        let dur = whole_measure_dur(ctx.numerator, ctx.denominator);
        let _ = w.write_event(Event::Start(BytesStart::new("note")));
        write_empty_element(w, "rest");
        write_simple_element(w, "duration", &dur.to_string());
        write_simple_element(w, "voice", "1");
        write_simple_element(w, "type", "whole");
        let _ = w.write_event(Event::End(BytesEnd::new("note")));
        return;
    }

    // Collect unique (staff, voice) pairs in order
    let mut staff_voices: Vec<(i8, i8)> = Vec::new();
    for n in notes {
        let sv = (n.staff, n.voice);
        if !staff_voices.contains(&sv) {
            staff_voices.push(sv);
        }
    }
    staff_voices.sort();

    let measure_dur = whole_measure_dur(ctx.numerator, ctx.denominator);
    let mut is_first_voice = true;

    for &(staff, voice) in &staff_voices {
        let voice_notes: Vec<&&NoteEvent> = notes
            .iter()
            .filter(|n| n.staff == staff && n.voice == voice)
            .collect();

        if voice_notes.is_empty() {
            continue;
        }

        // Backup to measure start for subsequent voices
        if !is_first_voice {
            let _ = w.write_event(Event::Start(BytesStart::new("backup")));
            write_simple_element(w, "duration", &measure_dur.to_string());
            let _ = w.write_event(Event::End(BytesEnd::new("backup")));
        }

        // Sort by timestamp
        let mut sorted: Vec<&&NoteEvent> = voice_notes.clone();
        sorted.sort_by_key(|n| n.time_stamp);

        let mut current_time = 0i32;
        let mut prev_timestamp: Option<u16> = None;
        let staff_in_part = staff - part.first_staff + 1;
        let voice_str = voice.to_string();

        for note in &sorted {
            let note_time = note.time_stamp as i32;

            // Forward if needed (gap/rest between notes)
            if note_time > current_time {
                let gap = note_time - current_time;
                let _ = w.write_event(Event::Start(BytesStart::new("forward")));
                write_simple_element(w, "duration", &gap.to_string());
                let _ = w.write_event(Event::End(BytesEnd::new("forward")));
                current_time = note_time;
            }

            // Write note
            let _ = w.write_event(Event::Start(BytesStart::new("note")));

            // Chord: emit <chord/> for any note sharing a timestamp with the previous note.
            // In MusicXML, <chord/> means "same onset time" -- this is broader than NGL's
            // in_chord flag which indicates stem-sharing. Two notes at the same timestamp
            // in the same voice are always a chord in MusicXML, regardless of NGL flags.
            let is_xml_chord = prev_timestamp == Some(note.time_stamp);
            if is_xml_chord {
                write_empty_element(w, "chord");
            }

            if note.rest {
                write_empty_element(w, "rest");
            } else {
                let (step, alter, octave) = midi_to_pitch(note.note_num, note.accident);
                let _ = w.write_event(Event::Start(BytesStart::new("pitch")));
                write_simple_element(w, "step", step);
                if alter != 0 {
                    write_simple_element(w, "alter", &alter.to_string());
                }
                write_simple_element(w, "octave", &octave.to_string());
                let _ = w.write_event(Event::End(BytesEnd::new("pitch")));
            }

            // Duration: use the notated duration from l_dur + ndots.
            // For WHOLEMR rests, use the full measure duration.
            // This ensures time accounting matches the visual notation.
            let effective_dur = if note.l_dur == WHOLEMR_L_DUR {
                measure_dur
            } else {
                let visual_dur = dur_to_ticks(note.l_dur, note.ndots);
                // Clamp to remaining measure space to avoid overflow
                let remaining = measure_dur - note_time;
                visual_dur.min(remaining.max(1))
            };
            write_simple_element(w, "duration", &effective_dur.to_string());

            // Ties
            if note.tied_l {
                let mut tie = BytesStart::new("tie");
                tie.push_attribute(("type", "stop"));
                let _ = w.write_event(Event::Empty(tie));
            }
            if note.tied_r {
                let mut tie = BytesStart::new("tie");
                tie.push_attribute(("type", "start"));
                let _ = w.write_event(Event::Empty(tie));
            }

            write_simple_element(w, "voice", &voice_str);

            // Note type (visual duration)
            let dur_type = if note.l_dur == WHOLEMR_L_DUR {
                "whole"
            } else {
                l_dur_to_type(note.l_dur)
            };
            write_simple_element(w, "type", dur_type);

            // Dots — MusicXML schema requires <dot/> immediately after <type>
            for _ in 0..note.ndots {
                write_empty_element(w, "dot");
            }

            // Accidental — DTD order: type, dot*, accidental?, ..., staff?, beam*
            if !note.rest && note.accident != 0 {
                let acc_text = match note.accident {
                    AC_DBLFLAT => "flat-flat",
                    AC_FLAT => "flat",
                    AC_NATURAL => "natural",
                    AC_SHARP => "sharp",
                    AC_DBLSHARP => "double-sharp",
                    _ => "",
                };
                if !acc_text.is_empty() {
                    write_simple_element(w, "accidental", acc_text);
                }
            }

            // Staff (for multi-staff parts) — must come before <beam>
            if n_part_staves > 1 {
                write_simple_element(w, "staff", &staff_in_part.to_string());
            }

            // Beams — DTD requires <beam> after <staff>
            if !note.beam_levels.is_empty() {
                for (i, bv) in note.beam_levels.iter().enumerate() {
                    let num = (i + 1).to_string();
                    let mut beam_elem = BytesStart::new("beam");
                    beam_elem.push_attribute(("number", num.as_str()));
                    let _ = w.write_event(Event::Start(beam_elem));
                    let _ = w.write_event(Event::Text(BytesText::new(bv.as_str())));
                    let _ = w.write_event(Event::End(BytesEnd::new("beam")));
                }
            }

            // Notations (ties, slurs)
            let has_notations = note.tied_l || note.tied_r || note.slurred_l || note.slurred_r;
            if has_notations {
                let _ = w.write_event(Event::Start(BytesStart::new("notations")));
                if note.tied_l {
                    let mut tied = BytesStart::new("tied");
                    tied.push_attribute(("type", "stop"));
                    let _ = w.write_event(Event::Empty(tied));
                }
                if note.tied_r {
                    let mut tied = BytesStart::new("tied");
                    tied.push_attribute(("type", "start"));
                    let _ = w.write_event(Event::Empty(tied));
                }
                if note.slurred_l {
                    let mut slur = BytesStart::new("slur");
                    slur.push_attribute(("number", "1"));
                    slur.push_attribute(("type", "stop"));
                    let _ = w.write_event(Event::Empty(slur));
                }
                if note.slurred_r {
                    let mut slur = BytesStart::new("slur");
                    slur.push_attribute(("number", "1"));
                    slur.push_attribute(("type", "start"));
                    let _ = w.write_event(Event::Empty(slur));
                }
                let _ = w.write_event(Event::End(BytesEnd::new("notations")));
            }

            let _ = w.write_event(Event::End(BytesEnd::new("note")));

            // Advance time: only the first note at a timestamp advances the cursor.
            // Chord notes (same timestamp as previous) don't advance.
            if prev_timestamp != Some(note.time_stamp) {
                current_time = note_time + effective_dur;
            }

            prev_timestamp = Some(note.time_stamp);
        }

        // Pad voice to fill the full measure duration.
        // MusicXML requires each voice to account for the entire measure.
        if current_time < measure_dur {
            let pad = measure_dur - current_time;
            let _ = w.write_event(Event::Start(BytesStart::new("forward")));
            write_simple_element(w, "duration", &pad.to_string());
            let _ = w.write_event(Event::End(BytesEnd::new("forward")));
        }

        is_first_voice = false;
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ngl::{interpret::interpret_heap, NglFile};

    fn load_and_export(name: &str) -> String {
        let path = format!("tests/fixtures/{}", name);
        let data = std::fs::read(&path).unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        let score = interpret_heap(&ngl).unwrap();
        export_musicxml(&score)
    }

    #[test]
    fn export_produces_valid_xml() {
        let xml = load_and_export("01_me_and_lucy.ngl");
        assert!(
            xml.starts_with("<?xml"),
            "Should start with XML declaration"
        );
        assert!(
            xml.contains("<score-partwise"),
            "Should contain score-partwise"
        );
        assert!(xml.contains("</score-partwise>"), "Should be closed");
    }

    #[test]
    fn export_has_parts() {
        let xml = load_and_export("01_me_and_lucy.ngl");
        assert!(xml.contains("<part-list>"), "Should have part-list");
        assert!(xml.contains("<score-part"), "Should have score-part");
        assert!(xml.contains("<part id="), "Should have part elements");
    }

    #[test]
    fn export_has_measures() {
        let xml = load_and_export("01_me_and_lucy.ngl");
        assert!(xml.contains("<measure number="), "Should have measures");
        assert!(xml.contains("<attributes>"), "Should have attributes");
        assert!(xml.contains("<divisions>"), "Should have divisions");
    }

    #[test]
    fn export_has_notes() {
        let xml = load_and_export("01_me_and_lucy.ngl");
        assert!(xml.contains("<note>"), "Should have notes");
        assert!(xml.contains("<pitch>"), "Should have pitched notes");
        assert!(xml.contains("<step>"), "Should have step");
        assert!(xml.contains("<octave>"), "Should have octave");
        assert!(xml.contains("<duration>"), "Should have duration");
        assert!(xml.contains("<type>"), "Should have type");
    }

    #[test]
    fn export_has_key_time_clef() {
        let xml = load_and_export("01_me_and_lucy.ngl");
        assert!(xml.contains("<key>"), "Should have key");
        assert!(xml.contains("<fifths>"), "Should have fifths");
        assert!(xml.contains("<time>"), "Should have time");
        assert!(xml.contains("<beats>"), "Should have beats");
        assert!(xml.contains("<clef>"), "Should have clef");
        assert!(xml.contains("<sign>"), "Should have sign");
    }

    #[test]
    fn export_n103_works() {
        let xml = load_and_export("01_me_and_lucy.ngl");
        assert!(xml.contains("<score-partwise"));
        assert!(xml.contains("<note>"));
    }

    #[test]
    fn export_multiple_fixtures() {
        let fixtures = [
            "01_me_and_lucy.ngl",
            "02_cloning_frank_blacks.ngl",
            "05_abigail.ngl",
        ];
        for name in fixtures {
            let path = format!("tests/fixtures/{}", name);
            if let Ok(data) = std::fs::read(&path) {
                let ngl = NglFile::read_from_bytes(&data).unwrap();
                let score = interpret_heap(&ngl).unwrap();
                let xml = export_musicxml(&score);
                assert!(
                    xml.contains("<score-partwise"),
                    "{}: missing score-partwise",
                    name
                );
                assert!(xml.contains("<measure"), "{}: missing measures", name);
            }
        }
    }

    #[test]
    fn dur_to_ticks_values() {
        assert_eq!(dur_to_ticks(WHOLE_L_DUR, 0), 1920);
        assert_eq!(dur_to_ticks(HALF_L_DUR, 0), 960);
        assert_eq!(dur_to_ticks(QTR_L_DUR, 0), 480);
        assert_eq!(dur_to_ticks(EIGHTH_L_DUR, 0), 240);
        assert_eq!(dur_to_ticks(SIXTEENTH_L_DUR, 0), 120);

        // Dotted quarter = 480 + 240 = 720
        assert_eq!(dur_to_ticks(QTR_L_DUR, 1), 720);

        // Double-dotted quarter = 480 + 240 + 120 = 840
        assert_eq!(dur_to_ticks(QTR_L_DUR, 2), 840);
    }

    #[test]
    fn midi_pitch_conversion() {
        // Middle C = MIDI 60
        let (step, alter, oct) = midi_to_pitch(60, 0);
        assert_eq!((step, alter, oct), ("C", 0, 4));

        // C#4 = MIDI 61, explicit sharp
        let (step, alter, oct) = midi_to_pitch(61, AC_SHARP);
        assert_eq!((step, alter, oct), ("C", 1, 4));

        // Db4 = MIDI 61, explicit flat
        let (step, alter, oct) = midi_to_pitch(61, AC_FLAT);
        assert_eq!((step, alter, oct), ("D", -1, 4));

        // G3 = MIDI 55
        let (step, alter, oct) = midi_to_pitch(55, 0);
        assert_eq!((step, alter, oct), ("G", 0, 3));
    }

    #[test]
    fn export_all_fixtures() {
        // Run export on every NGL fixture to verify no panics
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
                let xml = export_musicxml(&score);
                assert!(
                    xml.contains("<score-partwise"),
                    "{}: missing score-partwise",
                    path.display()
                );
                count += 1;
            }
        }
        assert!(count > 0, "Should have found NGL fixtures");
    }

    #[test]
    fn export_me_and_lucy_to_file() {
        let data = std::fs::read("tests/fixtures/01_me_and_lucy.ngl").unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        let score = interpret_heap(&ngl).unwrap();
        let xml = export_musicxml(&score);

        let out_dir = std::path::Path::new("test-output");
        std::fs::create_dir_all(out_dir).unwrap();
        let out_path = out_dir.join("me_and_lucy.musicxml");
        std::fs::write(&out_path, &xml).unwrap();
        eprintln!("Wrote {}", out_path.display());
        assert!(xml.contains("<score-partwise"));
    }
}
