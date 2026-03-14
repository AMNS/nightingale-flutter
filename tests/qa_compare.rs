/// QA Compare: Before/After Visual Diffs for Rendering Changes
///
/// Compares PDFs rendered before and after a code change, generating visual
/// diff images showing exactly what changed (matching pixels dimmed, changed
/// pixels in bright red). Only fixtures with visual changes are shown.
///
/// Usage:
/// ```bash
/// # Smart mode (default): Run via shell script (auto git checkout, render, compare)
/// ./scripts/qa-compare-smart.sh
///
/// # Manual mode: If you've already generated before/after PDFs:
/// cargo test --test qa_compare -- --nocapture
/// ```
///
/// Output: test-output/qa-compare/
///   before/          — PDFs + PNGs from HEAD~1
///   after/           — PDFs + PNGs from HEAD
///   diff/            — Diff images (red highlights) - ONLY for changed fixtures
///   changed.txt      — List of changed fixture names (for Flutter to load)
mod common;

use common::compare_images_and_diff;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Discover all before/after PNG pairs in the qa-compare directory.
fn find_qa_compare_pairs() -> Result<HashMap<String, (PathBuf, PathBuf)>, String> {
    let before_dir = Path::new("test-output/qa-compare/before");
    let after_dir = Path::new("test-output/qa-compare/after");

    if !before_dir.exists() || !after_dir.exists() {
        return Err(format!(
            "Before/after directories not found. Run ./scripts/qa-compare-smart.sh first.\n\
             Expected:\n  {}\n  {}",
            before_dir.display(),
            after_dir.display()
        ));
    }

    let mut pairs = HashMap::new();

    // Scan after directory for PNG files
    for entry in fs::read_dir(after_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("png") {
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                let before_png = before_dir.join(format!("{}.png", name));

                if before_png.exists() {
                    pairs.insert(name.to_string(), (before_png, path.clone()));
                }
            }
        }
    }

    if pairs.is_empty() {
        return Err(
            "No PNG pairs found. Ensure test-output/qa-compare/{before,after}/ contain PNGs."
                .to_string(),
        );
    }

    Ok(pairs)
}

#[test]
fn qa_compare_before_after() {
    println!("\n=== QA Compare: Before/After Visual Diffs ===\n");

    // Discover all PNG pairs
    let pairs = match find_qa_compare_pairs() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    println!("Found {} fixture(s) to compare\n", pairs.len());

    // Create diff directory
    let diff_dir = Path::new("test-output/qa-compare/diff");
    fs::create_dir_all(diff_dir).expect("create diff directory");

    // Compare each pair and collect results
    let mut changed_fixtures = Vec::new();
    let mut unchanged_count = 0;

    for (name, (before_path, after_path)) in pairs.iter() {
        let diff_path = diff_dir.join(format!("{}_diff.png", name));

        match compare_images_and_diff(before_path, after_path, &diff_path) {
            Ok((total, diff_px, pct)) => {
                if diff_px > 0 {
                    // Only track fixtures with visual changes
                    println!(
                        "⚠ CHANGED: {} — {:.3}% ({} / {} pixels)",
                        name, pct, diff_px, total
                    );
                    changed_fixtures.push((name.clone(), pct, diff_px, total));
                } else {
                    unchanged_count += 1;
                    println!("✓ UNCHANGED: {}", name);
                    // Remove diff image if it exists (no changes)
                    let _ = fs::remove_file(&diff_path);
                }
            }
            Err(e) => {
                eprintln!("✗ ERROR comparing {}: {}", name, e);
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Total fixtures:     {}", pairs.len());
    println!("Changed:            {}", changed_fixtures.len());
    println!("Unchanged:          {}", unchanged_count);

    // Write manifest of changed fixtures for Flutter to load
    if !changed_fixtures.is_empty() {
        // Sort by diff percentage (highest first)
        changed_fixtures.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let manifest_path = Path::new("test-output/qa-compare/changed.txt");
        let manifest_content: String = changed_fixtures
            .iter()
            .map(|(name, pct, diff_px, total)| format!("{}|{:.3}|{}/{}", name, pct, diff_px, total))
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(manifest_path, manifest_content).expect("write changed.txt");

        println!("\n📋 Changed fixtures manifest:");
        println!("   {}", manifest_path.display());
        println!("\n🎨 Review changes in Flutter:");
        println!("   cd nightingale && flutter run");
        println!("   Then navigate to: QA Compare (Before/After) screen");

        // Fail the test if there are changes (so CI catches regressions)
        panic!(
            "\n⚠ Visual changes detected in {} fixture(s).\n\
             Review in Flutter QA Compare screen.",
            changed_fixtures.len()
        );
    } else {
        // No changes - write empty manifest
        let manifest_path = Path::new("test-output/qa-compare/changed.txt");
        fs::write(manifest_path, "").expect("write empty changed.txt");

        println!("\n✓ No visual changes detected. All fixtures match.");
    }
}
