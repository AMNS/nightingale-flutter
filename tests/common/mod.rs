//! Shared test utilities for bitmap regression and visual diff testing.
//!
//! Used by: ngl_all.rs, notelist_all.rs, golden_diff.rs

#![allow(dead_code)]

use image::{GenericImageView, Rgba, RgbaImage};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Try to convert a PDF to PNG using available system tools.
///
/// Falls back through: sips (macOS) → pdftoppm (poppler-utils) → magick (ImageMagick).
/// Returns Ok(true) if conversion succeeded, Ok(false) if no tool available.
pub fn pdf_to_png(pdf_path: &Path, png_path: &Path) -> Result<bool, String> {
    // Try sips (macOS built-in)
    if let Ok(output) = Command::new("sips")
        .args([
            "-s",
            "format",
            "png",
            "-s",
            "dpiHeight",
            "72",
            "-s",
            "dpiWidth",
            "72",
        ])
        .arg(pdf_path)
        .arg("--out")
        .arg(png_path)
        .output()
    {
        if output.status.success() {
            return Ok(true);
        }
    }

    // Try pdftoppm (poppler-utils, common on Linux)
    let prefix = png_path.with_extension("");
    if let Ok(output) = Command::new("pdftoppm")
        .args(["-png", "-r", "72", "-f", "1", "-l", "1"])
        .arg(pdf_path)
        .arg(&prefix)
        .output()
    {
        if output.status.success() {
            // pdftoppm outputs prefix-1.png (or prefix-01.png)
            let candidates = [
                prefix.with_extension("").to_string_lossy().to_string() + "-1.png",
                prefix.with_extension("").to_string_lossy().to_string() + "-01.png",
            ];
            for cand in &candidates {
                let p = Path::new(cand);
                if p.exists() {
                    fs::rename(p, png_path).map_err(|e| e.to_string())?;
                    return Ok(true);
                }
            }
        }
    }

    // Try ImageMagick (cross-platform)
    if let Ok(output) = Command::new("magick")
        .args(["convert", "-density", "72"])
        .arg(format!("{}[0]", pdf_path.display())) // [0] = first page
        .arg(png_path)
        .output()
    {
        if output.status.success() {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Compare two images pixel-by-pixel and generate a visual diff.
///
/// Returns (total_pixels, diff_pixels, diff_pct).
/// Writes a diff image to `diff_path` where:
/// - Matching pixels are shown at 30% opacity (dimmed)
/// - Different pixels are shown in bright red
pub fn compare_images_and_diff(
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
                // Match: show dimmed (30% opacity blend with white)
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
