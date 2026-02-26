//! Visual diff tool for golden bitmap changes.
//!
//! Compares current golden bitmaps (tests/golden_bitmaps/) against the last
//! committed versions (from git HEAD). Generates diff images highlighting
//! pixel-level changes in red.
//!
//! Run:  cargo test --test golden_diff -- --nocapture
//!
//! Output goes to /tmp/nightingale-test-output/golden-diff/
//! Each changed file gets three images:
//!   {name}_old.png    — committed version (from git HEAD)
//!   {name}_new.png    — current working-tree version
//!   {name}_diff.png   — visual diff (matching=dimmed, changed=red)

mod common;

use common::{DiffEntry, NewEntry};
use std::fs;
use std::path::Path;
use std::process::Command;

const DIFF_DIR: &str = "/tmp/nightingale-test-output/golden-diff";
const REPORT_PATH: &str = "/tmp/nightingale-test-output/golden-diff/review.html";

/// Extract a file's contents from git HEAD.
fn git_show_head(repo_file: &str) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(["show", &format!("HEAD:{}", repo_file)])
        .output()
        .ok()?;
    if output.status.success() {
        Some(output.stdout)
    } else {
        None
    }
}

#[test]
fn diff_changed_golden_bitmaps() {
    let golden_dir = Path::new("tests/golden_bitmaps");
    let golden_dir_abs = fs::canonicalize(golden_dir).expect("canonicalize golden dir");
    let diff_dir = Path::new(DIFF_DIR);
    fs::create_dir_all(diff_dir).unwrap();

    // Find all golden bitmaps in working tree (both NGL and Notelist)
    let entries: Vec<_> = fs::read_dir(golden_dir)
        .expect("tests/golden_bitmaps/ must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
        .collect();

    let mut changed: Vec<DiffEntry> = Vec::new();
    let mut new_entries: Vec<NewEntry> = Vec::new();
    let mut unchanged = 0;

    for entry in &entries {
        let path = entry.path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let repo_path = format!("tests/golden_bitmaps/{}", filename);

        // Get the committed version
        let old_bytes = match git_show_head(&repo_path) {
            Some(b) => b,
            None => {
                let new_path = diff_dir.join(format!("{}_new.png", stem));
                fs::copy(&path, &new_path).ok();
                eprintln!("[NEW] {}", filename);
                new_entries.push(NewEntry {
                    name: filename.to_string(),
                    new_path: new_path.to_string_lossy().to_string(),
                });
                continue;
            }
        };

        // Read current version
        let new_bytes = fs::read(&path).unwrap();

        if old_bytes == new_bytes {
            unchanged += 1;
            continue;
        }

        // Write old + new to diff dir
        let old_path = diff_dir.join(format!("{}_old.png", stem));
        let new_path = diff_dir.join(format!("{}_new.png", stem));
        let diff_path = diff_dir.join(format!("{}_diff.png", stem));

        fs::write(&old_path, &old_bytes).unwrap();
        fs::copy(&path, &new_path).unwrap();

        match common::compare_images_and_diff(&old_path, &new_path, &diff_path) {
            Ok((total, diff_px, pct)) => {
                eprintln!(
                    "[CHANGED] {}  — {}/{} pixels differ ({:.2}%)",
                    filename, diff_px, total, pct
                );
                changed.push(DiffEntry {
                    name: filename.to_string(),
                    old_path: old_path.to_string_lossy().to_string(),
                    new_path: new_path.to_string_lossy().to_string(),
                    diff_path: diff_path.to_string_lossy().to_string(),
                    total_pixels: total,
                    diff_pixels: diff_px,
                    diff_pct: pct,
                    golden_path: golden_dir_abs.join(filename).to_string_lossy().to_string(),
                });
            }
            Err(e) => {
                eprintln!("[ERROR] {} — {}", filename, e);
            }
        }
    }

    // Generate HTML report
    if !changed.is_empty() || !new_entries.is_empty() {
        let report_path = Path::new(REPORT_PATH);
        match common::generate_html_diff_report(report_path, &changed, &new_entries, unchanged) {
            Ok(()) => eprintln!("\nHTML report: file://{}", report_path.display()),
            Err(e) => eprintln!("\nFailed to generate HTML report: {}", e),
        }
    }

    eprintln!(
        "\n=== Golden bitmap diff summary ===\n\
         Unchanged: {}\n\
         Changed:   {}\n\
         New:       {}\n\
         Diff dir:  {}",
        unchanged,
        changed.len(),
        new_entries.len(),
        diff_dir.display()
    );

    if !changed.is_empty() {
        eprintln!("\nChanged files (open diff dir to review):");
        for f in &changed {
            eprintln!("  {}", f.name);
        }
        eprintln!("\nopen {}\n", diff_dir.display());
    }
}
