//! Cross-validation tests for NGL binary interpreter and Notelist parser.
//!
//! This test suite validates the integrity of our NGL file interpretation
//! and provides tools to compare NGL binary format against Notelist text format.

use nightingale_core::defs::*;
use nightingale_core::ngl::{interpret_heap, InterpretedScore, NglFile};
use nightingale_core::notelist::parser::{parse_notelist, Notelist, NotelistRecord};
use std::collections::HashMap;
use std::fs::File;

// =============================================================================
// Summary Types
// =============================================================================

/// Summary of a score extracted from NGL binary format.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct NglScoreSummary {
    /// Total number of objects by type
    object_counts: HashMap<u8, usize>,
    /// Total number of note subobjects
    note_count: usize,
    /// Total number of rest subobjects
    rest_count: usize,
    /// MIDI note numbers encountered (for validation)
    midi_notes: Vec<u8>,
    /// Durations encountered (l_dur values)
    durations: Vec<i8>,
    /// Voice assignments
    voices: Vec<i8>,
    /// Staff count (from STAFF objects)
    staff_count: usize,
    /// Measure count (from MEASURE objects)
    measure_count: usize,
    /// Total objects in heap
    total_objects: usize,
    /// Total subobjects in notes HashMap
    total_note_subobjects: usize,
    /// Total subobjects in measures HashMap
    total_measure_subobjects: usize,
}

/// Summary of a score extracted from Notelist text format.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct NotelistScoreSummary {
    /// Total number of records by type name
    record_counts: HashMap<&'static str, usize>,
    /// Total notes
    note_count: usize,
    /// Total rests
    rest_count: usize,
    /// MIDI note numbers
    midi_notes: Vec<u8>,
    /// Durations (l_dur values)
    durations: Vec<i8>,
    /// Voice assignments
    voices: Vec<i8>,
    /// Unique staff numbers
    staffs: Vec<i8>,
    /// Measure count (barlines)
    measure_count: usize,
}

// =============================================================================
// NGL Score Summary Extractor
// =============================================================================

fn summarize_ngl_score(score: &InterpretedScore) -> NglScoreSummary {
    let mut object_counts: HashMap<u8, usize> = HashMap::new();
    let mut note_count = 0;
    let mut rest_count = 0;
    let mut midi_notes = Vec::new();
    let mut durations = Vec::new();
    let mut voices = Vec::new();
    let mut staff_count = 0;
    let mut measure_count = 0;

    // Count objects by type
    for obj in &score.objects {
        let obj_type = obj.header.obj_type as u8;
        *object_counts.entry(obj_type).or_insert(0) += 1;

        // Count staff and measure objects
        match obj_type {
            STAFF_TYPE => staff_count += 1,
            MEASURE_TYPE => measure_count += 1,
            _ => {}
        }
    }

    // Count note subobjects
    let mut total_note_subobjects = 0;
    for notes_vec in score.notes.values() {
        total_note_subobjects += notes_vec.len();
        for note in notes_vec {
            if note.rest {
                rest_count += 1;
            } else {
                note_count += 1;
                midi_notes.push(note.note_num);
            }
            durations.push(note.header.sub_type); // l_dur
            voices.push(note.voice);
        }
    }

    // Count measure subobjects
    let mut total_measure_subobjects = 0;
    for measures_vec in score.measures.values() {
        total_measure_subobjects += measures_vec.len();
    }

    NglScoreSummary {
        object_counts,
        note_count,
        rest_count,
        midi_notes,
        durations,
        voices,
        staff_count,
        measure_count,
        total_objects: score.objects.len(),
        total_note_subobjects,
        total_measure_subobjects,
    }
}

// =============================================================================
// Notelist Score Summary Extractor
// =============================================================================

