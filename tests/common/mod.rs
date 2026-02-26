//! Shared test utilities for bitmap regression and visual diff testing.
//!
//! Used by: ngl_all.rs, notelist_all.rs, golden_diff.rs

#![allow(dead_code)]

use image::{GenericImageView, Rgba, RgbaImage};
use std::fmt::Write as FmtWrite;
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

/// An entry in the visual diff report.
pub struct DiffEntry {
    /// Display name (e.g. "nl_clef_change_page1")
    pub name: String,
    /// Absolute path to old (committed) image
    pub old_path: String,
    /// Absolute path to new (working tree) image
    pub new_path: String,
    /// Absolute path to diff image
    pub diff_path: String,
    /// Total pixel count
    pub total_pixels: u64,
    /// Number of differing pixels
    pub diff_pixels: u64,
    /// Diff percentage
    pub diff_pct: f64,
    /// Path to the golden bitmap (for approval)
    pub golden_path: String,
}

/// A new file that has no previous version.
pub struct NewEntry {
    pub name: String,
    pub new_path: String,
}

/// Generate an HTML visual diff report.
///
/// Creates a self-contained HTML file with side-by-side before/after/diff
/// images, per-file approval checkboxes, and a "generate approve command"
/// button. Images are referenced via file:// URIs (local only).
pub fn generate_html_diff_report(
    report_path: &Path,
    changed: &[DiffEntry],
    new_files: &[NewEntry],
    unchanged_count: usize,
) -> Result<(), String> {
    let mut html = String::with_capacity(8192);

    // Header + CSS
    write!(
        html,
        r##"<!DOCTYPE html>
<html lang="en"><head>
<meta charset="utf-8">
<title>Nightingale Visual Diff Review</title>
<style>
  * {{ box-sizing: border-box; }}
  body {{ font-family: system-ui, -apple-system, sans-serif; max-width: 1400px;
         margin: 0 auto; padding: 20px; background: #fafafa; color: #333; }}
  h1 {{ border-bottom: 2px solid #333; padding-bottom: 8px; }}
  .summary {{ background: #fff; border: 1px solid #ddd; border-radius: 8px;
              padding: 16px; margin: 16px 0; display: flex; gap: 24px; }}
  .summary .stat {{ text-align: center; }}
  .summary .stat .num {{ font-size: 32px; font-weight: bold; }}
  .summary .stat .label {{ font-size: 13px; color: #888; }}
  .stat.changed .num {{ color: #c33; }}
  .stat.new .num {{ color: #393; }}
  .stat.unchanged .num {{ color: #999; }}
  .entry {{ background: #fff; border: 1px solid #ddd; border-radius: 8px;
            margin: 20px 0; padding: 20px; }}
  .entry.changed {{ border-left: 4px solid #c33; }}
  .entry.new {{ border-left: 4px solid #393; }}
  .entry-header {{ display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }}
  .entry-header h2 {{ margin: 0; font-size: 18px; }}
  .entry-header .pct {{ font-size: 14px; color: #c33; font-weight: normal; }}
  .images {{ display: flex; gap: 8px; margin: 12px 0; }}
  .images .col {{ flex: 1; text-align: center; }}
  .images .col img {{ width: 100%; border: 1px solid #eee; cursor: pointer;
                      transition: transform 0.2s; }}
  .images .col img:hover {{ transform: scale(1.02); }}
  .images .col .label {{ font-size: 12px; color: #999; margin-bottom: 4px; }}
  .approve-bar {{ display: flex; align-items: center; gap: 12px; margin-top: 12px;
                  padding-top: 12px; border-top: 1px solid #eee; }}
  .approve-bar label {{ cursor: pointer; user-select: none; }}
  .approve-bar input[type=checkbox] {{ width: 18px; height: 18px; }}
  .cmd {{ font-family: monospace; font-size: 12px; color: #666; background: #f5f5f5;
          padding: 4px 8px; border-radius: 4px; }}
  .actions {{ background: #fff; border: 1px solid #ddd; border-radius: 8px;
              padding: 20px; margin: 24px 0; }}
  .actions h2 {{ margin-top: 0; }}
  button {{ background: #333; color: #fff; border: none; padding: 10px 20px;
           border-radius: 6px; cursor: pointer; font-size: 14px; }}
  button:hover {{ background: #555; }}
  button.approve {{ background: #393; }}
  button.approve:hover {{ background: #2a7a2a; }}
  #cmd-output {{ font-family: monospace; font-size: 13px; background: #1e1e1e;
                 color: #d4d4d4; padding: 16px; border-radius: 6px; margin-top: 12px;
                 white-space: pre-wrap; display: none; }}
  .modal {{ display: none; position: fixed; top: 0; left: 0; width: 100%; height: 100%;
            background: rgba(0,0,0,0.85); z-index: 1000; cursor: pointer;
            justify-content: center; align-items: center; }}
  .modal img {{ max-width: 95%; max-height: 95%; object-fit: contain; }}
  .modal.active {{ display: flex; }}
  .new-img {{ max-width: 50%; }}
</style>
</head><body>
<h1>Nightingale Visual Diff Review</h1>
"##
    )
    .unwrap();

    // Summary bar
    write!(
        html,
        r#"<div class="summary">
  <div class="stat changed"><div class="num">{}</div><div class="label">Changed</div></div>
  <div class="stat new"><div class="num">{}</div><div class="label">New</div></div>
  <div class="stat unchanged"><div class="num">{}</div><div class="label">Unchanged</div></div>
</div>
"#,
        changed.len(),
        new_files.len(),
        unchanged_count
    )
    .unwrap();

    if changed.is_empty() && new_files.is_empty() {
        write!(
            html,
            "<p>No changes detected. All golden bitmaps match.</p>"
        )
        .unwrap();
    }

    // Changed entries
    for (i, entry) in changed.iter().enumerate() {
        write!(
            html,
            r##"<div class="entry changed" id="entry-{i}">
  <div class="entry-header">
    <input type="checkbox" id="cb-{i}" class="approve-cb" data-golden="{golden}" data-new="{new_p}" checked>
    <h2>{name}</h2>
    <span class="pct">{diff_px} pixels differ ({pct:.2}%)</span>
  </div>
  <div class="images">
    <div class="col"><div class="label">Before</div><img src="file://{old}" onclick="showModal(this)" alt="before"></div>
    <div class="col"><div class="label">Diff</div><img src="file://{diff}" onclick="showModal(this)" alt="diff"></div>
    <div class="col"><div class="label">After</div><img src="file://{new_p}" onclick="showModal(this)" alt="after"></div>
  </div>
  <div class="approve-bar">
    <label for="cb-{i}">Approve this change</label>
    <span class="cmd">cp "{new_p}" "{golden}"</span>
  </div>
</div>
"##,
            i = i,
            name = entry.name,
            old = entry.old_path,
            new_p = entry.new_path,
            diff = entry.diff_path,
            golden = entry.golden_path,
            diff_px = entry.diff_pixels,
            pct = entry.diff_pct,
        )
        .unwrap();
    }

    // New file entries
    for (i, entry) in new_files.iter().enumerate() {
        let idx = changed.len() + i;
        write!(
            html,
            r##"<div class="entry new" id="entry-{idx}">
  <div class="entry-header">
    <h2>{name}</h2>
    <span class="pct" style="color:#393">New file</span>
  </div>
  <img class="new-img" src="file://{new_p}" onclick="showModal(this)" alt="new">
</div>
"##,
            idx = idx,
            name = entry.name,
            new_p = entry.new_path,
        )
        .unwrap();
    }

    // Action bar
    if !changed.is_empty() {
        write!(
            html,
            r##"<div class="actions">
  <h2>Approve Changes</h2>
  <p>Check the changes you want to keep, then generate the approval command:</p>
  <button class="approve" onclick="generateCmd()">Generate Approval Command</button>
  <button onclick="selectAll(true)">Select All</button>
  <button onclick="selectAll(false)">Deselect All</button>
  <div id="cmd-output"></div>
</div>
"##
        )
        .unwrap();
    }

    // JavaScript: modal zoom + command generation
    write!(
        html,
        r##"<div class="modal" id="modal" onclick="this.classList.remove('active')">
  <img id="modal-img" src="">
</div>
<script>
function showModal(img) {{
  const m = document.getElementById('modal');
  document.getElementById('modal-img').src = img.src;
  m.classList.add('active');
}}
function selectAll(val) {{
  document.querySelectorAll('.approve-cb').forEach(cb => cb.checked = val);
}}
function generateCmd() {{
  const out = document.getElementById('cmd-output');
  const lines = [];
  document.querySelectorAll('.approve-cb:checked').forEach(cb => {{
    lines.push('cp "' + cb.dataset.new + '" "' + cb.dataset.golden + '"');
  }});
  if (lines.length === 0) {{
    out.textContent = '# No changes selected.';
  }} else {{
    out.textContent = '#!/bin/sh\\n# Approve selected golden bitmap changes\\n\\n' + lines.join('\\n') + '\\n\\necho "Approved ' + lines.length + ' golden bitmap(s)."';
  }}
  out.style.display = 'block';
  // Copy to clipboard
  navigator.clipboard.writeText(out.textContent).then(() => {{
    out.textContent += '\\n\\n# (Copied to clipboard)';
  }}).catch(() => {{}});
}}
</script>
</body></html>
"##
    )
    .unwrap();

    fs::write(report_path, &html).map_err(|e| format!("write HTML: {}", e))?;
    Ok(())
}
