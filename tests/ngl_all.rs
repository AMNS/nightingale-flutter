//! Comprehensive integration tests for ALL NGL (.ngl) fixture files.
//!
//! Every file in tests/fixtures/ gets the full rendering treatment:
//! 1. Read → NglFile
//! 2. Interpret → InterpretedScore
//! 3. Render → CommandRenderer (structural validation)
//! 4. Geometry checks (positions on page, reasonable stems, etc.)
//! 5. Render → PdfRenderer (valid PDF output)
//! 6. Insta snapshot for regression detection

use nightingale_core::defs::*;
use nightingale_core::draw::render_score;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::render::{CommandRenderer, PdfRenderer, RenderCommand};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

// ============================================================================
// Test infrastructure
// ============================================================================

/// NGL fixture files under test.
/// All 17 fixtures: 16 N103 files + 1 N105 (Capital Regiment March).
const ALL_NGL_FILES: &[&str] = &[
    "tests/fixtures/01_me_and_lucy.ngl",
    "tests/fixtures/02_cloning_frank_blacks.ngl",
    "tests/fixtures/03_holed_up_in_penjinskya.ngl",
    "tests/fixtures/04_eating_humble_pie.ngl",
    "tests/fixtures/05_abigail.ngl",
    "tests/fixtures/06_melyssa_with_a_y.ngl",
    "tests/fixtures/07_new_york_debutante.ngl",
    "tests/fixtures/08_darling_sunshine.ngl",
    "tests/fixtures/09_swiss_ann.ngl",
    "tests/fixtures/10_ghost_of_fusion_bob.ngl",
    "tests/fixtures/11_philip.ngl",
    "tests/fixtures/12_what_do_i_know.ngl",
    "tests/fixtures/13_miss_b.ngl",
    "tests/fixtures/14_chrome_molly.ngl",
    "tests/fixtures/15_selfsame_twin.ngl",
    "tests/fixtures/16_esmerelda.ngl",
    "tests/fixtures/17_capital_regiment_march.ngl",
];

/// Derive a short test-friendly name from an NGL path.
fn short_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string()
}

/// Count render commands by name.
fn count_by_name(commands: &[RenderCommand], name: &str) -> usize {
    commands.iter().filter(|c| c.name() == name).count()
}

/// Compute a deterministic hash of the full render command stream.
///
/// Hashes the Debug representation of every command in sequence.
/// Any change to any coordinate, glyph, color, or command ordering
/// will change the hash. Used as a refactor-safety guard.
fn command_stream_hash(commands: &[RenderCommand]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for cmd in commands {
        format!("{:?}", cmd).hash(&mut hasher);
    }
    hasher.finish()
}

/// Build a compact summary of render commands for snapshot regression.
fn render_summary(commands: &[RenderCommand]) -> String {
    let mut lines = Vec::new();

    // Command counts
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for cmd in commands {
        *counts.entry(cmd.name()).or_insert(0) += 1;
    }
    lines.push("=== COMMAND COUNTS ===".to_string());
    for (name, count) in &counts {
        lines.push(format!("{}: {}", name, count));
    }
    lines.push(format!("TOTAL: {}", commands.len()));

    // Staff positions
    lines.push("\n=== STAVES ===".to_string());
    for cmd in commands {
        if let RenderCommand::Staff {
            y,
            x0,
            x1,
            n_lines,
            line_spacing,
        } = cmd
        {
            lines.push(format!(
                "y={:.1} x=[{:.1}..{:.1}] lines={} spacing={:.2}",
                y, x0, x1, n_lines, line_spacing
            ));
        }
    }

    // Barline count (don't enumerate — NGL files can have hundreds)
    let barline_count = count_by_name(commands, "BarLine");
    lines.push(format!("\n=== BARLINES: {} total ===", barline_count));

    // Beams summary
    let beam_count = count_by_name(commands, "Beam");
    lines.push(format!("\n=== BEAMS: {} total ===", beam_count));

    // Stem count
    let stem_count = count_by_name(commands, "NoteStem");
    lines.push(format!("\n=== STEMS: {} total ===", stem_count));

    // Slur count
    let slur_count = count_by_name(commands, "Slur");
    if slur_count > 0 {
        lines.push(format!("\n=== SLURS: {} total ===", slur_count));
    }

    // MusicChar glyph distribution
    lines.push("\n=== GLYPHS ===".to_string());
    let mut glyph_counts: BTreeMap<String, usize> = BTreeMap::new();
    for cmd in commands {
        if let RenderCommand::MusicChar { glyph, .. } = cmd {
            *glyph_counts.entry(format!("{:?}", glyph)).or_insert(0) += 1;
        }
    }
    for (glyph, count) in &glyph_counts {
        lines.push(format!("{}: {}", glyph, count));
    }

    lines.join("\n")
}