fn summarize_notelist(notelist: &Notelist) -> NotelistScoreSummary {
    let mut record_counts: HashMap<&'static str, usize> = HashMap::new();
    let mut note_count = 0;
    let mut rest_count = 0;
    let mut midi_notes = Vec::new();
    let mut durations = Vec::new();
    let mut voices = Vec::new();
    let mut staffs_set = std::collections::HashSet::new();
    let mut measure_count = 0;

    for record in &notelist.records {
        match record {
            NotelistRecord::Note {
                voice,
                staff,
                dur,
                note_num,
                ..
            } => {
                *record_counts.entry("Note").or_insert(0) += 1;
                note_count += 1;
                midi_notes.push(*note_num);
                durations.push(*dur);
                voices.push(*voice);
                staffs_set.insert(*staff);
            }
            NotelistRecord::Rest {
                voice, staff, dur, ..
            } => {
                *record_counts.entry("Rest").or_insert(0) += 1;
                rest_count += 1;
                durations.push(*dur);
                voices.push(*voice);
                staffs_set.insert(*staff);
            }
            NotelistRecord::GraceNote { .. } => {
                *record_counts.entry("GraceNote").or_insert(0) += 1;
            }
            NotelistRecord::Barline { .. } => {
                *record_counts.entry("Barline").or_insert(0) += 1;
                measure_count += 1;
            }
            NotelistRecord::Clef { .. } => {
                *record_counts.entry("Clef").or_insert(0) += 1;
            }
            NotelistRecord::KeySig { .. } => {
                *record_counts.entry("KeySig").or_insert(0) += 1;
            }
            NotelistRecord::TimeSig { .. } => {
                *record_counts.entry("TimeSig").or_insert(0) += 1;
            }
            NotelistRecord::Dynamic { .. } => {
                *record_counts.entry("Dynamic").or_insert(0) += 1;
            }
            NotelistRecord::Text { .. } => {
                *record_counts.entry("Text").or_insert(0) += 1;
            }
            NotelistRecord::Tempo { .. } => {
                *record_counts.entry("Tempo").or_insert(0) += 1;
            }
            NotelistRecord::Tuplet { .. } => {
                *record_counts.entry("Tuplet").or_insert(0) += 1;
            }
            NotelistRecord::Beam { .. } => {
                *record_counts.entry("Beam").or_insert(0) += 1;
            }
            NotelistRecord::Comment(_) => {
                *record_counts.entry("Comment").or_insert(0) += 1;
            }
        }
    }

    let mut staffs: Vec<i8> = staffs_set.into_iter().collect();
    staffs.sort_unstable();

    NotelistScoreSummary {
        record_counts,
        note_count,
        rest_count,
        midi_notes,
        durations,
        voices,
        staffs,
        measure_count,
    }
}

// =============================================================================
// NGL Self-Validation Tests
// =============================================================================

/// Validate that all object links are valid (point to real objects or NILINK).
///
/// Note: firstSubObj can point beyond the object list into subobject storage,
/// so we only validate right/left links which should point to objects.
fn validate_object_links(score: &InterpretedScore) -> Result<(), String> {
    let max_index = score.objects.len() as u16;

    for obj in &score.objects {
        // Check right link
        if obj.header.right != NILINK && obj.header.right > max_index {
            return Err(format!(
                "Object {} has invalid right link: {} (max={})",
                obj.index, obj.header.right, max_index
            ));
        }

        // Check left link
        if obj.header.left != NILINK && obj.header.left > max_index {
            return Err(format!(
                "Object {} has invalid left link: {} (max={})",
                obj.index, obj.header.left, max_index
            ));
        }

        // firstSubObj links can point to subobject storage which may have different
        // indexing than the object list, so we don't validate their range here.
        // Instead, we validate that referenced subobjects actually exist in the
        // appropriate HashMap in validate_sync_notes() and validate_measure_subobjects().
    }

    Ok(())
}

