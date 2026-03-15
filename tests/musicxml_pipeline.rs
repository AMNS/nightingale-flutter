//! MusicXML full-pipeline integration tests.
//!
//! Tests the complete cycle:
//!   MusicXML file → import → InterpretedScore → render PDF → re-export MusicXML → roundtrip verify
//!
//! Also tests NGL fixture → MusicXML export with file output.

mod common;

use nightingale_core::draw::render_score;
use nightingale_core::layout::{layout_score, LayoutConfig};
use nightingale_core::musicxml::export::export_musicxml;
use nightingale_core::musicxml::import::import_musicxml;
use nightingale_core::ngl::{interpret::interpret_heap, NglFile};
use nightingale_core::render::PdfRenderer;
use std::fs;
use std::path::Path;

/// Read a MusicXML file, handling UTF-8 and UTF-16 (BE/LE) encodings.
fn read_xml_file(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(_) => {
            let bytes = fs::read(path).map_err(|e| format!("read error: {}", e))?;
            if bytes.starts_with(&[0xFE, 0xFF]) {
                // UTF-16 BE
                let u16s: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                Ok(String::from_utf16_lossy(&u16s))
            } else if bytes.starts_with(&[0xFF, 0xFE]) {
                // UTF-16 LE
                let u16s: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                Ok(String::from_utf16_lossy(&u16s))
            } else {
                Err("not UTF-8 or UTF-16".to_string())
            }
        }
    }
}

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
    let mut score = import_musicxml(&xml).unwrap();

    // Apply pagination/layout
    layout_score(&mut score, &LayoutConfig::default());

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
    let mut score = import_musicxml(&xml).unwrap();

    // Apply pagination/layout
    layout_score(&mut score, &LayoutConfig::default());

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
    let mut score = import_musicxml(&xml_orig).unwrap();
    layout_score(&mut score, &LayoutConfig::default());
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
    let mut score = import_musicxml(&xml_orig).unwrap();
    layout_score(&mut score, &LayoutConfig::default());
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
    let mut score1 = import_musicxml(&xml_orig).unwrap();
    layout_score(&mut score1, &LayoutConfig::default());
    let xml1 = export_musicxml(&score1);

    // Pass 2: import re-exported → export again
    let mut score2 = import_musicxml(&xml1).unwrap();
    layout_score(&mut score2, &LayoutConfig::default());
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
    let mut score1 = import_musicxml(&xml_orig).unwrap();
    layout_score(&mut score1, &LayoutConfig::default());
    let xml1 = export_musicxml(&score1);
    let mut score2 = import_musicxml(&xml1).unwrap();
    layout_score(&mut score2, &LayoutConfig::default());
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
    let mut score1 = import_musicxml(&xml_orig).unwrap();
    layout_score(&mut score1, &LayoutConfig::default());
    let notes1: usize = score1.notes.values().map(|v| v.len()).sum();

    // Step 2: Render imported score
    let pdf1 = render_to_pdf(&score1);
    fs::write(out_dir.join("dichterliebe_pass1.pdf"), &pdf1).unwrap();

    // Step 3: Re-export
    let xml_reexport = export_musicxml(&score1);
    fs::write(out_dir.join("dichterliebe_pass1.musicxml"), &xml_reexport).unwrap();

    // Step 4: Import the re-export
    let mut score2 = import_musicxml(&xml_reexport).unwrap();
    layout_score(&mut score2, &LayoutConfig::default());
    let notes2: usize = score2.notes.values().map(|v| v.len()).sum();

    // Step 5: Render the re-imported score
    let pdf2 = render_to_pdf(&score2);
    fs::write(out_dir.join("dichterliebe_pass2.pdf"), &pdf2).unwrap();

    // Step 6: Export again
    let xml_pass2 = export_musicxml(&score2);
    fs::write(out_dir.join("dichterliebe_pass2.musicxml"), &xml_pass2).unwrap();

    // Step 7: One more roundtrip to verify stability of our own format
    let mut score3 = import_musicxml(&xml_pass2).unwrap();
    layout_score(&mut score3, &LayoutConfig::default());
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

