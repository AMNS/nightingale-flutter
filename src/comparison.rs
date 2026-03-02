//! Image comparison engine for OG vs modern rendering.
//!
//! Provides pixel-level diffing between our BitmapRenderer output and
//! OG Nightingale reference PDFs (rendered via CoreGraphics on macOS).
//!
//! Used by both:
//! - `cargo test --test og_comparison` (headless CI)
//! - Flutter QA Compare screen (interactive review)

use crate::og_render::{self, RenderedPage};

/// Mapping from fixture names to OG reference PDF filenames.
///
/// All 8 Tim Crawford fixtures are registered here.
/// tc_ich_bin_ja and tc_schildt were originally EPS/PS only; converted to PDF
/// via ps2pdf from the og_reference/ source files.
pub const OG_FIXTURES: &[OgFixture] = &[
    OgFixture {
        fixture_name: "tc_02",
        og_pdf: "02.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_03a",
        og_pdf: "03a.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_03b",
        og_pdf: "03b.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_04",
        og_pdf: "04.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_05",
        og_pdf: "05.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_55_1",
        og_pdf: "55_1.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_ich_bin_ja",
        og_pdf: "ich_bin_ja.ng.pdf",
    },
    OgFixture {
        fixture_name: "tc_schildt",
        og_pdf: "schildt.ng.pdf",
    },
];

/// A fixture with an OG reference PDF.
pub struct OgFixture {
    pub fixture_name: &'static str,
    pub og_pdf: &'static str,
}

/// Result of comparing one page of a fixture.
#[derive(Debug, Clone)]
pub struct PageComparison {
    /// Fixture name (e.g. "tc_02").
    pub fixture_name: String,
    /// 1-based page number.
    pub page_num: usize,
    /// Our rendered RGBA pixels.
    pub ours_rgba: Vec<u8>,
    pub ours_width: u32,
    pub ours_height: u32,
    /// OG reference rendered RGBA pixels.
    pub og_rgba: Vec<u8>,
    pub og_width: u32,
    pub og_height: u32,
    /// Diff image RGBA pixels (matching=dimmed, different=red).
    pub diff_rgba: Vec<u8>,
    pub diff_width: u32,
    pub diff_height: u32,
    /// Total pixel count in the comparison area.
    pub total_pixels: u64,
    /// Number of pixels that differ.
    pub diff_pixels: u64,
    /// Percentage of pixels that differ (0.0–100.0).
    pub diff_pct: f64,
}

/// Result summary for a complete fixture comparison.
#[derive(Debug, Clone)]
pub struct FixtureComparison {
    pub fixture_name: String,
    pub page_count: usize,
    pub pages: Vec<PageComparison>,
    /// Average diff percentage across all pages.
    pub avg_diff_pct: f64,
}

/// Compare two RGBA images pixel-by-pixel, producing a diff image.
///
/// Returns `(diff_rgba, width, height, total_pixels, diff_pixels, diff_pct)`.
///
/// The diff image shows:
/// - Matching pixels: dimmed (30% opacity blend with white)
/// - Different pixels: bright red
pub fn compare_rgba_images(
    img_a: &[u8],
    w_a: u32,
    h_a: u32,
    img_b: &[u8],
    w_b: u32,
    h_b: u32,
) -> (Vec<u8>, u32, u32, u64, u64, f64) {
    let w = w_a.max(w_b);
    let h = h_a.max(h_b);
    let total = w as u64 * h as u64;
    let mut diff_rgba = vec![255u8; (w * h * 4) as usize];
    let mut diff_count: u64 = 0;

    for y in 0..h {
        for x in 0..w {
            let px_a = if x < w_a && y < h_a {
                let idx = ((y * w_a + x) * 4) as usize;
                [img_a[idx], img_a[idx + 1], img_a[idx + 2], img_a[idx + 3]]
            } else {
                [255, 255, 255, 255]
            };
            let px_b = if x < w_b && y < h_b {
                let idx = ((y * w_b + x) * 4) as usize;
                [img_b[idx], img_b[idx + 1], img_b[idx + 2], img_b[idx + 3]]
            } else {
                [255, 255, 255, 255]
            };

            let out_idx = ((y * w + x) * 4) as usize;
            if px_a == px_b {
                // Match: show dimmed (30% opacity blend with white)
                diff_rgba[out_idx] = ((px_a[0] as u16 * 30 + 255 * 70) / 100) as u8;
                diff_rgba[out_idx + 1] = ((px_a[1] as u16 * 30 + 255 * 70) / 100) as u8;
                diff_rgba[out_idx + 2] = ((px_a[2] as u16 * 30 + 255 * 70) / 100) as u8;
                diff_rgba[out_idx + 3] = 255;
            } else {
                // Diff: bright red
                diff_rgba[out_idx] = 255;
                diff_rgba[out_idx + 1] = 0;
                diff_rgba[out_idx + 2] = 0;
                diff_rgba[out_idx + 3] = 255;
                diff_count += 1;
            }
        }
    }

    let pct = if total > 0 {
        diff_count as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    (diff_rgba, w, h, total, diff_count, pct)
}

/// Render a page from an OG reference PDF at the given DPI.
///
/// Thin wrapper around `og_render::render_pdf_page` with path construction.
pub fn render_og_page(
    og_ref_dir: &str,
    og_pdf: &str,
    page_num: usize,
    dpi: f64,
) -> Option<RenderedPage> {
    let pdf_path = format!("{}/{}", og_ref_dir, og_pdf);
    og_render::render_pdf_page(&pdf_path, page_num, dpi)
}

/// Get the page count for an OG reference PDF.
pub fn og_page_count(og_ref_dir: &str, og_pdf: &str) -> Option<usize> {
    let pdf_path = format!("{}/{}", og_ref_dir, og_pdf);
    og_render::pdf_page_count(&pdf_path)
}