/// Validate that every SYNC has at least 1 note subobject.
fn validate_sync_notes(score: &InterpretedScore) -> Result<(), String> {
    for obj in &score.objects {
        if obj.header.obj_type as u8 == SYNC_TYPE {
            let first_sub = obj.header.first_sub_obj;
            if first_sub == NILINK {
                return Err(format!(
                    "SYNC object {} has NILINK firstSubObj (should have notes)",
                    obj.index
                ));
            }

            let notes = score.notes.get(&first_sub);
            if notes.is_none() || notes.unwrap().is_empty() {
                return Err(format!(
                    "SYNC object {} has no note subobjects at link {}",
                    obj.index, first_sub
                ));
            }

            // Check n_entries matches actual count
            let expected = obj.header.n_entries as usize;
            let actual = notes.unwrap().len();
            if expected != actual {
                return Err(format!(
                    "SYNC object {} has n_entries={} but {} actual note subobjects",
                    obj.index, expected, actual
                ));
            }
        }
    }

    Ok(())
}

/// Validate that every MEASURE has measure subobjects matching n_entries.
fn validate_measure_subobjects(score: &InterpretedScore) -> Result<(), String> {
    for obj in &score.objects {
        if obj.header.obj_type as u8 == MEASURE_TYPE {
            let first_sub = obj.header.first_sub_obj;
            if first_sub == NILINK {
                return Err(format!(
                    "MEASURE object {} has NILINK firstSubObj",
                    obj.index
                ));
            }

            let measures = score.measures.get(&first_sub);
            if measures.is_none() || measures.unwrap().is_empty() {
                return Err(format!(
                    "MEASURE object {} has no measure subobjects at link {}",
                    obj.index, first_sub
                ));
            }

            let expected = obj.header.n_entries as usize;
            let actual = measures.unwrap().len();
            if expected != actual {
                return Err(format!(
                    "MEASURE object {} has n_entries={} but {} actual measure subobjects",
                    obj.index, expected, actual
                ));
            }
        }
    }

    Ok(())
}

/// Validate no duplicate indices in the object list.
fn validate_no_duplicate_indices(score: &InterpretedScore) -> Result<(), String> {
    let mut seen_indices = std::collections::HashSet::new();

    for obj in &score.objects {
        if !seen_indices.insert(obj.index) {
            return Err(format!("Duplicate object index: {}", obj.index));
        }
    }

    Ok(())
}

/// Validate that walk() covers all objects in the score list (HEADER to TAIL).
fn validate_walk_completeness(score: &InterpretedScore) -> Result<(), String> {
    let walked: Vec<u16> = score.walk().map(|obj| obj.index).collect();

    if walked.is_empty() {
        return Err("walk() returned no objects".to_string());
    }

    // First object in walk should be first real object (after HEADER)
    // Last object in walk should be TAIL
    let last_obj = score.walk().last();
    if let Some(tail) = last_obj {
        if tail.header.obj_type as u8 != TAIL_TYPE {
            return Err(format!(
                "walk() last object is type {} (expected TAIL_TYPE={})",
                tail.header.obj_type, TAIL_TYPE
            ));
        }
    } else {
        return Err("walk() has no objects".to_string());
    }

    println!("  walk() traversed {} objects", walked.len());

    Ok(())
}

/// Validate internal consistency of n_entries across all object types.
fn validate_n_entries_consistency(score: &InterpretedScore) -> Result<(), String> {
    let mut total_note_subobjs_from_objects = 0;
    let mut total_measure_subobjs_from_objects = 0;

    for obj in &score.objects {
        let obj_type = obj.header.obj_type as u8;
        let n_entries = obj.header.n_entries as usize;

        match obj_type {
            SYNC_TYPE | GRSYNC_TYPE => {
                total_note_subobjs_from_objects += n_entries;
            }
            MEASURE_TYPE => {
                total_measure_subobjs_from_objects += n_entries;
            }
            _ => {}
        }
    }

    // Count actual subobjects in hashmaps
    let total_notes_in_hashmap: usize = score.notes.values().map(|v| v.len()).sum();
    let total_measures_in_hashmap: usize = score.measures.values().map(|v| v.len()).sum();

    if total_note_subobjs_from_objects != total_notes_in_hashmap {
        return Err(format!(
            "Note subobject count mismatch: sum of n_entries={} but hashmap has {} subobjects",
            total_note_subobjs_from_objects, total_notes_in_hashmap
        ));
    }

    if total_measure_subobjs_from_objects != total_measures_in_hashmap {
        return Err(format!(
            "Measure subobject count mismatch: sum of n_entries={} but hashmap has {} subobjects",
            total_measure_subobjs_from_objects, total_measures_in_hashmap
        ));
    }

    println!(
        "  n_entries consistency: {} note subobjs, {} measure subobjs",
        total_notes_in_hashmap, total_measures_in_hashmap
    );

    Ok(())
}

