// Bridge API: OG vs Modern rendering comparison.
//
// Provides functions for the Flutter QA Compare screen to:
// 1. List fixtures with OG reference PDFs
// 2. Render comparisons (our bitmap + OG bitmap + diff)
// 3. Save/load feedback

use nightingale_core::comparison::{
    compare_rgba_images, og_page_count, render_og_page, OG_FIXTURES,
};
use nightingale_core::draw::draw_high_level::render_score;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::render::{BitmapRenderer, MusicRenderer};
use std::fs;

// ── DTO types ───────────────────────────────────────────────────

/// Information about a fixture with OG reference.
#[derive(Debug, Clone)]
pub struct OgFixtureInfo {
    /// Fixture name (e.g. "tc_02")
    pub fixture_name: String,
    /// OG reference PDF filename
    pub og_pdf: String,
    /// Number of pages in the OG PDF (0 if unreadable)
    pub og_page_count: i32,
    /// Number of pages in our rendering
    pub our_page_count: i32,
    /// Whether the OG PDF file exists on disk
    pub og_exists: bool,
}

/// Result of comparing one page.
#[derive(Debug, Clone)]
pub struct ComparisonPageResult {
    /// Our rendered RGBA bitmap (width * height * 4 bytes)
    pub ours_rgba: Vec<u8>,
    pub ours_width: u32,
    pub ours_height: u32,
    /// OG reference RGBA bitmap
    pub og_rgba: Vec<u8>,
    pub og_width: u32,
    pub og_height: u32,
    /// Diff RGBA bitmap (matching=dimmed, different=red)
    pub diff_rgba: Vec<u8>,
    pub diff_width: u32,
    pub diff_height: u32,
    /// Total pixel count
    pub total_pixels: u64,
    /// Number of differing pixels
    pub diff_pixels: u64,
    /// Diff percentage (0.0-100.0)
    pub diff_pct: f64,
}

// ── Font loading helper ─────────────────────────────────────────

/// Load music fonts and text fonts into a BitmapRenderer.
///
/// Loads Sonata for OG-accurate glyph shapes, with Bravura as fallback.
fn load_fonts(renderer: &mut BitmapRenderer, font_dir: &str) {
    let font_dir_path = std::path::Path::new(font_dir);
    // Load Sonata first (for OG comparison — glyphs match reference PDFs)
    let sonata_path = font_dir_path.join("Sonata.ttf");
    if sonata_path.exists() {
        if let Ok(data) = fs::read(&sonata_path) {
            renderer.load_sonata_font(data);
        }
    }
    // Bravura as fallback for any glyphs not in Sonata
    let bravura_path = font_dir_path.join("Bravura.otf");
    if bravura_path.exists() {
        if let Ok(data) = fs::read(&bravura_path) {
            renderer.load_music_font(data);
        }
    }
    renderer.load_text_fonts_from_dir(font_dir_path);
}

// ── Public API ──────────────────────────────────────────────────

/// List all OG fixtures with metadata.
///
/// `project_root` is the absolute path to the nightingale-modernize directory.
/// Used to locate tests/fixtures/ and tests/og_reference/.
pub fn list_og_fixtures(project_root: String) -> Vec<OgFixtureInfo> {
    eprintln!("[compare] list_og_fixtures: project_root={}", project_root);
    let fixture_dir = format!("{}/tests/fixtures", project_root);
    let og_ref_dir = format!("{}/tests/og_reference", project_root);
    eprintln!(
        "[compare]   fixture_dir exists={}, og_ref_dir exists={}",
        std::path::Path::new(&fixture_dir).exists(),
        std::path::Path::new(&og_ref_dir).exists()
    );

    OG_FIXTURES
        .iter()
        .map(|f| {
            let og_pdf_path = format!("{}/{}", og_ref_dir, f.og_pdf);
            let og_exists = std::path::Path::new(&og_pdf_path).exists();
            eprintln!(
                "[compare]   {} — og_pdf={} exists={}",
                f.fixture_name, og_pdf_path, og_exists
            );

            let og_pages = if og_exists {
                og_page_count(&og_ref_dir, f.og_pdf).unwrap_or(0) as i32
            } else {
                0
            };

            // Count our pages by doing a quick render
            let ngl_path = format!("{}/{}.ngl", fixture_dir, f.fixture_name);
            let font_dir = format!("{}/assets/fonts", project_root);
            let our_pages = match fs::read(&ngl_path) {
                Ok(data) => match NglFile::read_from_bytes(&data) {
                    Ok(ngl) => match interpret_heap(&ngl) {
                        Ok(score) => {
                            let mut r = BitmapRenderer::new(72.0);
                            r.set_page_size(score.page_width_pt, score.page_height_pt);
                            load_fonts(&mut r, &font_dir);
                            render_score(&score, &mut r);
                            r.page_count() as i32
                        }
                        Err(e) => {
                            eprintln!("[compare]   {} — interpret_heap error: {:?}", f.fixture_name, e);
                            0
                        }
                    },
                    Err(e) => {
                        eprintln!("[compare]   {} — NglFile parse error: {:?}", f.fixture_name, e);
                        0
                    }
                },
                Err(e) => {
                    eprintln!("[compare]   {} — read NGL error: {} (path={})", f.fixture_name, e, ngl_path);
                    0
                }
            };

            OgFixtureInfo {
                fixture_name: f.fixture_name.to_string(),
                og_pdf: f.og_pdf.to_string(),
                og_page_count: og_pages,
                our_page_count: our_pages,
                og_exists,
            }
        })
        .collect()
}