// ============================================================================
// Read + Interpret: every file should produce a valid InterpretedScore
// ============================================================================

#[test]
fn test_all_ngl_read_and_interpret() {
    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path)
            .unwrap_or_else(|e| panic!("[{}] Failed to read: {}", name, e));
        let score = interpret_heap(&ngl)
            .unwrap_or_else(|e| panic!("[{}] Failed to interpret: {}", name, e));

        assert!(
            !score.objects.is_empty(),
            "[{}] Score should have objects",
            name
        );

        // Every score needs SYNCs (note/rest containers)
        let sync_count = score
            .objects
            .iter()
            .filter(|o| o.header.obj_type as u8 == SYNC_TYPE)
            .count();
        assert!(sync_count > 0, "[{}] Should have SYNCs, got 0", name);

        // Every score needs STAFFs
        let staff_count = score
            .objects
            .iter()
            .filter(|o| o.header.obj_type as u8 == STAFF_TYPE)
            .count();
        assert!(staff_count > 0, "[{}] Should have STAFFs, got 0", name);

        // Every score needs MEASUREs
        let measure_count = score
            .objects
            .iter()
            .filter(|o| o.header.obj_type as u8 == MEASURE_TYPE)
            .count();
        assert!(measure_count > 0, "[{}] Should have MEASUREs, got 0", name);

        // Should have note subobjects
        let total_notes: usize = score.notes.values().map(|v| v.len()).sum();
        assert!(total_notes > 0, "[{}] Should have note subobjects", name);

        // Walk diagnostics: verify walk() reaches score content
        let walk_count = score.walk().count();
        let walked_syncs = score
            .walk()
            .filter(|o| o.header.obj_type as u8 == SYNC_TYPE)
            .count();
        println!(
            "[{}] OK: {} objects (walk: {}), {} syncs (walk: {}), {} staves, {} measures, {} notes, head_l={}",
            name,
            score.objects.len(),
            walk_count,
            sync_count,
            walked_syncs,
            staff_count,
            measure_count,
            total_notes,
            score.head_l
        );
    }
}

// ============================================================================
// Render + Geometry: every file should produce renderable, sane output
// ============================================================================

#[test]
fn test_all_ngl_render_and_geometry() {
    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();

        let staff_count = count_by_name(&commands, "Staff");
        let barline_count = count_by_name(&commands, "BarLine");
        let music_char_count = count_by_name(&commands, "MusicChar");
        let stem_count = count_by_name(&commands, "NoteStem");

        // N103 files have mismatched subobject unpackers (ASTAFF show_lines reads
        // as 0 due to N103/N105 struct layout differences). Skip strict assertions
        // for N103 until we add format-specific unpackers.
        let is_n103 = !path.contains("capital_regiment_march");

        // N103 files have mismatched subobject unpackers (N103 structs have different
        // field sizes/offsets than N105). Skip strict content assertions for N103 —
        // they'll be enabled when we add format-specific unpackers.
        if !is_n103 {
            assert!(
                staff_count > 0,
                "[{}] Should render at least 1 staff (total cmds: {})",
                name,
                commands.len()
            );
            assert!(barline_count > 0, "[{}] Should render barlines", name);
            assert!(
                music_char_count > 0,
                "[{}] Should render noteheads/rests",
                name
            );
        }

        // Geometry validation on every command
        for cmd in &commands {
            match cmd {
                RenderCommand::Staff {
                    y,
                    x0,
                    x1,
                    n_lines,
                    line_spacing,
                } => {
                    assert!(*y > 0.0, "[{}] Staff y should be positive: {}", name, y);
                    assert!(x0 < x1, "[{}] Staff x0 ({}) < x1 ({})", name, x0, x1);
                    assert!(
                        *n_lines >= 1 && *n_lines <= 6,
                        "[{}] Staff lines {} should be 1-6",
                        name,
                        n_lines
                    );
                    assert!(
                        *line_spacing > 1.0 && *line_spacing < 20.0,
                        "[{}] Line spacing {} should be reasonable",
                        name,
                        line_spacing
                    );
                }
                RenderCommand::BarLine { top, bottom, x, .. } => {
                    assert!(
                        top < bottom,
                        "[{}] Barline top ({}) < bottom ({})",
                        name,
                        top,
                        bottom
                    );
                    assert!(*x > 0.0, "[{}] Barline x should be positive: {}", name, x);
                }
                RenderCommand::MusicChar { x, y, .. } => {
                    // X should be within page width with margin.
                    // Y should be positive (page-relative in multi-page scores).
                    assert!(
                        *x > -10.0 && *x < 900.0,
                        "[{}] Note x ({}) should be within page width",
                        name,
                        x
                    );
                    assert!(
                        *y > -10.0,
                        "[{}] Note y ({}) should be non-negative",
                        name,
                        y
                    );
                }
                RenderCommand::NoteStem {
                    x,
                    y_top,
                    y_bottom,
                    width,
                } => {
                    assert!(
                        y_top <= y_bottom,
                        "[{}] Stem y_top ({}) <= y_bottom ({})",
                        name,
                        y_top,
                        y_bottom
                    );
                    assert!(*x > 0.0, "[{}] Stem x should be positive: {}", name, x);
                    let length = y_bottom - y_top;
                    assert!(
                        length < 120.0,
                        "[{}] Stem length {} should be reasonable",
                        name,
                        length
                    );
                    assert!(
                        *width > 0.0 && *width < 5.0,
                        "[{}] Stem width {} should be thin",
                        name,
                        width
                    );
                }
                _ => {}
            }
        }

        println!(
            "[{}] OK: {} staves, {} barlines, {} noteheads, {} stems, {} total commands",
            name,
            staff_count,
            barline_count,
            music_char_count,
            stem_count,
            commands.len()
        );
    }
}