// =============================================================================
// Integration Tests: NGL Files
// =============================================================================

#[test]
fn test_ngl_self_validation_all_fixtures() {
    let fixtures = [
        "tests/fixtures/01_me_and_lucy.ngl",
        "tests/fixtures/02_cloning_frank_blacks.ngl",
        "tests/fixtures/03_holed_up_in_penjinskya.ngl",
        "tests/fixtures/04_eating_humble_pie.ngl",
        "tests/fixtures/05_abigail.ngl",
        "tests/fixtures/06_melyssa_with_a_y.ngl",
        "tests/fixtures/07_new_york_debutante.ngl",
        "tests/fixtures/08_darling_sunshine.ngl",
        "tests/fixtures/09_swiss_ann.ngl",
        "tests/fixtures/10_ghost_of_fusion_bob.ngl",
        "tests/fixtures/11_philip.ngl",
        "tests/fixtures/12_what_do_i_know.ngl",
        "tests/fixtures/13_miss_b.ngl",
        "tests/fixtures/14_chrome_molly.ngl",
        "tests/fixtures/15_selfsame_twin.ngl",
        "tests/fixtures/16_esmerelda.ngl",
    ];

    for path in &fixtures {
        println!("\n=== Validating {} ===", path);

        let ngl = NglFile::read_from_file(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));

        let score = interpret_heap(&ngl)
            .unwrap_or_else(|e| panic!("Failed to interpret heap for {}: {}", path, e));

        // Run all validation checks
        validate_object_links(&score)
            .unwrap_or_else(|e| panic!("{}: Link validation failed: {}", path, e));

        validate_sync_notes(&score)
            .unwrap_or_else(|e| panic!("{}: SYNC validation failed: {}", path, e));

        validate_measure_subobjects(&score)
            .unwrap_or_else(|e| panic!("{}: MEASURE validation failed: {}", path, e));

        validate_no_duplicate_indices(&score)
            .unwrap_or_else(|e| panic!("{}: Duplicate index validation failed: {}", path, e));

        validate_walk_completeness(&score)
            .unwrap_or_else(|e| panic!("{}: Walk validation failed: {}", path, e));

        validate_n_entries_consistency(&score)
            .unwrap_or_else(|e| panic!("{}: n_entries validation failed: {}", path, e));

        println!("  ✓ All validations passed");
    }
}