/// Get a comparison for a specific fixture page.
///
/// `project_root` is the absolute path to nightingale-modernize.
/// `fixture_name` is e.g. "tc_02".
/// `page_num` is 1-based.
///
/// Returns an empty result if the fixture or page can't be rendered.
pub fn get_comparison(
    project_root: String,
    fixture_name: String,
    page_num: i32,
) -> ComparisonPageResult {
    eprintln!(
        "[compare] get_comparison: root={} fixture={} page={}",
        project_root, fixture_name, page_num
    );

    let empty = ComparisonPageResult {
        ours_rgba: vec![],
        ours_width: 0,
        ours_height: 0,
        og_rgba: vec![],
        og_width: 0,
        og_height: 0,
        diff_rgba: vec![],
        diff_width: 0,
        diff_height: 0,
        total_pixels: 0,
        diff_pixels: 0,
        diff_pct: 0.0,
    };

    // Find the OG fixture
    let fixture = match OG_FIXTURES.iter().find(|f| f.fixture_name == fixture_name) {
        Some(f) => f,
        None => {
            eprintln!("[compare]   FAIL: fixture '{}' not found in OG_FIXTURES", fixture_name);
            return empty;
        }
    };

    let fixture_dir = format!("{}/tests/fixtures", project_root);
    let og_ref_dir = format!("{}/tests/og_reference", project_root);
    let font_dir = format!("{}/assets/fonts", project_root);

    // Render our version
    let ngl_path = format!("{}/{}.ngl", fixture_dir, fixture_name);
    eprintln!("[compare]   ngl_path={} exists={}", ngl_path, std::path::Path::new(&ngl_path).exists());
    let data = match fs::read(&ngl_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[compare]   FAIL: read NGL: {}", e);
            return empty;
        }
    };
    let ngl = match NglFile::read_from_bytes(&data) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("[compare]   FAIL: parse NGL: {:?}", e);
            return empty;
        }
    };
    let score = match interpret_heap(&ngl) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[compare]   FAIL: interpret_heap: {:?}", e);
            return empty;
        }
    };

    let mut renderer = BitmapRenderer::new(72.0);
    renderer.set_page_size(score.page_width_pt, score.page_height_pt);
    load_fonts(&mut renderer, &font_dir);
    render_score(&score, &mut renderer);

    let page_idx = (page_num - 1) as usize;
    eprintln!(
        "[compare]   rendered {} pages, requesting page_idx={}",
        renderer.page_count(),
        page_idx
    );
    let (ours_rgba, ours_w, ours_h) = match (
        renderer.page_data(page_idx),
        renderer.page_dimensions(page_idx),
    ) {
        (Some(data), Some((w, h))) => (data.to_vec(), w, h),
        _ => {
            eprintln!("[compare]   FAIL: page_data or page_dimensions returned None for idx {}", page_idx);
            return empty;
        }
    };
    eprintln!("[compare]   ours: {}x{}", ours_w, ours_h);

    // Render OG page
    let og_pdf_path = format!("{}/{}", og_ref_dir, fixture.og_pdf);
    eprintln!("[compare]   og_pdf={} exists={}", og_pdf_path, std::path::Path::new(&og_pdf_path).exists());
    let og = match render_og_page(&og_ref_dir, fixture.og_pdf, page_num as usize, 72.0) {
        Some(r) => r,
        None => {
            eprintln!("[compare]   FAIL: render_og_page returned None");
            return empty;
        }
    };
    eprintln!("[compare]   og: {}x{}", og.width, og.height);

    // Compare
    let (diff_rgba, diff_w, diff_h, total, diff_px, pct) =
        compare_rgba_images(&ours_rgba, ours_w, ours_h, &og.rgba, og.width, og.height);

    eprintln!(
        "[compare]   diff: {:.2}% ({}/{} px) canvas {}x{}",
        pct, diff_px, total, diff_w, diff_h
    );

    ComparisonPageResult {
        ours_rgba,
        ours_width: ours_w,
        ours_height: ours_h,
        og_rgba: og.rgba,
        og_width: og.width,
        og_height: og.height,
        diff_rgba,
        diff_width: diff_w,
        diff_height: diff_h,
        total_pixels: total,
        diff_pixels: diff_px,
        diff_pct: pct,
    }
}
