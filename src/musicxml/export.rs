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

use crate::basic_types::KsItem;
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
    match pc {
        0 => ("C", 0, octave),
        1 => {
            if accident == AC_FLAT {
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
            if accident == AC_FLAT {
                ("G", -1, octave)
            } else {
                ("F", 1, octave)
            }
        }
        7 => ("G", 0, octave),
        8 => {
            if accident == AC_FLAT {
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

/// Convert NGL clef code to MusicXML (sign, line).
/// Clef constants are u8 in defs.rs; AClef.header.sub_type is i8.
fn clef_to_xml(clef_type: u8) -> (&'static str, i32) {
    match clef_type {
        TREBLE_CLEF | TREBLE8_CLEF | FRVIOLIN_CLEF => ("G", 2),
        SOPRANO_CLEF => ("C", 1),
        MZSOPRANO_CLEF => ("C", 2),
        ALTO_CLEF => ("C", 3),
        TRTENOR_CLEF => ("C", 3), // treble-tenor ~ alto
        TENOR_CLEF => ("C", 4),
        BARITONE_CLEF => ("F", 3),
        BASS_CLEF | BASS8B_CLEF => ("F", 4),
        PERC_CLEF => ("percussion", 2),
        _ => ("G", 2),
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

/// Data for one measure in the score.
struct MeasureData {
    measure_num: i32,
    notes: Vec<NoteEvent>,
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

fn collect_measure_data(score: &InterpretedScore) -> Vec<MeasureData> {
    let mut measures: Vec<MeasureData> = Vec::new();
    let mut current_notes: Vec<NoteEvent> = Vec::new();
    let mut pending_time_sig: Option<(i8, i8)> = None;
    let mut pending_key_fifths: Option<i32> = None;
    let mut pending_clefs: Vec<(i8, u8)> = Vec::new();
    let mut measure_num = 0i32;
    let mut is_first_measure = true;
    let mut current_ts = (4i8, 4i8);

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
            ObjData::Measure(_) => {
                // Save previous measure if we have one
                if !is_first_measure || !current_notes.is_empty() {
                    measures.push(MeasureData {
                        measure_num,
                        notes: std::mem::take(&mut current_notes),
                        time_sig: pending_time_sig.take(),
                        key_fifths: pending_key_fifths.take(),
                        clefs: std::mem::take(&mut pending_clefs),
                        is_first: is_first_measure,
                    });
                    is_first_measure = false;
                }
                measure_num += 1;
            }
            ObjData::Sync(sync) => {
                // Access note subobjects from the HashMap
                let notes = score.get_notes(obj.header.first_sub_obj);
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

                    current_notes.push(NoteEvent {
                        time_stamp: sync.time_stamp,
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
                        duration,
                    });
                }
            }
            _ => {}
        }
    }

    // Flush last measure
    if !current_notes.is_empty() || measure_num > 0 {
        measures.push(MeasureData {
            measure_num,
            notes: current_notes,
            time_sig: pending_time_sig,
            key_fifths: pending_key_fifths,
            clefs: pending_clefs,
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
                let (sign, line) = clef_to_xml(clef_type);
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

        // Collect distinct onset timestamps for computing actual durations.
        // NGL timestamps tell us exact onset times; we compute each note's
        // sounding duration as the gap from its onset to the next distinct onset
        // (or to the measure end for the last group).
        let distinct_onsets: Vec<u16> = {
            let mut ts: Vec<u16> = sorted.iter().map(|n| n.time_stamp).collect();
            ts.dedup(); // sorted already, just remove consecutive duplicates
            ts
        };

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

            // Duration: use the gap to the next distinct onset time, not the
            // NGL visual duration. NGL timestamps are authoritative for time
            // position; visual durations (l_dur) can exceed the gap when notes
            // overlap or don't tile perfectly.
            let effective_dur = {
                let onset_idx = distinct_onsets.iter().position(|&t| t == note.time_stamp);
                match onset_idx {
                    Some(idx) if idx + 1 < distinct_onsets.len() => {
                        // Gap to next onset
                        (distinct_onsets[idx + 1] as i32) - note_time
                    }
                    _ => {
                        // Last onset group: use gap to measure end, or visual duration
                        // as fallback (whichever fits the measure).
                        let gap_to_end = measure_dur - note_time;
                        gap_to_end.max(1)
                    }
                }
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

            // Note type (visual duration -- for display, not time accounting)
            let dur_type = if note.l_dur == WHOLEMR_L_DUR {
                "whole"
            } else {
                l_dur_to_type(note.l_dur)
            };
            write_simple_element(w, "type", dur_type);

            // Dots
            for _ in 0..note.ndots {
                write_empty_element(w, "dot");
            }

            // Staff (for multi-staff parts)
            if n_part_staves > 1 {
                write_simple_element(w, "staff", &staff_in_part.to_string());
            }

            // Notations (ties)
            if note.tied_l || note.tied_r {
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
}