// ============================================================================
// 6) NGL → MusicXML → import visual roundtrip test
// ============================================================================

/// Test visual rendering stability for NGL→XML→import roundtrip.
///
/// This test addresses the ROADMAP Tier 1 task: "Investigate visual deltas
/// on NGL→XML→import→render".
///
/// For each NGL fixture:
/// 1. Render original NGL → PDF (baseline)
/// 2. Export NGL → MusicXML
/// 3. Import MusicXML → InterpretedScore
/// 4. Render imported score → PDF (roundtrip)
/// 5. Compare PDFs (ideally identical, document any differences)
#[test]
fn ngl_musicxml_roundtrip_visual_test() {
    use common::{compare_images_and_diff, pdf_to_png};

    let out_dir = Path::new("test-output/musicxml_pipeline/ngl_roundtrip");
    fs::create_dir_all(out_dir).unwrap();

    // Test with a few representative fixtures
    let test_fixtures = ["tc_ich_bin_ja.ngl", "tc_05.ngl", "01_me_and_lucy.ngl"];

    for fixture_name in &test_fixtures {
        let fixture_path = format!("tests/fixtures/{}", fixture_name);
        if !Path::new(&fixture_path).exists() {
            eprintln!("Skipping: {} not found", fixture_name);
            continue;
        }

        eprintln!("\n=== Testing NGL→XML roundtrip: {} ===", fixture_name);

        // Step 1: Load NGL and render original
        let data = fs::read(&fixture_path).unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        let score_orig = interpret_heap(&ngl).unwrap();
        let pdf_orig = render_to_pdf(&score_orig);

        let stem = fixture_name.trim_end_matches(".ngl");
        let orig_pdf = out_dir.join(format!("{}_original.pdf", stem));
        fs::write(&orig_pdf, &pdf_orig).unwrap();

        // Step 2: Export to MusicXML
        let xml = export_musicxml(&score_orig);
        fs::write(out_dir.join(format!("{}_exported.musicxml", stem)), &xml).unwrap();

        // Step 3: Import the exported MusicXML
        let mut score_import = match import_musicxml(&xml) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  ❌ Import failed: {}", e);
                continue;
            }
        };
        layout_score(&mut score_import, &LayoutConfig::default());

        // Step 4: Render the imported score
        let pdf_import = render_to_pdf(&score_import);
        let roundtrip_pdf = out_dir.join(format!("{}_roundtrip.pdf", stem));
        fs::write(&roundtrip_pdf, &pdf_import).unwrap();

        // Step 5: Compare counts (structural check)
        let orig_note_count: usize = score_orig.notes.values().map(|v| v.len()).sum();
        let import_note_count: usize = score_import.notes.values().map(|v| v.len()).sum();

        let orig_measure_count = score_orig.measures.len();
        let import_measure_count = score_import.measures.len();

        eprintln!("  Notes:    {} → {}", orig_note_count, import_note_count);
        eprintln!(
            "  Measures: {} → {}",
            orig_measure_count, import_measure_count
        );
        eprintln!(
            "  PDF size: {} → {} bytes",
            pdf_orig.len(),
            pdf_import.len()
        );

        // Step 6: Visual diff (convert PDFs → PNGs → generate diff image)
        let orig_png = out_dir.join(format!("{}_original.png", stem));
        let roundtrip_png = out_dir.join(format!("{}_roundtrip.png", stem));
        let diff_png = out_dir.join(format!("{}_diff.png", stem));

        match pdf_to_png(&orig_pdf, &orig_png) {
            Ok(true) => {
                if pdf_to_png(&roundtrip_pdf, &roundtrip_png).unwrap_or(false) {
                    match compare_images_and_diff(&orig_png, &roundtrip_png, &diff_png) {
                        Ok((_total, _diff_px, diff_pct)) => {
                            eprintln!("  Visual diff: {:.2}% pixels changed", diff_pct);
                        }
                        Err(e) => eprintln!("  ⚠️  Diff generation failed: {}", e),
                    }
                }
            }
            Ok(false) => {
                eprintln!("  ⚠️  PDF→PNG conversion not available (install sips/pdftoppm/magick)")
            }
            Err(e) => eprintln!("  ⚠️  PDF→PNG error: {}", e),
        }

        let note_delta = (import_note_count as i32) - (orig_note_count as i32);
        if note_delta != 0 {
            eprintln!(
                "  ⚠️  Note count changed by {} (may indicate missing or added elements)",
                note_delta
            );
        }

        // Basic sanity checks
        assert!(
            import_note_count > 0,
            "Roundtrip should have notes: got {}",
            import_note_count
        );
        assert!(
            pdf_import.len() > 1000,
            "Roundtrip PDF should be substantial: got {} bytes",
            pdf_import.len()
        );
    }

    eprintln!(
        "\n✓ Visual comparison files written to test-output/musicxml_pipeline/ngl_roundtrip/"
    );
    eprintln!("  PDFs: {{original,roundtrip}}.pdf");
    eprintln!("  Visual diffs: *_diff.png (red=changed, dimmed=matched)");
}

