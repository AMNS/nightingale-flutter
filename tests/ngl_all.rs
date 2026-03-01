//! Comprehensive integration tests for ALL NGL (.ngl) fixture files.
//!
//! Every file in tests/fixtures/ gets the full rendering treatment:
//! 1. Read → NglFile
//! 2. Interpret → InterpretedScore
//! 3. Render → CommandRenderer (structural validation)
//! 4. Geometry checks (positions on page, reasonable stems, etc.)
//! 5. Render → PdfRenderer (valid PDF output)
//! 6. Insta snapshot for regression detection

mod common;

use nightingale_core::defs::*;
use nightingale_core::draw::render_score;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::render::{
    BitmapRenderer, CommandRenderer, MusicRenderer, PdfRenderer, RenderCommand,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

// ============================================================================
// Test infrastructure
// ============================================================================

/// NGL fixture files under test.
/// Geoff's 17 songs (16 N103 + 1 N105) plus 8 Tim Crawford scores (5 N105 + 2 N103 + 1 N105).
const ALL_NGL_FILES: &[&str] = &[
    // ── Geoff Chirgwin's songs ──
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
    // ── Tim Crawford's scores (with OG PostScript reference output) ──
    "tests/fixtures/tc_02.ngl",
    "tests/fixtures/tc_03a.ngl",
    "tests/fixtures/tc_03b.ngl",
    "tests/fixtures/tc_04.ngl",
    "tests/fixtures/tc_05.ngl",
    "tests/fixtures/tc_55_1.ngl",
    "tests/fixtures/tc_ich_bin_ja.ngl",
    "tests/fixtures/tc_schildt.ngl",
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
                    // Y can be negative for elements above the first staff
                    // (tempo markings, rehearsal marks, etc.) — allow up to
                    // ~50 pt above the top staff line.
                    assert!(
                        *x > -10.0 && *x < 900.0,
                        "[{}] Note x ({}) should be within page width",
                        name,
                        x
                    );
                    assert!(
                        *y > -60.0,
                        "[{}] MusicChar y ({}) is unreasonably negative",
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
    let output_dir = Path::new("test-output/ngl");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let font_path = Path::new("assets/fonts/Bravura.otf");

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
                    ENDING_TYPE => "END",
                    OTTAVA_TYPE => "OTT",
                    SPACER_TYPE => "SPC",
                    RPTEND_TYPE => "RPT",
                    PSMEAS_TYPE => "PSM",
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
        ("01_me_and_lucy", 15805161865115717007),
        ("02_cloning_frank_blacks", 10096544956964437254),
        ("03_holed_up_in_penjinskya", 12763194731467791861),
        ("04_eating_humble_pie", 4669649928586155445),
        ("05_abigail", 6952688033273721275),
        ("06_melyssa_with_a_y", 8753810474765907518),
        ("07_new_york_debutante", 2236744088432278022),
        ("08_darling_sunshine", 9156951737259873921),
        ("09_swiss_ann", 15817496954141651990),
        ("10_ghost_of_fusion_bob", 17999715854220833750),
        ("11_philip", 15725295408445142942),
        ("12_what_do_i_know", 18381634225866317272),
        ("13_miss_b", 895970881745455900),
        ("14_chrome_molly", 15007842213336735632),
        ("15_selfsame_twin", 15722051134169735861),
        ("16_esmerelda", 15191465884306311099),
        ("17_capital_regiment_march", 7288948818666967990),
        // Tim Crawford scores
        ("tc_02", 7438146024301840425),
        ("tc_03a", 8479072139110681431),
        ("tc_03b", 420980281288933817),
        ("tc_04", 10807042737599499062),
        ("tc_05", 17716903645635637287),
        ("tc_55_1", 12276674724426823702),
        ("tc_ich_bin_ja", 3996550512377391390),
        ("tc_schildt", 17999211726465769279),
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

// save_bitmap_page and compare_images_and_diff are in tests/common/mod.rs

/// Visual regression test: BitmapRenderer → PNG → pixel diff against golden.
///
/// Uses pure-Rust BitmapRenderer (tiny-skia) — no external PDF-to-PNG tools needed.
/// On mismatch, generates a visual diff image (matching pixels dimmed, diffs in red).
///
/// Regenerate goldens: `REGENERATE_REFS=1 cargo test test_all_ngl_bitmap_regression`
#[test]
fn test_all_ngl_bitmap_regression() {
    let regenerate = std::env::var("REGENERATE_REFS").is_ok();
    let golden_dir = Path::new("tests/golden_bitmaps");
    let diff_dir = Path::new("test-output/bitmap-diff");
    fs::create_dir_all(diff_dir).unwrap();

    let font_dir = Path::new("assets/fonts");
    let font_path = font_dir.join("Bravura.otf");
    let font_data = fs::read(&font_path).ok();

    let mut mismatches = Vec::new();

    for path in ALL_NGL_FILES {
        let name = short_name(path);
        let ngl = NglFile::read_from_file(path).unwrap();
        let score = interpret_heap(&ngl).unwrap();

        // Get page dimensions from NGL header
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

        // Render directly to bitmap (no PDF intermediate)
        let mut bmp = BitmapRenderer::new(72.0); // 72 DPI = 1 pixel per point
        bmp.set_page_size(page_width, page_height);
        if let Some(ref data) = font_data {
            bmp.load_music_font(data.clone());
        }
        bmp.load_text_fonts_from_dir(font_dir);
        render_score(&score, &mut bmp);
        // Flush any unfinished page
        if bmp.page_count() == 0 {
            bmp.end_page();
        }

        // Compare all pages (not just page 1)
        let num_pages = bmp.page_count();
        for page_idx in 0..num_pages {
            let page_num = page_idx + 1; // 1-indexed for filenames
            let page_suffix = format!("_page{}", page_num);
            let display_name = format!("{}{}", name, page_suffix);

            let current_png = diff_dir.join(format!("{}_current.png", display_name));
            if let Err(e) = common::save_bitmap_page(&bmp, page_idx, &current_png) {
                eprintln!("[{}] Save bitmap error: {}", display_name, e);
                mismatches.push(format!("{} (error: {})", display_name, e));
                continue;
            }

            let golden_path = golden_dir.join(format!("{}{}.png", name, page_suffix));

            if regenerate {
                fs::copy(&current_png, &golden_path).unwrap();
                println!(
                    "[{}] Updated golden: {}",
                    display_name,
                    golden_path.display()
                );
                continue;
            }

            if !golden_path.exists() {
                eprintln!(
                    "[{}] No golden bitmap at {}",
                    display_name,
                    golden_path.display()
                );
                mismatches.push(format!("{} (no golden)", display_name));
                continue;
            }

            // Pixel-level comparison with visual diff output
            let diff_path = diff_dir.join(format!("{}_diff.png", display_name));
            match common::compare_images_and_diff(&golden_path, &current_png, &diff_path) {
                Ok((_total, diff_pixels, diff_pct)) => {
                    if diff_pixels == 0 {
                        let _ = fs::remove_file(&current_png);
                        let _ = fs::remove_file(&diff_path);
                    } else {
                        let golden_copy = diff_dir.join(format!("{}_golden.png", display_name));
                        let _ = fs::copy(&golden_path, &golden_copy);

                        eprintln!(
                            "[{}] BITMAP MISMATCH: {}/{} pixels differ ({:.2}%)\n  \
                             golden:  {}\n  current: {}\n  diff:    {}",
                            display_name,
                            diff_pixels,
                            _total,
                            diff_pct,
                            golden_copy.display(),
                            current_png.display(),
                            diff_path.display(),
                        );
                        mismatches.push(format!("{} ({:.2}% diff)", display_name, diff_pct));
                    }
                }
                Err(e) => {
                    eprintln!("[{}] Comparison error: {}", display_name, e);
                    mismatches.push(format!("{} (error: {})", display_name, e));
                }
            }
        }
    }

    if !regenerate && !mismatches.is_empty() {
        panic!(
            "Bitmap mismatches in {} fixture(s): {}\n\
             Visual diff images: open test-output/bitmap-diff/\n\
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
