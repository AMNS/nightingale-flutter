//! OG vs Modern rendering comparison tests.
//!
//! Compares our BitmapRenderer output against OG Nightingale reference PDFs
//! rendered via CoreGraphics (macOS native). Generates an HTML report with
//! side-by-side images and pixel diff metrics.
//!
//! Run:  cargo test --test og_comparison -- --nocapture
//!
//! Output goes to test-output/og-comparison/
//!   {name}_ours_page{N}.png     — our BitmapRenderer output
//!   {name}_og_page{N}.png       — OG reference (CoreGraphics render)
//!   {name}_diff_page{N}.png     — visual diff (matching=dimmed, different=red)
//!   report.html                  — interactive comparison report

mod common;

use image::RgbaImage;
use nightingale_core::comparison::{compare_rgba_images, OG_FIXTURES};
use nightingale_core::draw::draw_high_level::render_score;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::og_render;
use nightingale_core::render::{BitmapRenderer, MusicRenderer};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::Path;

const OUT_DIR: &str = "test-output/og-comparison";
const FIXTURE_DIR: &str = "tests/fixtures";
const OG_REF_DIR: &str = "tests/og_reference";
const FONT_DIR: &str = "assets/fonts";
const BRAVURA_PATH: &str = "assets/fonts/Bravura.otf";
const SONATA_PATH: &str = "assets/fonts/Sonata.ttf";

/// A single page comparison result.
#[allow(dead_code)]
struct PageResult {
    fixture_name: String,
    page_num: usize,
    ours_path: String,
    og_path: String,
    diff_path: String,
    total_pixels: u64,
    diff_pixels: u64,
    diff_pct: f64,
}

/// Render one of our NGL fixtures with BitmapRenderer, return page RGBA data.
fn render_our_fixture(fixture_name: &str) -> Vec<(Vec<u8>, u32, u32)> {
    let ngl_path = format!("{}/{}.ngl", FIXTURE_DIR, fixture_name);
    let data = fs::read(&ngl_path).unwrap_or_else(|_| panic!("read {}", ngl_path));
    let ngl =
        NglFile::read_from_bytes(&data).unwrap_or_else(|e| panic!("parse {}: {}", ngl_path, e));
    let score = interpret_heap(&ngl).unwrap_or_else(|e| panic!("interpret {}: {}", ngl_path, e));

    let mut renderer = BitmapRenderer::new(72.0);
    renderer.set_page_size(score.page_width_pt, score.page_height_pt);

    // Load Sonata font first (for OG comparison — glyphs match OG reference PDFs).
    // Falls back to Bravura/SMuFL if Sonata is not available.
    let sonata_path = Path::new(SONATA_PATH);
    if sonata_path.exists() {
        if let Ok(font_data) = fs::read(sonata_path) {
            renderer.load_sonata_font(font_data);
        }
    }
    // Load Bravura as fallback for any glyphs not in Sonata
    let font_path = Path::new(BRAVURA_PATH);
    if font_path.exists() {
        if let Ok(font_data) = fs::read(font_path) {
            renderer.load_music_font(font_data);
        }
    }
    // Load text fonts (Liberation Sans/Serif for lyrics, titles, etc.)
    let font_dir = Path::new(FONT_DIR);
    renderer.load_text_fonts_from_dir(font_dir);

    render_score(&score, &mut renderer);

    let mut pages = Vec::new();
    for p in 0..renderer.page_count() {
        let data = renderer.page_data(p).expect("page data");
        let (w, h) = renderer.page_dimensions(p).expect("page dims");
        pages.push((data.to_vec(), w, h));
    }
    pages
}

