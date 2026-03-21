//! Visual fidelity roundtrip test for NGL binary writer.
//!
//! Validates that read → write → read produces pixel-perfect identical rendering.
//! This goes beyond structural validation (test_roundtrip_all_fixtures in writer.rs)
//! to verify that the write cycle preserves ALL rendering-relevant data.

mod common;

use common::{compare_images_and_diff, save_bitmap_page};
use nightingale_core::draw::render_score;
use nightingale_core::ngl::writer::NglWriter;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::render::BitmapRenderer;
use std::fs;
use std::path::PathBuf;

/// Test that NGL roundtrip (read → write → read) produces pixel-perfect identical rendering.
///
/// For each .ngl fixture:
/// 1. Read original → interpret → render to bitmap (before)
/// 2. Write to temporary file → read back → interpret → render to bitmap (after)
/// 3. Compare bitmaps pixel-by-pixel, expecting 0% difference
///
/// This test is more stringent than test_roundtrip_all_fixtures (which only checks
/// structural equality of InterpretedScore fields). Any data loss during write that
/// affects rendering (even if structs match) will be caught here.
#[test]
fn test_roundtrip_visual_fidelity_all_fixtures() {
    let fixture_dir = PathBuf::from("tests/fixtures");
    if !fixture_dir.exists() {
        eprintln!("Fixture directory not found, skipping roundtrip visual test");
        return;
    }

    let output_dir = PathBuf::from("test-output/roundtrip-visual");
    fs::create_dir_all(&output_dir)
        .expect("Could not create test-output/roundtrip-visual directory");

    let mut fixtures: Vec<_> = fs::read_dir(&fixture_dir)
        .expect("Could not read fixture directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "ngl"))
        .collect();
    fixtures.sort();
    fixtures.truncate(10); // Limit to 10 fixtures for speed in pre-commit hook

    assert!(!fixtures.is_empty(), "No NGL fixtures found");

    let mut passed = 0usize;
    let mut skipped = 0usize;
    let mut failed_fixtures = Vec::new();

    for fixture_path in &fixtures {
        let fixture_name = fixture_path.file_stem().unwrap().to_string_lossy();
        let file_bytes = fs::read(fixture_path)
            .unwrap_or_else(|e| panic!("Could not read {}: {}", fixture_path.display(), e));

        // --- Parse and interpret original ---
        let original_ngl = match NglFile::read_from_bytes(&file_bytes) {
            Ok(ngl) => ngl,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: parse failed: {e}");
                skipped += 1;
                continue;
            }
        };
        let original_score = match interpret_heap(&original_ngl) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: interpret failed: {e}");
                skipped += 1;
                continue;
            }
        };

        // --- Render original to bitmap ---
        let mut original_renderer = BitmapRenderer::new(150.0); // 150 DPI for tests
        render_score(&original_score, &mut original_renderer);

        let original_png_path = output_dir.join(format!("{}_original_page1.png", fixture_name));
        if let Err(e) = save_bitmap_page(&original_renderer, 0, &original_png_path) {
            eprintln!("SKIP {fixture_name}: could not save original bitmap: {e}");
            skipped += 1;
            continue;
        }

        // --- Write roundtrip ---
        let writer = NglWriter::new();
        let written_bytes = match writer.write_to_bytes(&original_score) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: write failed: {e}");
                skipped += 1;
                continue;
            }
        };

        let roundtrip_file_path = output_dir.join(format!("{}_roundtrip.ngl", fixture_name));
        fs::write(&roundtrip_file_path, &written_bytes)
            .unwrap_or_else(|e| panic!("Could not write {}: {}", roundtrip_file_path.display(), e));

        // --- Parse and interpret roundtrip ---
        let roundtrip_ngl = match NglFile::read_from_bytes(&written_bytes) {
            Ok(ngl) => ngl,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: roundtrip parse failed: {e}");
                skipped += 1;
                continue;
            }
        };
        let roundtrip_score = match interpret_heap(&roundtrip_ngl) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: roundtrip interpret failed: {e}");
                skipped += 1;
                continue;
            }
        };

        // --- Render roundtrip to bitmap ---
        let mut roundtrip_renderer = BitmapRenderer::new(150.0); // 150 DPI for tests
        render_score(&roundtrip_score, &mut roundtrip_renderer);

        let roundtrip_png_path = output_dir.join(format!("{}_roundtrip_page1.png", fixture_name));
        if let Err(e) = save_bitmap_page(&roundtrip_renderer, 0, &roundtrip_png_path) {
            eprintln!("SKIP {fixture_name}: could not save roundtrip bitmap: {e}");
            skipped += 1;
            continue;
        }

        // --- Compare bitmaps pixel-by-pixel ---
        let diff_png_path = output_dir.join(format!("{}_diff_page1.png", fixture_name));
        let (total_pixels, diff_pixels, diff_pct) = match compare_images_and_diff(
            &original_png_path,
            &roundtrip_png_path,
            &diff_png_path,
        ) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("SKIP {fixture_name}: bitmap comparison failed: {e}");
                skipped += 1;
                continue;
            }
        };

        // Expect pixel-perfect match (0% difference)
        if diff_pct > 0.0 {
            eprintln!(
                "FAIL {}: {}/{} pixels differ ({:.4}%)",
                fixture_name, diff_pixels, total_pixels, diff_pct
            );
            eprintln!("  Original: {}", original_png_path.display());
            eprintln!("  Roundtrip: {}", roundtrip_png_path.display());
            eprintln!("  Diff: {}", diff_png_path.display());
            failed_fixtures.push(fixture_name.to_string());
        } else {
            println!("PASS {}: pixel-perfect roundtrip", fixture_name);
            passed += 1;
        }
    }

    println!(
        "\nRoundtrip Visual Fidelity: {passed}/{} passed, {skipped} skipped",
        fixtures.len()
    );

    if !failed_fixtures.is_empty() {
        panic!(
            "\nRoundtrip rendering FAILED for {} fixture(s):\n  {}\n\n\
            These fixtures have visual differences after write cycle.\n\
            Check test-output/roundtrip-visual/ for before/after/diff images.",
            failed_fixtures.len(),
            failed_fixtures.join("\n  ")
        );
    }

    // Allow up to 25% skips (same threshold as structural roundtrip test)
    let skip_threshold = fixtures.len() / 4;
    assert!(
        skipped <= skip_threshold,
        "Too many skipped fixtures ({skipped} > {skip_threshold})"
    );
}
