/// QA Compare: Before/After Visual Diffs for Rendering Changes
///
/// Compares PDFs rendered before and after a code change using the high-quality
/// PDF rendering path (the "QA Compare" section). Generates visual diff images
/// showing exactly what changed.
///
/// Usage:
/// ```bash
/// cargo test --test qa_compare -- --nocapture
/// ```
mod common;

use std::path::Path;

#[test]
fn grace_note_accidental_offset_scaling() {
    // Fixture: tc_old_kinderszenen_13_6.ngl (has grace notes with accidentals)
    let before_png = Path::new("test-output/audit/before/tc_old_kinderszenen_13_6.png");
    let after_png = Path::new("test-output/audit/after/tc_old_kinderszenen_13_6.png");
    let diff_path = Path::new("test-output/qa-diffs/tc_old_kinderszenen_13_6_diff.png");

    std::fs::create_dir_all("test-output/qa-diffs").expect("create qa-diffs dir");

    if !before_png.exists() || !after_png.exists() {
        eprintln!("Before/after PNGs not found. Generate them with:");
        eprintln!("  git checkout HEAD~1 && cargo test --test ngl_all kinderszenen");
        eprintln!("  cp test-output/ngl/tc_old_kinderszenen_13_6.pdf test-output/audit/before/");
        eprintln!("  sips -s format png -s dpiWidth 150 -s dpiHeight 150 test-output/audit/before/tc_old_kinderszenen_13_6.pdf --out test-output/audit/before/tc_old_kinderszenen_13_6.png");
        eprintln!("  git checkout main");
        return;
    }

    match common::compare_images_and_diff(before_png, after_png, diff_path) {
        Ok((total, diff, pct)) => {
            println!("\n=== tc_old_kinderszenen_13_6 (grace notes with accidentals) ===");
            println!("Total pixels: {}", total);
            println!("Differing pixels: {}", diff);
            println!("Diff percentage: {:.2}%", pct);
            println!("Diff image: {}\n", diff_path.display());

            if pct < 0.1 {
                println!(
                    "✓ PASS: Minimal change ({:.2}%), consistent with grace note offset scaling",
                    pct
                );
            } else if pct < 1.0 {
                println!(
                    "⚠ WARNING: Moderate change ({:.2}%), review diff image",
                    pct
                );
            } else {
                println!(
                    "✗ FAIL: Large change ({:.2}%), verify rendering is correct",
                    pct
                );
            }
        }
        Err(e) => eprintln!("Error comparing images: {}", e),
    }
}

#[test]
fn beamed_grace_notes() {
    // Fixture: beamed_grace_notes.ngl (complex grace note patterns)
    let before_png = Path::new("test-output/audit/before/beamed_grace_notes.png");
    let after_png = Path::new("test-output/audit/after/beamed_grace_notes.png");
    let diff_path = Path::new("test-output/qa-diffs/beamed_grace_notes_diff.png");

    std::fs::create_dir_all("test-output/qa-diffs").expect("create qa-diffs dir");

    if !before_png.exists() || !after_png.exists() {
        eprintln!("Before/after PNGs not found.");
        return;
    }

    match common::compare_images_and_diff(before_png, after_png, diff_path) {
        Ok((total, diff, pct)) => {
            println!("\n=== beamed_grace_notes.ngl (complex grace note patterns) ===");
            println!("Total pixels: {}", total);
            println!("Differing pixels: {}", diff);
            println!("Diff percentage: {:.2}%", pct);
            println!("Diff image: {}\n", diff_path.display());

            if pct < 0.5 {
                println!("✓ PASS: Minimal change ({:.2}%)", pct);
            } else {
                println!(
                    "⚠ WARNING: Moderate change ({:.2}%), review diff image",
                    pct
                );
            }
        }
        Err(e) => eprintln!("Error comparing images: {}", e),
    }
}
