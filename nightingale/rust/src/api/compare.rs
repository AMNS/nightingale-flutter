// Bridge API: OG vs Modern rendering comparison + QA before/after comparison.
//
// Provides functions for the Flutter QA Compare screen to:
// 1. List fixtures with OG reference PDFs
// 2. Render comparisons (our bitmap + OG bitmap + diff)
// 3. List and compare before/after PNG pairs from qa-compare workflow
// 4. Save/load feedback

use nightingale_core::comparison::{
    compare_rgba_images, og_page_count, render_og_page, OG_FIXTURES,
};
use nightingale_core::draw::draw_high_level::render_score;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::render::{BitmapRenderer, MusicRenderer};
use std::fs;
use std::path::Path;

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

/// Information about a before/after QA comparison fixture.
#[derive(Debug, Clone)]
pub struct QaCompareFixtureInfo {
    /// Fixture name (e.g. "grace_notes_test")
    pub fixture_name: String,
    /// Whether both before and after PNGs exist
    pub has_pair: bool,
}

/// Result of comparing before/after PNGs.
#[derive(Debug, Clone)]
pub struct QaComparisonResult {
    /// Before PNG RGBA bitmap (width * height * 4 bytes)
    pub before_rgba: Vec<u8>,
    pub before_width: u32,
    pub before_height: u32,
    /// After PNG RGBA bitmap
    pub after_rgba: Vec<u8>,
    pub after_width: u32,
    pub after_height: u32,
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
                            eprintln!(
                                "[compare]   {} — interpret_heap error: {:?}",
                                f.fixture_name, e
                            );
                            0
                        }
                    },
                    Err(e) => {
                        eprintln!(
                            "[compare]   {} — NglFile parse error: {:?}",
                            f.fixture_name, e
                        );
                        0
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[compare]   {} — read NGL error: {} (path={})",
                        f.fixture_name, e, ngl_path
                    );
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
            eprintln!(
                "[compare]   FAIL: fixture '{}' not found in OG_FIXTURES",
                fixture_name
            );
            return empty;
        }
    };

    let fixture_dir = format!("{}/tests/fixtures", project_root);
    let og_ref_dir = format!("{}/tests/og_reference", project_root);
    let font_dir = format!("{}/assets/fonts", project_root);

    // Render our version
    let ngl_path = format!("{}/{}.ngl", fixture_dir, fixture_name);
    eprintln!(
        "[compare]   ngl_path={} exists={}",
        ngl_path,
        std::path::Path::new(&ngl_path).exists()
    );
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
            eprintln!(
                "[compare]   FAIL: page_data or page_dimensions returned None for idx {}",
                page_idx
            );
            return empty;
        }
    };
    eprintln!("[compare]   ours: {}x{}", ours_w, ours_h);

    // Render OG page
    let og_pdf_path = format!("{}/{}", og_ref_dir, fixture.og_pdf);
    eprintln!(
        "[compare]   og_pdf={} exists={}",
        og_pdf_path,
        std::path::Path::new(&og_pdf_path).exists()
    );
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

// ── QA Compare: Before/After PNG comparison ───────────────────────

/// Load a PNG file and convert to RGBA bytes.
///
/// For now, we just validate that the file exists and is a PNG.
/// The actual PNG→RGBA decoding will happen in nightingale-core
/// via the comparison module if needed, or we can extend this later
/// with proper image decoding (using the `image` crate).
fn load_png(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let data = fs::read(path).ok()?;

    // Validate PNG signature
    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        eprintln!("[compare] PNG signature check failed: {}", path.display());
        return None;
    }

    // For QA Compare on macOS, use native Image I/O to decode PNG
    // This requires platform-specific code, so we'll return the raw PNG bytes
    // and let the caller handle decoding (or use a proper image crate).
    #[cfg(target_os = "macos")]
    {
        // Placeholder: return dummy dimensions for now
        // TODO: Implement PNG decoding using CoreImage or the `image` crate
        eprintln!(
            "[compare] PNG loading: {} (decode not yet impl)",
            path.display()
        );

        // For now, return empty to indicate loading works but decoding is stubbed
        // This allows the Flutter API to be built and will be fixed when we
        // add proper image decoding
        Some((vec![], 0, 0))
    }

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!(
            "[compare] PNG loading: {} (not implemented on this platform)",
            path.display()
        );
        Some((vec![], 0, 0))
    }
}