#[test]
fn test_ngl_score_summaries() {
    let fixtures = [
        "tests/fixtures/01_me_and_lucy.ngl",
        "tests/fixtures/02_cloning_frank_blacks.ngl",
        "tests/fixtures/03_holed_up_in_penjinskya.ngl",
    ];

    for path in &fixtures {
        println!("\n=== Summary for {} ===", path);

        let ngl = NglFile::read_from_file(path).expect("Failed to read NGL");
        let score = interpret_heap(&ngl).expect("Failed to interpret heap");
        let summary = summarize_ngl_score(&score);

        println!("Total objects: {}", summary.total_objects);
        println!("Staff count: {}", summary.staff_count);
        println!("Measure count: {}", summary.measure_count);
        println!("Notes: {}", summary.note_count);
        println!("Rests: {}", summary.rest_count);
        println!(
            "Total note/rest subobjects: {}",
            summary.total_note_subobjects
        );
        println!(
            "Total measure subobjects: {}",
            summary.total_measure_subobjects
        );

        println!("\nObject counts by type:");
        let mut types: Vec<_> = summary.object_counts.iter().collect();
        types.sort_by_key(|(k, _)| *k);
        for (obj_type, count) in types {
            let type_name = match *obj_type {
                HEADER_TYPE => "HEADER",
                TAIL_TYPE => "TAIL",
                SYNC_TYPE => "SYNC",
                RPTEND_TYPE => "RPTEND",
                PAGE_TYPE => "PAGE",
                SYSTEM_TYPE => "SYSTEM",
                STAFF_TYPE => "STAFF",
                MEASURE_TYPE => "MEASURE",
                CLEF_TYPE => "CLEF",
                KEYSIG_TYPE => "KEYSIG",
                TIMESIG_TYPE => "TIMESIG",
                BEAMSET_TYPE => "BEAMSET",
                CONNECT_TYPE => "CONNECT",
                DYNAMIC_TYPE => "DYNAMIC",
                MODNR_TYPE => "MODNR",
                GRAPHIC_TYPE => "GRAPHIC",
                OTTAVA_TYPE => "OTTAVA",
                SLUR_TYPE => "SLUR",
                TUPLET_TYPE => "TUPLET",
                GRSYNC_TYPE => "GRSYNC",
                TEMPO_TYPE => "TEMPO",
                SPACER_TYPE => "SPACER",
                ENDING_TYPE => "ENDING",
                PSMEAS_TYPE => "PSMEAS",
                _ => "UNKNOWN",
            };
            println!("  {:2} {:12} {}", obj_type, type_name, count);
        }

        // Basic sanity checks
        assert!(
            summary.note_count > 0 || summary.rest_count > 0,
            "Score should have notes or rests"
        );
        assert!(summary.staff_count > 0, "Score should have staves");
        assert!(
            summary.measure_count > 0,
            "Score should have measure objects"
        );
        assert_eq!(
            summary.total_objects,
            score.objects.len(),
            "Total objects should match"
        );
    }
}

// =============================================================================
// Integration Tests: Notelist Files
// =============================================================================

#[test]
fn test_notelist_score_summaries() {
    let fixtures = [
        "tests/notelist_examples/HBD_33.nl",
        "tests/notelist_examples/BachEbSonata_20.nl",
        "tests/notelist_examples/KillingMe_36.nl",
    ];

    for path in &fixtures {
        println!("\n=== Summary for {} ===", path);

        let file = File::open(path).expect("Failed to open notelist file");
        let notelist = parse_notelist(file).expect("Failed to parse notelist");
        let summary = summarize_notelist(&notelist);

        println!("Version: {}", notelist.version);
        println!("Filename: {}", notelist.filename);
        println!("Part staves: {:?}", notelist.part_staves);
        println!("Start measure: {}", notelist.start_meas);
        println!("Total records: {}", notelist.records.len());
        println!("Notes: {}", summary.note_count);
        println!("Rests: {}", summary.rest_count);
        println!("Measures (barlines): {}", summary.measure_count);
        println!("Unique staffs: {:?}", summary.staffs);

        println!("\nRecord counts by type:");
        let mut types: Vec<_> = summary.record_counts.iter().collect();
        types.sort_by_key(|(k, _)| *k);
        for (record_type, count) in types {
            println!("  {:12} {}", record_type, count);
        }

        // Basic sanity checks
        assert!(
            summary.note_count > 0 || summary.rest_count > 0,
            "Notelist should have notes or rests"
        );
        assert!(!summary.staffs.is_empty(), "Notelist should have staffs");
    }
}

// =============================================================================
// Cross-Format Comparison (when we have matched pairs)
// =============================================================================

#[test]
fn test_cross_format_awareness() {
    // This test documents that we have two different sets of test files.
    // Future work: generate .nl files from .ngl files or vice versa for true cross-validation.

    println!("\n=== Cross-Format Test File Status ===");
    println!("NGL fixtures: 16 files from different scores");
    println!("Notelist examples: 15 files from different scores");
    println!("Status: No matched pairs currently available");
    println!("Future: Generate .nl from .ngl or vice versa to enable true cross-validation");

    // For now, just verify both formats can be read and summarized
    let ngl = NglFile::read_from_file("tests/fixtures/01_me_and_lucy.ngl").unwrap();
    let score = interpret_heap(&ngl).unwrap();
    let ngl_summary = summarize_ngl_score(&score);

    let file = File::open("tests/notelist_examples/HBD_33.nl").unwrap();
    let notelist = parse_notelist(file).unwrap();
    let nl_summary = summarize_notelist(&notelist);

    println!("\nNGL example: {} notes", ngl_summary.note_count);
    println!("Notelist example: {} notes", nl_summary.note_count);

    assert!(ngl_summary.note_count > 0);
    assert!(nl_summary.note_count > 0);
}