#[test]
fn compare_og_references() {
    let out_dir = Path::new(OUT_DIR);
    fs::create_dir_all(out_dir).unwrap();

    let mut results: Vec<PageResult> = Vec::new();
    let mut skipped = 0;

    for fixture in OG_FIXTURES {
        let og_pdf_path = format!("{}/{}", OG_REF_DIR, fixture.og_pdf);

        // Check OG PDF exists
        if !Path::new(&og_pdf_path).exists() {
            eprintln!(
                "[SKIP] {} — OG PDF not found: {}",
                fixture.fixture_name, og_pdf_path
            );
            skipped += 1;
            continue;
        }

        // Get OG page count
        let og_pages = match og_render::pdf_page_count(&og_pdf_path) {
            Some(n) => n,
            None => {
                eprintln!(
                    "[SKIP] {} — cannot read OG PDF: {}",
                    fixture.fixture_name, og_pdf_path
                );
                skipped += 1;
                continue;
            }
        };

        // Render our version
        let our_pages = render_our_fixture(fixture.fixture_name);
        let page_count = og_pages.min(our_pages.len());

        eprintln!(
            "[{}] OG pages={}, our pages={}, comparing {}",
            fixture.fixture_name,
            og_pages,
            our_pages.len(),
            page_count
        );

        for (page_idx, our_page) in our_pages.iter().enumerate().take(page_count) {
            let page_num = page_idx + 1; // 1-based
            let (ours_rgba, ours_w, ours_h) = (our_page.0.as_slice(), our_page.1, our_page.2);

            // Render OG page via CoreGraphics
            let og_rendered = match og_render::render_pdf_page(&og_pdf_path, page_num, 72.0) {
                Some(r) => r,
                None => {
                    eprintln!("  [SKIP] page {} — CoreGraphics render failed", page_num);
                    continue;
                }
            };

            // Compare
            let (diff_rgba, diff_w, diff_h, total, diff_px, pct) = compare_rgba_images(
                ours_rgba,
                ours_w,
                ours_h,
                &og_rendered.rgba,
                og_rendered.width,
                og_rendered.height,
            );

            // Save images
            let ours_filename = format!("{}_ours_page{}.png", fixture.fixture_name, page_num);
            let og_filename = format!("{}_og_page{}.png", fixture.fixture_name, page_num);
            let diff_filename = format!("{}_diff_page{}.png", fixture.fixture_name, page_num);

            let ours_path = out_dir.join(&ours_filename);
            let og_path = out_dir.join(&og_filename);
            let diff_path = out_dir.join(&diff_filename);

            RgbaImage::from_raw(ours_w, ours_h, ours_rgba.to_vec())
                .expect("ours image")
                .save(&ours_path)
                .expect("save ours");
            RgbaImage::from_raw(og_rendered.width, og_rendered.height, og_rendered.rgba)
                .expect("og image")
                .save(&og_path)
                .expect("save og");
            RgbaImage::from_raw(diff_w, diff_h, diff_rgba)
                .expect("diff image")
                .save(&diff_path)
                .expect("save diff");

            eprintln!(
                "  page {}: {}/{} pixels differ ({:.2}%) — ours {}x{} vs OG {}x{}",
                page_num,
                diff_px,
                total,
                pct,
                ours_w,
                ours_h,
                og_rendered.width,
                og_rendered.height
            );

            results.push(PageResult {
                fixture_name: fixture.fixture_name.to_string(),
                page_num,
                ours_path: fs::canonicalize(&ours_path)
                    .unwrap_or(ours_path.clone())
                    .to_string_lossy()
                    .to_string(),
                og_path: fs::canonicalize(&og_path)
                    .unwrap_or(og_path.clone())
                    .to_string_lossy()
                    .to_string(),
                diff_path: fs::canonicalize(&diff_path)
                    .unwrap_or(diff_path.clone())
                    .to_string_lossy()
                    .to_string(),
                total_pixels: total,
                diff_pixels: diff_px,
                diff_pct: pct,
            });
        }
    }

    // Generate HTML report
    let report_path = out_dir.join("report.html");
    generate_og_comparison_report(&report_path, &results, skipped).expect("generate HTML report");

    // Summary
    eprintln!("\n=== OG Comparison Summary ===");
    eprintln!("Fixtures compared: {}", OG_FIXTURES.len() - skipped);
    eprintln!("Skipped: {}", skipped);
    eprintln!("Pages compared: {}", results.len());
    if !results.is_empty() {
        let avg_diff: f64 = results.iter().map(|r| r.diff_pct).sum::<f64>() / results.len() as f64;
        eprintln!("Average diff: {:.2}%", avg_diff);
        let worst = results
            .iter()
            .max_by(|a, b| a.diff_pct.partial_cmp(&b.diff_pct).unwrap())
            .unwrap();
        eprintln!(
            "Worst: {} page {} ({:.2}%)",
            worst.fixture_name, worst.page_num, worst.diff_pct
        );
        let best = results
            .iter()
            .min_by(|a, b| a.diff_pct.partial_cmp(&b.diff_pct).unwrap())
            .unwrap();
        eprintln!(
            "Best:  {} page {} ({:.2}%)",
            best.fixture_name, best.page_num, best.diff_pct
        );
    }
    eprintln!("\nHTML report: file://{}", report_path.display());
}