/// List all before/after QA compare fixtures in test-output/qa-compare/.
pub fn list_qa_compare_fixtures(project_root: String) -> Vec<QaCompareFixtureInfo> {
    eprintln!(
        "[compare] list_qa_compare_fixtures: project_root={}",
        project_root
    );

    let qa_compare_dir = format!("{}/test-output/qa-compare", project_root);
    let before_dir = format!("{}/before", qa_compare_dir);
    let after_dir = format!("{}/after", qa_compare_dir);

    let before_path = Path::new(&before_dir);
    let after_path = Path::new(&after_dir);

    if !before_path.exists() || !after_path.exists() {
        eprintln!("[compare]   QA compare dirs not found");
        return vec![];
    }

    let mut fixtures = Vec::new();

    // Scan before directory for PNG files
    if let Ok(entries) = fs::read_dir(before_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_stem() {
                if let Some(stem) = name.to_str() {
                    // Check if corresponding after PNG exists
                    let after_png = format!("{}/{}.png", after_dir, stem);
                    let has_pair = Path::new(&after_png).exists();

                    if has_pair {
                        fixtures.push(QaCompareFixtureInfo {
                            fixture_name: stem.to_string(),
                            has_pair: true,
                        });
                    }
                }
            }
        }
    }

    fixtures.sort_by(|a, b| a.fixture_name.cmp(&b.fixture_name));
    eprintln!("[compare]   found {} QA compare fixtures", fixtures.len());
    fixtures
}

/// Get a QA comparison for a before/after PNG pair.
pub fn get_qa_comparison(project_root: String, fixture_name: String) -> QaComparisonResult {
    eprintln!("[compare] get_qa_comparison: fixture={}", fixture_name);

    let qa_compare_dir = format!("{}/test-output/qa-compare", project_root);
    let before_path = format!("{}/before/{}.png", qa_compare_dir, fixture_name);
    let after_path = format!("{}/after/{}.png", qa_compare_dir, fixture_name);

    let empty = QaComparisonResult {
        before_rgba: vec![],
        before_width: 0,
        before_height: 0,
        after_rgba: vec![],
        after_width: 0,
        after_height: 0,
        diff_rgba: vec![],
        diff_width: 0,
        diff_height: 0,
        total_pixels: 0,
        diff_pixels: 0,
        diff_pct: 0.0,
    };

    // Load before PNG
    let (before_rgba, before_w, before_h) = match load_png(Path::new(&before_path)) {
        Some((data, w, h)) => {
            eprintln!("[compare]   before: {}x{}", w, h);
            (data, w, h)
        }
        None => {
            eprintln!("[compare]   FAIL: load before PNG: {}", before_path);
            return empty;
        }
    };

    // Load after PNG
    let (after_rgba, after_w, after_h) = match load_png(Path::new(&after_path)) {
        Some((data, w, h)) => {
            eprintln!("[compare]   after: {}x{}", w, h);
            (data, w, h)
        }
        None => {
            eprintln!("[compare]   FAIL: load after PNG: {}", after_path);
            return empty;
        }
    };

    // Compare using the same compare_rgba_images function
    let (diff_rgba, diff_w, diff_h, total, diff_px, pct) = compare_rgba_images(
        &before_rgba,
        before_w,
        before_h,
        &after_rgba,
        after_w,
        after_h,
    );

    eprintln!(
        "[compare]   diff: {:.2}% ({}/{} px) canvas {}x{}",
        pct, diff_px, total, diff_w, diff_h
    );

    QaComparisonResult {
        before_rgba,
        before_width: before_w,
        before_height: before_h,
        after_rgba,
        after_width: after_w,
        after_height: after_h,
        diff_rgba,
        diff_width: diff_w,
        diff_height: diff_h,
        total_pixels: total,
        diff_pixels: diff_px,
        diff_pct: pct,
    }
}
