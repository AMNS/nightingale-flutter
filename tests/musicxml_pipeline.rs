//! MusicXML full-pipeline integration tests.
//!
//! Tests the complete cycle:
//!   MusicXML file → import → InterpretedScore → render PDF → re-export MusicXML → roundtrip verify
//!
//! Also tests NGL fixture → MusicXML export with file output.

use nightingale_core::draw::render_score;
use nightingale_core::musicxml::export::export_musicxml;
use nightingale_core::musicxml::import::import_musicxml;
use nightingale_core::ngl::{interpret::interpret_heap, NglFile};
use nightingale_core::render::PdfRenderer;
use std::fs;
use std::path::Path;

/// Helper: render an InterpretedScore to PDF bytes
fn render_to_pdf(score: &nightingale_core::ngl::interpret::InterpretedScore) -> Vec<u8> {
    let page_w = if score.page_width_pt > 0.0 {
        score.page_width_pt
    } else {
        612.0
    };
    let page_h = if score.page_height_pt > 0.0 {
        score.page_height_pt
    } else {
        792.0
    };
    let mut pdf = PdfRenderer::new(page_w, page_h);
    let font_path = Path::new("assets/fonts/Bravura.otf");
    if font_path.exists() {
        pdf.load_music_font_file(font_path);
    }
    render_score(score, &mut pdf);
    pdf.finish()
}

/// Helper: count occurrences of a tag in XML
fn count_tag(xml: &str, tag: &str) -> usize {
    xml.matches(tag).count()
}

// ============================================================================
// 1) Canonical MusicXML import → Nightingale rendering
// ============================================================================

#[test]
fn import_dichterliebe_and_render_pdf() {
    let xml_path = "tests/musicxml_examples/Dichterliebe01.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    // Import
    let xml = fs::read_to_string(xml_path).unwrap();
    let score = import_musicxml(&xml).unwrap();

    // Verify import quality
    let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
    let total_parts = score.part_infos.len();
    eprintln!(
        "Dichterliebe: {} notes across {} parts",
        total_notes, total_parts
    );
    assert!(
        total_notes > 100,
        "Should import many notes: got {}",
        total_notes
    );
    assert!(
        total_parts >= 2,
        "Should have voice + piano: got {}",
        total_parts
    );

    // Render to PDF
    let pdf_bytes = render_to_pdf(&score);
    assert!(pdf_bytes.len() > 1000, "PDF should be non-trivial");
    assert!(pdf_bytes.starts_with(b"%PDF"), "Should be valid PDF");

    // Write output files
    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(out_dir.join("dichterliebe_imported.pdf"), &pdf_bytes).unwrap();
    eprintln!(
        "Wrote dichterliebe_imported.pdf ({} bytes)",
        pdf_bytes.len()
    );
}

#[test]
fn import_actor_prelude_and_render_pdf() {
    let xml_path = "tests/musicxml_examples/ActorPreludeSample.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    // Import
    let xml = fs::read_to_string(xml_path).unwrap();
    let score = import_musicxml(&xml).unwrap();

    // Verify import quality — orchestral score
    let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
    let total_parts = score.part_infos.len();
    eprintln!(
        "Actor Prelude: {} notes across {} parts",
        total_notes, total_parts
    );
    assert!(
        total_notes > 200,
        "Orchestral should have many notes: got {}",
        total_notes
    );
    assert!(
        total_parts >= 5,
        "Orchestral should have many parts: got {}",
        total_parts
    );

    // Render to PDF
    let pdf_bytes = render_to_pdf(&score);
    assert!(pdf_bytes.len() > 1000, "PDF should be non-trivial");

    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(out_dir.join("actor_prelude_imported.pdf"), &pdf_bytes).unwrap();
    eprintln!(
        "Wrote actor_prelude_imported.pdf ({} bytes)",
        pdf_bytes.len()
    );
}

// ============================================================================
// 2) MusicXML re-export after import
// ============================================================================