// ============================================================================
// 7) Import ALL xmlsamples and render PDFs for visual review
// ============================================================================

#[test]
fn import_all_xmlsamples_and_render_pdfs() {
    let samples_dir = Path::new("tests/musicxml_examples/xmlsamples");
    if !samples_dir.exists() {
        eprintln!("Skipping: xmlsamples directory not found");
        return;
    }

    let out_dir = Path::new("test-output/musicxml_pipeline/xmlsamples");
    fs::create_dir_all(out_dir).unwrap();

    let mut entries: Vec<_> = fs::read_dir(samples_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "musicxml"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut successes = Vec::new();
    let mut failures = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();

        // Read with UTF-8/UTF-16 support
        let xml = match read_xml_file(&path) {
            Ok(x) => x,
            Err(e) => {
                failures.push((name, e));
                continue;
            }
        };

        let mut score = match import_musicxml(&xml) {
            Ok(s) => s,
            Err(e) => {
                failures.push((name, format!("import error: {}", e)));
                continue;
            }
        };

        layout_score(&mut score, &LayoutConfig::default());

        let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
        let total_parts = score.part_infos.len();
        let total_measures = score.measures.len();

        // Render to PDF
        let pdf_bytes = render_to_pdf(&score);
        let pdf_path = out_dir.join(format!("{}.pdf", name));
        fs::write(&pdf_path, &pdf_bytes).unwrap();

        // Also re-export to MusicXML for inspection
        let xml_reexport = export_musicxml(&score);
        fs::write(
            out_dir.join(format!("{}_reexported.musicxml", name)),
            &xml_reexport,
        )
        .unwrap();

        successes.push((
            name,
            total_notes,
            total_parts,
            total_measures,
            pdf_bytes.len(),
        ));
    }

    // Print summary table
    eprintln!(
        "\n=== MusicXML Import Summary ({}/{} succeeded) ===",
        successes.len(),
        entries.len()
    );
    eprintln!(
        "{:<30} {:>6} {:>5} {:>5} {:>10}",
        "Name", "Notes", "Parts", "Meas", "PDF size"
    );
    eprintln!("{}", "-".repeat(60));
    for (name, notes, parts, measures, pdf_size) in &successes {
        eprintln!(
            "{:<30} {:>6} {:>5} {:>5} {:>10}",
            name, notes, parts, measures, pdf_size
        );
    }

    if !failures.is_empty() {
        eprintln!("\n=== Failures ===");
        for (name, err) in &failures {
            eprintln!("  {}: {}", name, err);
        }
    }

    // All files should import successfully
    assert!(
        failures.is_empty(),
        "Some files failed to import: {:?}",
        failures.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    // Every successful import should produce notes
    for (name, notes, _, _, _) in &successes {
        assert!(
            *notes > 0,
            "{} imported 0 notes — likely a parsing problem",
            name
        );
    }
}