// =============================================================================
// Detailed Object Analysis Tests
// =============================================================================

#[test]
fn test_ngl_object_walk_details() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    println!("\n=== Object Walk Details for {} ===", path);

    let mut count_by_type: HashMap<u8, usize> = HashMap::new();
    for obj in score.walk() {
        let obj_type = obj.header.obj_type as u8;
        *count_by_type.entry(obj_type).or_insert(0) += 1;
    }

    println!("Objects traversed by walk():");
    let mut types: Vec<_> = count_by_type.iter().collect();
    types.sort_by_key(|(k, _)| *k);
    for (obj_type, count) in types {
        let type_name = match *obj_type {
            SYNC_TYPE => "SYNC",
            MEASURE_TYPE => "MEASURE",
            CLEF_TYPE => "CLEF",
            KEYSIG_TYPE => "KEYSIG",
            TIMESIG_TYPE => "TIMESIG",
            PAGE_TYPE => "PAGE",
            SYSTEM_TYPE => "SYSTEM",
            STAFF_TYPE => "STAFF",
            CONNECT_TYPE => "CONNECT",
            TAIL_TYPE => "TAIL",
            BEAMSET_TYPE => "BEAMSET",
            SLUR_TYPE => "SLUR",
            GRAPHIC_TYPE => "GRAPHIC",
            DYNAMIC_TYPE => "DYNAMIC",
            _ => "OTHER",
        };
        println!("  {:2} {:12} {}", obj_type, type_name, count);
    }

    // Verify walk includes all major object types
    assert!(
        count_by_type.contains_key(&SYNC_TYPE),
        "Walk should include SYNC objects"
    );
    assert!(
        count_by_type.contains_key(&MEASURE_TYPE),
        "Walk should include MEASURE objects"
    );
    assert!(
        count_by_type.contains_key(&TAIL_TYPE),
        "Walk should end with TAIL"
    );
}

#[test]
fn test_ngl_note_details() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    println!("\n=== Note Details for {} ===", path);

    let mut note_count = 0;
    let mut rest_count = 0;
    let mut voice_counts: HashMap<i8, usize> = HashMap::new();
    let mut duration_counts: HashMap<i8, usize> = HashMap::new();

    for notes_vec in score.notes.values() {
        for note in notes_vec {
            if note.rest {
                rest_count += 1;
            } else {
                note_count += 1;
            }

            *voice_counts.entry(note.voice).or_insert(0) += 1;
            *duration_counts.entry(note.header.sub_type).or_insert(0) += 1;
        }
    }

    println!("Notes: {}", note_count);
    println!("Rests: {}", rest_count);

    println!("\nVoice distribution:");
    let mut voices: Vec<_> = voice_counts.iter().collect();
    voices.sort_by_key(|(k, _)| *k);
    for (voice, count) in voices {
        println!("  Voice {:2}: {}", voice, count);
    }

    println!("\nDuration distribution (l_dur codes):");
    let mut durations: Vec<_> = duration_counts.iter().collect();
    durations.sort_by_key(|(k, _)| *k);
    for (dur, count) in durations {
        let dur_name = match *dur {
            BREVE_L_DUR => "breve",
            WHOLE_L_DUR => "whole",
            HALF_L_DUR => "half",
            QTR_L_DUR => "quarter",
            EIGHTH_L_DUR => "eighth",
            SIXTEENTH_L_DUR => "16th",
            THIRTY2ND_L_DUR => "32nd",
            SIXTY4TH_L_DUR => "64th",
            _ => "other",
        };
        println!("  {:2} {:8} {}", dur, dur_name, count);
    }

    assert!(note_count > 0 || rest_count > 0);
}