#[test]
fn dichterliebe_import_then_reexport() {
    let xml_path = "tests/musicxml_examples/Dichterliebe01.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    let xml_orig = fs::read_to_string(xml_path).unwrap();
    let score = import_musicxml(&xml_orig).unwrap();
    let xml_reexport = export_musicxml(&score);

    // Write re-exported file
    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(
        out_dir.join("dichterliebe_reexported.musicxml"),
        &xml_reexport,
    )
    .unwrap();

    // Verify structural integrity
    assert!(xml_reexport.contains("<score-partwise"));
    assert!(xml_reexport.contains("<part-list>"));
    assert!(count_tag(&xml_reexport, "<note>") > 100);

    eprintln!(
        "Re-exported Dichterliebe: {} bytes, {} notes, {} measures",
        xml_reexport.len(),
        count_tag(&xml_reexport, "<note>"),
        count_tag(&xml_reexport, "<measure ")
    );
}

#[test]
fn actor_prelude_import_then_reexport() {
    let xml_path = "tests/musicxml_examples/ActorPreludeSample.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    let xml_orig = fs::read_to_string(xml_path).unwrap();
    let score = import_musicxml(&xml_orig).unwrap();
    let xml_reexport = export_musicxml(&score);

    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(
        out_dir.join("actor_prelude_reexported.musicxml"),
        &xml_reexport,
    )
    .unwrap();

    assert!(xml_reexport.contains("<score-partwise"));
    assert!(count_tag(&xml_reexport, "<note>") > 200);
    assert!(count_tag(&xml_reexport, "<part id=") >= 5);

    eprintln!(
        "Re-exported Actor Prelude: {} bytes, {} notes, {} parts",
        xml_reexport.len(),
        count_tag(&xml_reexport, "<note>"),
        count_tag(&xml_reexport, "<part id=")
    );
}

// ============================================================================
// 3) Round-trip verification: original vs re-exported
// ============================================================================

#[test]
fn dichterliebe_roundtrip_fidelity() {
    let xml_path = "tests/musicxml_examples/Dichterliebe01.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    let xml_orig = fs::read_to_string(xml_path).unwrap();

    // Pass 1: import → export
    let score1 = import_musicxml(&xml_orig).unwrap();
    let xml1 = export_musicxml(&score1);

    // Pass 2: import re-exported → export again
    let score2 = import_musicxml(&xml1).unwrap();
    let xml2 = export_musicxml(&score2);

    // Compare structural element counts between pass 1 and pass 2
    // (These should be identical — our own format is stable)
    let notes1 = count_tag(&xml1, "<note>");
    let notes2 = count_tag(&xml2, "<note>");
    let pitches1 = count_tag(&xml1, "<pitch>");
    let pitches2 = count_tag(&xml2, "<pitch>");
    let rests1 = count_tag(&xml1, "<rest");
    let rests2 = count_tag(&xml2, "<rest");
    let measures1 = count_tag(&xml1, "<measure ");
    let measures2 = count_tag(&xml2, "<measure ");
    let parts1 = count_tag(&xml1, "<part id=");
    let parts2 = count_tag(&xml2, "<part id=");

    eprintln!("Dichterliebe roundtrip:");
    eprintln!("  Notes:    {} → {}", notes1, notes2);
    eprintln!("  Pitches:  {} → {}", pitches1, pitches2);
    eprintln!("  Rests:    {} → {}", rests1, rests2);
    eprintln!("  Measures: {} → {}", measures1, measures2);
    eprintln!("  Parts:    {} → {}", parts1, parts2);

    assert_eq!(
        notes1, notes2,
        "Note count must be stable across roundtrips"
    );
    assert_eq!(pitches1, pitches2, "Pitch count must be stable");
    assert_eq!(rests1, rests2, "Rest count must be stable");
    assert_eq!(measures1, measures2, "Measure count must be stable");
    assert_eq!(parts1, parts2, "Part count must be stable");

    // Also compare original → pass 1 (lossy direction)
    let orig_notes = count_tag(&xml_orig, "<note>");
    let orig_measures = count_tag(&xml_orig, "<measure ");
    let orig_parts = count_tag(&xml_orig, "<part id=");

    eprintln!("  Original notes: {}, imported: {}", orig_notes, notes1);
    eprintln!(
        "  Original measures: {}, imported: {}",
        orig_measures, measures1
    );
    eprintln!("  Original parts: {}, imported: {}", orig_parts, parts1);

    // Note count may differ (we may not handle all MusicXML features yet)
    // but it should be a substantial portion
    let note_ratio = notes1 as f64 / orig_notes as f64;
    assert!(
        note_ratio > 0.5,
        "Should preserve at least 50% of notes: {:.1}%",
        note_ratio * 100.0
    );
}