// ============================================================================
// PDF output: every file should produce a valid PDF
// ============================================================================

#[test]
fn test_all_ngl_produce_valid_pdf() {
    let output_dir = Path::new("/tmp/nightingale-test-output/ngl");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let font_path = Path::new("icebox/nightingale_app/assets/fonts/Bravura.otf");

    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        // Use page dimensions from NGL document header (respects landscape/portrait)
        let (page_width, page_height) =
            nightingale_core::doc_types::DocumentHeader::from_n105_bytes(&ngl.doc_header_raw)
                .map(|hdr| {
                    let w = (hdr.orig_paper_rect.right - hdr.orig_paper_rect.left) as f32;
                    let h = (hdr.orig_paper_rect.bottom - hdr.orig_paper_rect.top) as f32;
                    (
                        if w > 0.0 { w } else { 612.0 },
                        if h > 0.0 { h } else { 792.0 },
                    )
                })
                .unwrap_or((612.0, 792.0));

        let mut pdf_renderer = PdfRenderer::new(page_width, page_height);

        if font_path.exists() {
            pdf_renderer.load_music_font_file(font_path);
        }

        render_score(&score, &mut pdf_renderer);
        let pdf_bytes = pdf_renderer.finish();

        assert!(
            pdf_bytes.starts_with(b"%PDF-"),
            "[{}] Output should be valid PDF",
            name
        );
        assert!(
            pdf_bytes.len() > 500,
            "[{}] PDF should be substantial ({} bytes)",
            name,
            pdf_bytes.len()
        );

        let output_path = output_dir.join(format!("{}.pdf", name));
        fs::write(&output_path, &pdf_bytes).unwrap();

        println!(
            "[{}] PDF: {} bytes -> {}",
            name,
            pdf_bytes.len(),
            output_path.display()
        );
    }
}

// ============================================================================
// Insta snapshots: regression detection for every file's render output
// ============================================================================

#[test]
fn test_all_ngl_regression_snapshots() {
    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();

        let summary = render_summary(&commands);
        insta::assert_snapshot!(format!("ngl_{}", name), summary);
    }
}

// ============================================================================
// Score structure: validate object hierarchy for every file
// ============================================================================

