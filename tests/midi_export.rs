//! MIDI export integration tests.
//!
//! Generates actual .mid files to test-output/midi/ for all NGL fixtures.
//! This allows manual verification by opening files in a DAW or MIDI player.

use nightingale_core::midi::export;
use nightingale_core::ngl::{interpret_heap, NglFile};
use std::fs;
use std::path::PathBuf;

/// Export all NGL fixtures to MIDI files.
///
/// For each .ngl fixture:
/// 1. Read and interpret the score
/// 2. Export to Standard MIDI File format
/// 3. Write .mid file to test-output/midi/
///
/// This test doesn't validate MIDI correctness (that's what unit tests do),
/// but it produces tangible output you can play/inspect.
#[test]
fn test_export_all_fixtures_to_midi() {
    let fixture_dir = PathBuf::from("tests/fixtures");
    if !fixture_dir.exists() {
        eprintln!("Fixture directory not found, skipping MIDI export test");
        return;
    }

    let output_dir = PathBuf::from("test-output/midi");
    fs::create_dir_all(&output_dir).expect("Could not create test-output/midi directory");

    let mut fixtures: Vec<_> = fs::read_dir(&fixture_dir)
        .expect("Could not read fixture directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "ngl"))
        .collect();
    fixtures.sort();

    assert!(!fixtures.is_empty(), "No NGL fixtures found");

    let mut exported = 0usize;
    let mut skipped = 0usize;

    for fixture_path in &fixtures {
        let fixture_name = fixture_path.file_stem().unwrap().to_string_lossy();
        let file_bytes = fs::read(fixture_path)
            .unwrap_or_else(|e| panic!("Could not read {}: {}", fixture_path.display(), e));

        // Parse and interpret
        let ngl = match NglFile::read_from_bytes(&file_bytes) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: parse failed: {e}");
                skipped += 1;
                continue;
            }
        };

        let score = match interpret_heap(&ngl) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: interpret failed: {e}");
                skipped += 1;
                continue;
            }
        };

        // Export to MIDI
        let midi_bytes = export::export_to_midi(&score);

        // Write .mid file
        let midi_path = output_dir.join(format!("{}.mid", fixture_name));
        fs::write(&midi_path, &midi_bytes)
            .unwrap_or_else(|e| panic!("Could not write {}: {}", midi_path.display(), e));

        println!(
            "EXPORTED {}: {} bytes → {}",
            fixture_name,
            midi_bytes.len(),
            midi_path.display()
        );
        exported += 1;
    }

    println!(
        "\nMIDI Export: {exported}/{} exported, {skipped} skipped",
        fixtures.len()
    );
    println!("Output directory: {}", output_dir.display());

    // Require at least 50% success rate
    assert!(
        exported >= fixtures.len() / 2,
        "Too few fixtures exported ({exported} < {})",
        fixtures.len() / 2
    );
}