#[test]
fn actor_prelude_roundtrip_fidelity() {
    let xml_path = "tests/musicxml_examples/ActorPreludeSample.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    let xml_orig = fs::read_to_string(xml_path).unwrap();
    let score1 = import_musicxml(&xml_orig).unwrap();
    let xml1 = export_musicxml(&score1);
    let score2 = import_musicxml(&xml1).unwrap();
    let xml2 = export_musicxml(&score2);

    let notes1 = count_tag(&xml1, "<note>");
    let notes2 = count_tag(&xml2, "<note>");

    eprintln!("Actor Prelude roundtrip: {} → {} notes", notes1, notes2);
    assert_eq!(notes1, notes2, "Note count must be stable");

    let orig_notes = count_tag(&xml_orig, "<note>");
    let note_ratio = notes1 as f64 / orig_notes as f64;
    eprintln!(
        "  Original: {} notes, imported: {} ({:.1}%)",
        orig_notes,
        notes1,
        note_ratio * 100.0
    );
    assert!(
        note_ratio > 0.3,
        "Should preserve notes: {:.1}%",
        note_ratio * 100.0
    );
}

// ============================================================================
// 4) NGL fixture → MusicXML export
// ============================================================================

#[test]
fn export_ngl_orchestral_to_musicxml() {
    // tc_55_1 is our most complex NGL fixture (5-part orchestral)
    let fixture_path = "tests/fixtures/tc_55_1.ngl";
    if !Path::new(fixture_path).exists() {
        eprintln!("Skipping: {} not found", fixture_path);
        return;
    }

    let data = fs::read(fixture_path).unwrap();
    let ngl = NglFile::read_from_bytes(&data).unwrap();
    let score = interpret_heap(&ngl).unwrap();

    // Export to MusicXML
    let xml = export_musicxml(&score);

    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(out_dir.join("tc_55_1_exported.musicxml"), &xml).unwrap();

    let total_notes = count_tag(&xml, "<note>");
    let total_parts = count_tag(&xml, "<part id=");
    let total_measures = count_tag(&xml, "<measure ");

    eprintln!(
        "tc_55_1 NGL export: {} notes, {} parts, {} measures",
        total_notes, total_parts, total_measures
    );

    assert!(xml.contains("<score-partwise"));
    assert!(
        total_notes > 50,
        "Should have substantial notes: got {}",
        total_notes
    );
    assert!(
        total_parts >= 2,
        "Should have multiple parts: got {}",
        total_parts
    );
}

#[test]
fn export_ngl_ich_bin_ja_to_musicxml() {
    // tc_ich_bin_ja — 2-part vocal + lute, good for voice/instrument testing
    let fixture_path = "tests/fixtures/tc_ich_bin_ja.ngl";
    if !Path::new(fixture_path).exists() {
        eprintln!("Skipping: {} not found", fixture_path);
        return;
    }

    let data = fs::read(fixture_path).unwrap();
    let ngl = NglFile::read_from_bytes(&data).unwrap();
    let score = interpret_heap(&ngl).unwrap();

    let xml = export_musicxml(&score);

    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(out_dir.join("ich_bin_ja_exported.musicxml"), &xml).unwrap();

    let total_notes = count_tag(&xml, "<note>");
    eprintln!("ich_bin_ja NGL export: {} notes", total_notes);
    assert!(total_notes > 20, "Should have notes: got {}", total_notes);
}

#[test]
fn export_all_ngl_fixtures_to_musicxml() {
    let fixture_dir = "tests/fixtures";
    let out_dir = Path::new("test-output/musicxml_pipeline/ngl_exports");
    fs::create_dir_all(out_dir).unwrap();

    let entries = fs::read_dir(fixture_dir).unwrap();
    let mut count = 0;
    let mut total_notes = 0;

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "ngl") {
            let data = fs::read(&path).unwrap();
            let ngl = NglFile::read_from_bytes(&data).unwrap();
            let score = interpret_heap(&ngl).unwrap();
            let xml = export_musicxml(&score);

            let name = path.file_stem().unwrap().to_str().unwrap();
            fs::write(out_dir.join(format!("{}.musicxml", name)), &xml).unwrap();

            let notes = count_tag(&xml, "<note>");
            total_notes += notes;
            count += 1;
        }
    }

    eprintln!(
        "Exported {} NGL fixtures to MusicXML ({} total notes)",
        count, total_notes
    );
    assert!(count > 10, "Should export many fixtures: got {}", count);
}