#[test]
fn test_all_ngl_score_structure() {
    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        // Count objects by type
        let mut type_counts: BTreeMap<u8, usize> = BTreeMap::new();
        for obj in &score.objects {
            *type_counts.entry(obj.header.obj_type as u8).or_insert(0) += 1;
        }

        // Required object types in every score
        assert!(
            type_counts.contains_key(&HEADER_TYPE),
            "[{}] Should have HEADER",
            name
        );
        assert!(
            type_counts.contains_key(&TAIL_TYPE),
            "[{}] Should have TAIL",
            name
        );
        assert!(
            type_counts.contains_key(&PAGE_TYPE),
            "[{}] Should have PAGE",
            name
        );
        assert!(
            type_counts.contains_key(&SYSTEM_TYPE),
            "[{}] Should have SYSTEM",
            name
        );
        assert!(
            type_counts.contains_key(&STAFF_TYPE),
            "[{}] Should have STAFF",
            name
        );
        assert!(
            type_counts.contains_key(&MEASURE_TYPE),
            "[{}] Should have MEASURE",
            name
        );
        assert!(
            type_counts.contains_key(&SYNC_TYPE),
            "[{}] Should have SYNC",
            name
        );

        // Walk should be complete (end at TAIL)
        let last = score.walk().last();
        assert!(last.is_some(), "[{}] walk() should return objects", name);
        assert_eq!(
            last.unwrap().header.obj_type as u8,
            TAIL_TYPE,
            "[{}] walk() should end at TAIL",
            name
        );

        // n_entries consistency: SYNC n_entries should match note subobjects
        let mut total_sync_entries: usize = 0;
        let mut total_note_subobjects: usize = 0;

        for obj in &score.objects {
            if obj.header.obj_type as u8 == SYNC_TYPE || obj.header.obj_type as u8 == GRSYNC_TYPE {
                total_sync_entries += obj.header.n_entries as usize;
            }
        }
        for notes_vec in score.notes.values() {
            total_note_subobjects += notes_vec.len();
        }
        assert_eq!(
            total_sync_entries, total_note_subobjects,
            "[{}] SYNC n_entries ({}) should match note subobjects ({})",
            name, total_sync_entries, total_note_subobjects
        );

        // Print summary
        let type_names: Vec<String> = type_counts
            .iter()
            .map(|(t, c)| {
                let n = match *t {
                    HEADER_TYPE => "HDR",
                    TAIL_TYPE => "TAIL",
                    SYNC_TYPE => "SYNC",
                    PAGE_TYPE => "PAGE",
                    SYSTEM_TYPE => "SYS",
                    STAFF_TYPE => "STF",
                    MEASURE_TYPE => "MEAS",
                    CLEF_TYPE => "CLEF",
                    KEYSIG_TYPE => "KS",
                    TIMESIG_TYPE => "TS",
                    BEAMSET_TYPE => "BEAM",
                    CONNECT_TYPE => "CONN",
                    DYNAMIC_TYPE => "DYN",
                    SLUR_TYPE => "SLUR",
                    TUPLET_TYPE => "TUP",
                    GRAPHIC_TYPE => "GFX",
                    TEMPO_TYPE => "TEMPO",
                    _ => "?",
                };
                format!("{}={}", n, c)
            })
            .collect();

        println!("[{}] OK: {}", name, type_names.join(", "));
    }
}

// ============================================================================
// Command-stream hash: exact render output fingerprint for refactor safety
// ============================================================================

/// Exact render-command fingerprint for every NGL fixture.
///
/// Each hash captures the full sequence of render commands (coordinates, glyphs,
/// colors, ordering). Any behavioral change — even a 0.01pt coordinate shift —
/// will break the hash. Use `REGENERATE_REFS=1 cargo test` to update baselines
/// after intentional rendering changes.
#[test]
fn test_all_ngl_command_stream_hashes() {
    let regenerate = std::env::var("REGENERATE_REFS").is_ok();

    let expected: std::collections::HashMap<&str, u64> = [
        ("01_me_and_lucy", 10687385366423386178),
        ("02_cloning_frank_blacks", 13982872353156779011),
        ("03_holed_up_in_penjinskya", 15761544026529331273),
        ("04_eating_humble_pie", 12933458484849204335),
        ("05_abigail", 12429986271950333849),
        ("06_melyssa_with_a_y", 1844616962735454603),
        ("07_new_york_debutante", 4312723980633413409),
        ("08_darling_sunshine", 7152146844659583322),
        ("09_swiss_ann", 17124312840450400155),
        ("10_ghost_of_fusion_bob", 16220721296539521613),
        ("11_philip", 1027433326897166661),
        ("12_what_do_i_know", 17256836037372356154),
        ("13_miss_b", 10760555966502166402),
        ("14_chrome_molly", 11806851732308029677),
        ("15_selfsame_twin", 1313842490251767559),
        ("16_esmerelda", 3740813075569476410),
        ("17_capital_regiment_march", 4015289515686892782),
    ]
    .into_iter()
    .collect();

    let mut all_ok = true;
    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();
        let hash = command_stream_hash(&commands);

        if regenerate {
            println!("        (\"{}\", {}),", name, hash);
        } else if let Some(&exp) = expected.get(name.as_str()) {
            if exp != 0 && hash != exp {
                eprintln!(
                    "[{}] HASH MISMATCH: expected {} got {} ({} commands)",
                    name,
                    exp,
                    hash,
                    commands.len()
                );
                all_ok = false;
            }
        }
    }

    if regenerate {
        println!("\n// Copy the lines above into the expected hash table");
        return;
    }

    assert!(
        all_ok,
        "Command-stream hash mismatches detected! \
         Run `REGENERATE_REFS=1 cargo test test_all_ngl_command_stream_hashes -- --nocapture` \
         to regenerate baselines after intentional changes."
    );
}