/// Generate an HTML comparison report.
fn generate_og_comparison_report(
    report_path: &Path,
    results: &[PageResult],
    skipped: usize,
) -> Result<(), String> {
    let mut html = String::with_capacity(16384);

    write!(
        html,
        r##"<!DOCTYPE html>
<html lang="en"><head>
<meta charset="utf-8">
<title>OG vs Modern — Nightingale Rendering Comparison</title>
<style>
  * {{ box-sizing: border-box; }}
  body {{ font-family: system-ui, -apple-system, sans-serif; max-width: 1600px;
         margin: 0 auto; padding: 20px; background: #1a1a2e; color: #e0e0e0; }}
  h1 {{ border-bottom: 2px solid #4a4a6a; padding-bottom: 8px; color: #fff; }}
  .summary {{ background: #16213e; border: 1px solid #333; border-radius: 8px;
              padding: 16px; margin: 16px 0; display: flex; gap: 24px; flex-wrap: wrap; }}
  .summary .stat {{ text-align: center; min-width: 80px; }}
  .summary .stat .num {{ font-size: 28px; font-weight: bold; }}
  .summary .stat .label {{ font-size: 12px; color: #888; }}
  .stat.pages .num {{ color: #5dade2; }}
  .stat.avg .num {{ color: #f39c12; }}
  .stat.worst .num {{ color: #e74c3c; }}
  .stat.best .num {{ color: #2ecc71; }}
  .stat.skip .num {{ color: #888; }}

  .entry {{ background: #16213e; border: 1px solid #333; border-radius: 8px;
            margin: 20px 0; padding: 20px; }}
  .entry-header {{ display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }}
  .entry-header h2 {{ margin: 0; font-size: 18px; color: #fff; }}
  .entry-header .pct {{ font-size: 14px; font-weight: normal; padding: 2px 8px;
                        border-radius: 4px; }}
  .pct-good {{ background: #1a472a; color: #2ecc71; }}
  .pct-ok {{ background: #4a3f00; color: #f39c12; }}
  .pct-bad {{ background: #4a1a1a; color: #e74c3c; }}

  .images {{ display: flex; gap: 4px; margin: 12px 0; }}
  .images .col {{ flex: 1; text-align: center; overflow: hidden; }}
  .images .col img {{ width: 100%; border: 1px solid #333; cursor: pointer;
                      transition: transform 0.2s; background: #fff; }}
  .images .col img:hover {{ transform: scale(1.02); box-shadow: 0 4px 12px rgba(0,0,0,0.5); }}
  .images .col .label {{ font-size: 11px; color: #888; margin-bottom: 4px;
                         text-transform: uppercase; letter-spacing: 0.5px; }}

  .dim-info {{ font-size: 12px; color: #666; margin-top: 4px; }}

  /* View mode controls */
  .view-controls {{ display: flex; gap: 8px; margin-bottom: 12px; }}
  .view-controls button {{ background: #2a2a4a; color: #aaa; border: 1px solid #444;
                           padding: 6px 14px; border-radius: 4px; cursor: pointer;
                           font-size: 12px; }}
  .view-controls button.active {{ background: #3a3a6a; color: #fff; border-color: #5dade2; }}
  .view-controls button:hover {{ background: #3a3a5a; }}

  /* Overlay (blink) mode */
  .overlay-container {{ position: relative; display: none; }}
  .overlay-container img {{ width: 100%; background: #fff; }}
  .overlay-container .overlay-img {{ position: absolute; top: 0; left: 0;
                                     width: 100%; opacity: 0; transition: opacity 0.15s; }}
  .overlay-container.blink .overlay-img {{ animation: blink 1.5s ease-in-out infinite; }}
  @keyframes blink {{
    0%, 40% {{ opacity: 0; }}
    50%, 90% {{ opacity: 1; }}
    100% {{ opacity: 0; }}
  }}

  /* Slider reveal mode */
  .slider-container {{ position: relative; display: none; overflow: hidden; cursor: ew-resize; }}
  .slider-container img {{ width: 100%; display: block; background: #fff; }}
  .slider-container .slider-clip {{ position: absolute; top: 0; left: 0; width: 50%;
                                    height: 100%; overflow: hidden; }}
  .slider-container .slider-clip img {{ width: 200%; max-width: none; }}
  .slider-container .slider-line {{ position: absolute; top: 0; width: 2px; height: 100%;
                                    background: #5dade2; left: 50%; pointer-events: none; }}
  .slider-container .slider-labels {{ position: absolute; top: 8px; width: 100%;
                                      display: flex; justify-content: space-between;
                                      pointer-events: none; padding: 0 8px; }}
  .slider-container .slider-labels span {{ font-size: 11px; color: #fff;
                                           background: rgba(0,0,0,0.6);
                                           padding: 2px 6px; border-radius: 3px; }}

  .modal {{ display: none; position: fixed; top: 0; left: 0; width: 100%; height: 100%;
            background: rgba(0,0,0,0.92); z-index: 1000; cursor: pointer;
            justify-content: center; align-items: center; }}
  .modal img {{ max-width: 95%; max-height: 95%; object-fit: contain; image-rendering: pixelated; }}
  .modal.active {{ display: flex; }}
  .modal .label {{ position: fixed; top: 16px; left: 50%; transform: translateX(-50%);
                   color: #fff; font-size: 14px; background: rgba(0,0,0,0.7);
                   padding: 4px 12px; border-radius: 4px; }}

  /* Feedback panel */
  .feedback {{ margin-top: 12px; padding-top: 12px; border-top: 1px solid #333;
               display: flex; gap: 8px; align-items: center; }}
  .feedback button {{ background: #2a2a4a; color: #aaa; border: 1px solid #444;
                      padding: 4px 12px; border-radius: 4px; cursor: pointer;
                      font-size: 12px; }}
  .feedback button:hover {{ background: #3a3a5a; }}
  .feedback button.selected {{ border-color: #5dade2; color: #fff; }}
  .feedback .note {{ flex: 1; background: #0d1117; border: 1px solid #333;
                     color: #e0e0e0; padding: 4px 8px; border-radius: 4px;
                     font-size: 12px; }}
</style>
</head><body>
<h1>OG vs Modern — Nightingale Rendering Comparison</h1>
"##
    )
    .unwrap();

    // Summary
    let total_pages = results.len();
    let avg_diff = if total_pages > 0 {
        results.iter().map(|r| r.diff_pct).sum::<f64>() / total_pages as f64
    } else {
        0.0
    };
    let worst_pct = results.iter().map(|r| r.diff_pct).fold(0.0f64, f64::max);
    let best_pct = results.iter().map(|r| r.diff_pct).fold(100.0f64, f64::min);

    write!(
        html,
        r#"<div class="summary">
  <div class="stat pages"><div class="num">{}</div><div class="label">Pages Compared</div></div>
  <div class="stat avg"><div class="num">{:.1}%</div><div class="label">Avg Diff</div></div>
  <div class="stat best"><div class="num">{:.1}%</div><div class="label">Best Page</div></div>
  <div class="stat worst"><div class="num">{:.1}%</div><div class="label">Worst Page</div></div>
  <div class="stat skip"><div class="num">{}</div><div class="label">Skipped</div></div>
</div>
"#,
        total_pages, avg_diff, best_pct, worst_pct, skipped
    )
    .unwrap();

    // Entries
    for (i, r) in results.iter().enumerate() {
        let pct_class = if r.diff_pct < 5.0 {
            "pct-good"
        } else if r.diff_pct < 20.0 {
            "pct-ok"
        } else {
            "pct-bad"
        };

        write!(
            html,
            r##"<div class="entry" id="entry-{i}">
  <div class="entry-header">
    <h2>{name} — page {page}</h2>
    <span class="pct {pct_class}">{diff_px} px differ ({pct:.2}%)</span>
  </div>
  <div class="view-controls">
    <button class="active" onclick="setView({i},'sidebyside')">Side by Side</button>
    <button onclick="setView({i},'blink')">Blink</button>
    <button onclick="setView({i},'slider')">Slider</button>
  </div>
  <div class="images" id="sbs-{i}">
    <div class="col"><div class="label">Ours (Modern)</div><img src="file://{ours}" onclick="showModal(this,'Ours')" alt="ours"></div>
    <div class="col"><div class="label">Diff</div><img src="file://{diff}" onclick="showModal(this,'Diff')" alt="diff"></div>
    <div class="col"><div class="label">OG (Nightingale)</div><img src="file://{og}" onclick="showModal(this,'OG')" alt="og"></div>
  </div>
  <div class="overlay-container" id="blink-{i}">
    <img src="file://{ours}" alt="ours">
    <img class="overlay-img" src="file://{og}" alt="og">
  </div>
  <div class="slider-container" id="slider-{i}" onmousemove="handleSlider(event,{i})" ontouchmove="handleSliderTouch(event,{i})">
    <img src="file://{og}" alt="og">
    <div class="slider-clip"><img src="file://{ours}" alt="ours"></div>
    <div class="slider-line"></div>
    <div class="slider-labels"><span>Ours</span><span>OG</span></div>
  </div>
  <div class="feedback">
    <button onclick="setVerdict({i},'better')">Better</button>
    <button onclick="setVerdict({i},'same')">Same</button>
    <button onclick="setVerdict({i},'worse')">Worse</button>
    <button onclick="setVerdict({i},'bug')">Bug</button>
    <input class="note" type="text" placeholder="Notes..." id="note-{i}">
  </div>
</div>
"##,
            i = i,
            name = r.fixture_name,
            page = r.page_num,
            pct_class = pct_class,
            diff_px = r.diff_pixels,
            pct = r.diff_pct,
            ours = r.ours_path,
            og = r.og_path,
            diff = r.diff_path,
        )
        .unwrap();
    }

    // JavaScript
    write!(
        html,
        r##"
<div class="modal" id="modal" onclick="this.classList.remove('active')">
  <div class="label" id="modal-label"></div>
  <img id="modal-img" src="">
</div>
<script>
const verdicts = {{}};

function showModal(img, label) {{
  document.getElementById('modal-img').src = img.src;
  document.getElementById('modal-label').textContent = label;
  document.getElementById('modal').classList.add('active');
}}

function setView(idx, mode) {{
  const entry = document.getElementById('entry-' + idx);
  const sbs = document.getElementById('sbs-' + idx);
  const blink = document.getElementById('blink-' + idx);
  const slider = document.getElementById('slider-' + idx);

  sbs.style.display = mode === 'sidebyside' ? 'flex' : 'none';
  blink.style.display = mode === 'blink' ? 'block' : 'none';
  blink.classList.toggle('blink', mode === 'blink');
  slider.style.display = mode === 'slider' ? 'block' : 'none';

  // Update button states
  const btns = entry.querySelectorAll('.view-controls button');
  btns.forEach(b => b.classList.remove('active'));
  const labels = {{'sidebyside': 'Side by Side', 'blink': 'Blink', 'slider': 'Slider'}};
  btns.forEach(b => {{ if (b.textContent === labels[mode]) b.classList.add('active'); }});
}}

function handleSlider(e, idx) {{
  const container = document.getElementById('slider-' + idx);
  const rect = container.getBoundingClientRect();
  const x = (e.clientX - rect.left) / rect.width;
  const pct = Math.max(0, Math.min(1, x)) * 100;
  container.querySelector('.slider-clip').style.width = pct + '%';
  container.querySelector('.slider-line').style.left = pct + '%';
}}

function handleSliderTouch(e, idx) {{
  e.preventDefault();
  const touch = e.touches[0];
  handleSlider({{ clientX: touch.clientX }}, idx);
}}

function setVerdict(idx, verdict) {{
  verdicts[idx] = verdict;
  const entry = document.getElementById('entry-' + idx);
  const btns = entry.querySelectorAll('.feedback button');
  btns.forEach(b => {{
    b.classList.toggle('selected', b.textContent.toLowerCase() === verdict);
  }});
}}

// Export feedback as JSON
function exportFeedback() {{
  const feedback = [];
  for (const [idx, verdict] of Object.entries(verdicts)) {{
    const note = document.getElementById('note-' + idx)?.value || '';
    feedback.push({{ index: parseInt(idx), verdict, note }});
  }}
  const blob = new Blob([JSON.stringify(feedback, null, 2)], {{ type: 'application/json' }});
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = 'og_comparison_feedback.json'; a.click();
}}
</script>
</body></html>
"##
    )
    .unwrap();

    fs::write(report_path, &html).map_err(|e| format!("write HTML: {}", e))?;
    Ok(())
}