// ============================================================================
// 5) Full pipeline: MusicXML → import → render → export → render again
// ============================================================================

#[test]
fn full_pipeline_dichterliebe() {
    let xml_path = "tests/musicxml_examples/Dichterliebe01.musicxml";
    if !Path::new(xml_path).exists() {
        eprintln!("Skipping: {} not found", xml_path);
        return;
    }

    let out_dir = Path::new("test-output/musicxml_pipeline");
    fs::create_dir_all(out_dir).unwrap();

    // Step 1: Import
    let xml_orig = fs::read_to_string(xml_path).unwrap();
    let score1 = import_musicxml(&xml_orig).unwrap();
    let notes1: usize = score1.notes.values().map(|v| v.len()).sum();

    // Step 2: Render imported score
    let pdf1 = render_to_pdf(&score1);
    fs::write(out_dir.join("dichterliebe_pass1.pdf"), &pdf1).unwrap();

    // Step 3: Re-export
    let xml_reexport = export_musicxml(&score1);
    fs::write(out_dir.join("dichterliebe_pass1.musicxml"), &xml_reexport).unwrap();

    // Step 4: Import the re-export
    let score2 = import_musicxml(&xml_reexport).unwrap();
    let notes2: usize = score2.notes.values().map(|v| v.len()).sum();

    // Step 5: Render the re-imported score
    let pdf2 = render_to_pdf(&score2);
    fs::write(out_dir.join("dichterliebe_pass2.pdf"), &pdf2).unwrap();

    // Step 6: Export again
    let xml_pass2 = export_musicxml(&score2);
    fs::write(out_dir.join("dichterliebe_pass2.musicxml"), &xml_pass2).unwrap();

    // Step 7: One more roundtrip to verify stability of our own format
    let score3 = import_musicxml(&xml_pass2).unwrap();
    let notes3: usize = score3.notes.values().map(|v| v.len()).sum();
    let xml_pass3 = export_musicxml(&score3);

    eprintln!("Full pipeline Dichterliebe:");
    eprintln!(
        "  Pass 1 (from external): {} notes, {} byte PDF",
        notes1,
        pdf1.len()
    );
    eprintln!(
        "  Pass 2 (from our XML):  {} notes, {} byte PDF",
        notes2,
        pdf2.len()
    );
    eprintln!("  Pass 3 (from our XML):  {} notes", notes3);
    eprintln!(
        "  XML sizes: orig={}, pass1={}, pass2={}, pass3={}",
        xml_orig.len(),
        xml_reexport.len(),
        xml_pass2.len(),
        xml_pass3.len()
    );

    // First import from external MusicXML may gain notes (whole-measure rests
    // synthesized by our exporter for empty measures). That's expected.
    // But from pass 2 onward, our own format must be perfectly stable.
    assert_eq!(
        notes2, notes3,
        "Note count must be stable from our own MusicXML format: pass2={}, pass3={}",
        notes2, notes3
    );

    // XML note tag counts should be stable from pass 1 onward (our exporter is
    // deterministic).
    let xml_notes_pass1 = count_tag(&xml_reexport, "<note>");
    let xml_notes_pass2 = count_tag(&xml_pass2, "<note>");
    let xml_notes_pass3 = count_tag(&xml_pass3, "<note>");
    assert_eq!(
        xml_notes_pass1, xml_notes_pass2,
        "XML note count must be stable: pass1={}, pass2={}",
        xml_notes_pass1, xml_notes_pass2
    );
    assert_eq!(
        xml_notes_pass2, xml_notes_pass3,
        "XML note count must be stable: pass2={}, pass3={}",
        xml_notes_pass2, xml_notes_pass3
    );

    // Report the note gain from external import
    if notes1 != notes2 {
        eprintln!(
            "  Note: external import had {} notes, our normalized format has {} (+{} from whole-measure rests)",
            notes1, notes2, notes2 - notes1
        );
    }
}
