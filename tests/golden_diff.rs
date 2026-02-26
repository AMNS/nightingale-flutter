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

use image::{GenericImageView, Rgba, RgbaImage};
use std::fs;
use std::path::Path;
use std::process::Command;

const DIFF_DIR: &str = "/tmp/nightingale-test-output/golden-diff";

/// Compare two images pixel-by-pixel, produce a diff image.
///
/// Returns (total_pixels, diff_pixels, diff_pct).
/// Diff image: matching pixels shown dimmed (30% opacity), differences in bright red.
fn compare_images_and_diff(
    old_path: &Path,
    new_path: &Path,
    diff_path: &Path,
) -> Result<(u64, u64, f64), String> {
    let old = image::open(old_path).map_err(|e| format!("open old: {}", e))?;
    let new = image::open(new_path).map_err(|e| format!("open new: {}", e))?;

    let (ow, oh) = old.dimensions();
    let (nw, nh) = new.dimensions();

    let w = ow.max(nw);
    let h = oh.max(nh);
    let total = w as u64 * h as u64;

    let mut diff_img = RgbaImage::new(w, h);
    let mut diff_count: u64 = 0;

    for y in 0..h {
        for x in 0..w {
            let opx = if x < ow && y < oh {
                old.get_pixel(x, y)
            } else {
                Rgba([255, 255, 255, 255])
            };
            let npx = if x < nw && y < nh {
                new.get_pixel(x, y)
            } else {
                Rgba([255, 255, 255, 255])
            };

            if opx == npx {
                // Match: dimmed
                let r = (opx[0] as u16 * 30 + 255 * 70) / 100;
                let g = (opx[1] as u16 * 30 + 255 * 70) / 100;
                let b = (opx[2] as u16 * 30 + 255 * 70) / 100;
                diff_img.put_pixel(x, y, Rgba([r as u8, g as u8, b as u8, 255]));
            } else {
                diff_img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
                diff_count += 1;
            }
        }
    }

    diff_img
        .save(diff_path)
        .map_err(|e| format!("save diff: {}", e))?;

    let pct = if total > 0 {
        diff_count as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    Ok((total, diff_count, pct))
}

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
    let diff_dir = Path::new(DIFF_DIR);
    fs::create_dir_all(diff_dir).unwrap();

    // Find all golden bitmaps in working tree
    let entries: Vec<_> = fs::read_dir(golden_dir)
        .expect("tests/golden_bitmaps/ must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
        .collect();

    let mut changed = Vec::new();
    let mut unchanged = 0;
    let mut new_files = 0;

    for entry in &entries {
        let path = entry.path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let repo_path = format!("tests/golden_bitmaps/{}", filename);

        // Get the committed version
        let old_bytes = match git_show_head(&repo_path) {
            Some(b) => b,
            None => {
                new_files += 1;
                eprintln!("[NEW] {}", filename);
                // Copy new file to diff dir for review
                fs::copy(&path, diff_dir.join(format!("{}_new.png", stem))).ok();
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

        match compare_images_and_diff(&old_path, &new_path, &diff_path) {
            Ok((total, diff_px, pct)) => {
                eprintln!(
                    "[CHANGED] {}  — {}/{} pixels differ ({:.2}%)",
                    filename, diff_px, total, pct
                );
                changed.push(filename.to_string());
            }
            Err(e) => {
                eprintln!("[ERROR] {} — {}", filename, e);
            }
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
        new_files,
        diff_dir.display()
    );

    if !changed.is_empty() {
        eprintln!("\nChanged files (open diff dir to review):");
        for f in &changed {
            eprintln!("  {}", f);
        }
        eprintln!("\nopen {}\n", diff_dir.display());
    }
}