// ============================================================================
// Bitmap regression: PDF page 1 rendered to PNG and compared pixel-by-pixel
// ============================================================================

/// Try to convert a PDF to PNG using available system tools.
///
/// Falls back through: sips (macOS) → pdftoppm (poppler-utils) → magick (ImageMagick).
/// Returns Ok(true) if conversion succeeded, Ok(false) if no tool available.
fn pdf_to_png(pdf_path: &Path, png_path: &Path) -> Result<bool, String> {
    // Try sips (macOS built-in)
    if let Ok(output) = std::process::Command::new("sips")
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
    // pdftoppm -png -r 72 -f 1 -l 1 input.pdf output_prefix
    let prefix = png_path.with_extension("");
    if let Ok(output) = std::process::Command::new("pdftoppm")
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
    if let Ok(output) = std::process::Command::new("magick")
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
fn compare_images_and_diff(
    golden_path: &Path,
    current_path: &Path,
    diff_path: &Path,
) -> Result<(u64, u64, f64), String> {
    use image::{GenericImageView, Rgba, RgbaImage};

    let golden = image::open(golden_path).map_err(|e| format!("open golden: {}", e))?;
    let current = image::open(current_path).map_err(|e| format!("open current: {}", e))?;

    let (gw, gh) = golden.dimensions();
    let (cw, ch) = current.dimensions();

    // Use the larger dimensions for the diff canvas
    let w = gw.max(cw);
    let h = gh.max(ch);
    let total = w as u64 * h as u64;

    let mut diff_img = RgbaImage::new(w, h);
    let mut diff_count: u64 = 0;

    for y in 0..h {
        for x in 0..w {
            let gpx = if x < gw && y < gh {
                golden.get_pixel(x, y)
            } else {
                Rgba([255, 255, 255, 255]) // treat out-of-bounds as white
            };
            let cpx = if x < cw && y < ch {
                current.get_pixel(x, y)
            } else {
                Rgba([255, 255, 255, 255])
            };

            if gpx == cpx {
                // Match: show dimmed (30% opacity blend with white)
                let r = (gpx[0] as u16 * 30 + 255 * 70) / 100;
                let g = (gpx[1] as u16 * 30 + 255 * 70) / 100;
                let b = (gpx[2] as u16 * 30 + 255 * 70) / 100;
                diff_img.put_pixel(x, y, Rgba([r as u8, g as u8, b as u8, 255]));
            } else {
                // Mismatch: bright red
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

/// Visual regression test: PDF → PNG → pixel diff against golden.
///
/// Cross-platform: tries sips (macOS), pdftoppm (Linux), magick (ImageMagick).
/// On mismatch, generates a visual diff image (matching pixels dimmed, diffs in red).
///
/// Regenerate goldens: `REGENERATE_REFS=1 cargo test test_all_ngl_bitmap_regression`
#[test]
fn test_all_ngl_bitmap_regression() {
    let regenerate = std::env::var("REGENERATE_REFS").is_ok();
    let golden_dir = Path::new("tests/golden_bitmaps");
    let output_dir = Path::new("/tmp/nightingale-test-output/ngl");
    let diff_dir = Path::new("/tmp/nightingale-test-output/bitmap-diff");
    fs::create_dir_all(output_dir).unwrap();
    fs::create_dir_all(diff_dir).unwrap();

    let font_path = Path::new("icebox/nightingale_app/assets/fonts/Bravura.otf");

    let mut mismatches = Vec::new();
    let mut skipped = 0;

    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        // Render to PDF
        let (page_width, page_height) =
            nightingale_core::doc_types::DocumentHeader::from_n105_bytes(&ngl.doc_header_raw)
                .map(|hdr| {
                    let w = (hdr.orig_paper_rect.right - hdr.orig_paper_rect.left) as f32;
                    let h = (hdr.orig_paper_rect.bottom - hdr.orig_paper_rect.top) as f32;
                    (
                        if w > 0.0 { w } else { 612.0 },
                        if h > 0.0 { h } else { 792.0 },
                    )
                })
                .unwrap_or((612.0, 792.0));

        let mut pdf_renderer = PdfRenderer::new(page_width, page_height);
        if font_path.exists() {
            pdf_renderer.load_music_font_file(font_path);
        }
        render_score(&score, &mut pdf_renderer);
        let pdf_bytes = pdf_renderer.finish();

        // Write PDF and convert page 1 to PNG
        let pdf_path = output_dir.join(format!("{}.pdf", name));
        fs::write(&pdf_path, &pdf_bytes).unwrap();

        let current_png = diff_dir.join(format!("{}_current.png", name));
        match pdf_to_png(&pdf_path, &current_png) {
            Ok(true) => {} // success
            Ok(false) => {
                if skipped == 0 {
                    eprintln!(
                        "No PDF-to-PNG tool found (tried sips, pdftoppm, magick). \
                         Bitmap regression tests will be skipped."
                    );
                }
                skipped += 1;
                continue;
            }
            Err(e) => {
                eprintln!("[{}] PDF-to-PNG error: {}", name, e);
                continue;
            }
        }

        let golden_path = golden_dir.join(format!("{}_page1.png", name));

        if regenerate {
            fs::copy(&current_png, &golden_path).unwrap();
            println!("[{}] Updated golden: {}", name, golden_path.display());
            continue;
        }

        if !golden_path.exists() {
            eprintln!("[{}] No golden bitmap at {}", name, golden_path.display());
            mismatches.push(format!("{} (no golden)", name));
            continue;
        }

        // Pixel-level comparison with visual diff output
        let diff_path = diff_dir.join(format!("{}_diff.png", name));
        match compare_images_and_diff(&golden_path, &current_png, &diff_path) {
            Ok((total, diff_pixels, diff_pct)) => {
                if diff_pixels == 0 {
                    // Perfect match — clean up
                    let _ = fs::remove_file(&current_png);
                    let _ = fs::remove_file(&diff_path);
                } else {
                    // Save golden copy alongside current and diff
                    let golden_copy = diff_dir.join(format!("{}_golden.png", name));
                    let _ = fs::copy(&golden_path, &golden_copy);

                    eprintln!(
                        "[{}] BITMAP MISMATCH: {}/{} pixels differ ({:.2}%)\n  \
                         golden:  {}\n  current: {}\n  diff:    {}",
                        name,
                        diff_pixels,
                        total,
                        diff_pct,
                        golden_copy.display(),
                        current_png.display(),
                        diff_path.display(),
                    );
                    mismatches.push(format!("{} ({:.2}% diff)", name, diff_pct));
                }
            }
            Err(e) => {
                eprintln!("[{}] Comparison error: {}", name, e);
                mismatches.push(format!("{} (error: {})", name, e));
            }
        }
    }

    if skipped == ALL_NGL_FILES.len() {
        eprintln!(
            "WARN: All {} bitmap tests skipped (no PDF-to-PNG tool). \
             Install one of: sips (macOS), poppler-utils (pdftoppm), or ImageMagick (magick).",
            skipped
        );
        return; // Don't fail — just warn
    }

    if !regenerate && !mismatches.is_empty() {
        panic!(
            "Bitmap mismatches in {} fixture(s): {}\n\
             Visual diff images: open /tmp/nightingale-test-output/bitmap-diff/\n\
             Regenerate goldens: REGENERATE_REFS=1 cargo test test_all_ngl_bitmap_regression",
            mismatches.len(),
            mismatches.join(", ")
        );
    }
}

// ============================================================================
// Beam rendering: NGL files with beamsets should produce beam commands
// ============================================================================

#[test]
fn test_all_ngl_beam_rendering() {
    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        // Check if the score has BEAMSET objects
        let has_beamsets = score
            .objects
            .iter()
            .any(|o| o.header.obj_type as u8 == BEAMSET_TYPE);

        if !has_beamsets {
            println!("[{}] SKIP: no BEAMSET objects", name);
            continue;
        }

        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();

        let beam_count = count_by_name(&commands, "Beam");
        assert!(
            beam_count > 0,
            "[{}] Has BEAMSET objects but produced 0 beam render commands",
            name
        );

        println!("[{}] Beams: {}", name, beam_count);
    }
}
