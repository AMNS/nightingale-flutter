//! Comprehensive integration tests for ALL Notelist (.nl) files.
//!
//! Every file in tests/notelist_examples/ gets the same treatment:
//! 1. Parse → Notelist
//! 2. Convert → InterpretedScore
//! 3. Render → CommandRenderer (structural validation)
//! 4. Geometry checks (positions on page, reasonable stems, etc.)
//! 5. Render → PdfRenderer (valid PDF output)
//! 6. Insta snapshot for regression detection

mod common;

use nightingale_core::draw::render_score;
use nightingale_core::notelist::{
    notelist_to_score, notelist_to_score_with_config, parse_notelist, NotelistLayoutConfig,
};
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

/// All 41 notelist fixtures, in alphabetical order.
const ALL_NOTELISTS: &[&str] = &[
    "tests/notelist_examples/accidentals.nl",
    "tests/notelist_examples/BachEbSonata_20.2sizes.nl",
    "tests/notelist_examples/BachEbSonata_20.nl",
    "tests/notelist_examples/BachStAnne_63.nl",
    "tests/notelist_examples/barline_types.nl",
    "tests/notelist_examples/bass_clef_melody.nl",
    "tests/notelist_examples/beamed_eighths.nl",
    "tests/notelist_examples/BinchoisDePlus-17.nl",
    "tests/notelist_examples/chord_seconds.nl",
    "tests/notelist_examples/chromatic_scale.nl",
    "tests/notelist_examples/clef_change.nl",
    "tests/notelist_examples/compound_meter.nl",
    "tests/notelist_examples/Debussy.Images_9.nl",
    "tests/notelist_examples/dotted_notes.nl",
    "tests/notelist_examples/GoodbyePorkPieHat.nl",
    "tests/notelist_examples/grace_notes_test.nl",
    "tests/notelist_examples/HBD_33.nl",
    "tests/notelist_examples/keysig_d_major.nl",
    "tests/notelist_examples/keysig_eb_major.nl",
    "tests/notelist_examples/keysig_flats_all.nl",
    "tests/notelist_examples/keysig_sharps_all.nl",
    "tests/notelist_examples/KillingMe_36.nl",
    "tests/notelist_examples/ledger_lines.nl",
    "tests/notelist_examples/MahlerLiedVonDE_25.nl",
    "tests/notelist_examples/MendelssohnOp7N1_2.nl",
    "tests/notelist_examples/mixed_durations.nl",
    "tests/notelist_examples/RavelScarbo_15.nl",
    "tests/notelist_examples/rests_all_durations.nl",
    "tests/notelist_examples/SchenkerDiagram_Chopin_6.nl",
    "tests/notelist_examples/SchoenbergOp19N1-21.nl",
    "tests/notelist_examples/sixteenths_32nds.nl",
    "tests/notelist_examples/TestMIDIChannels_3.nl",
    "tests/notelist_examples/text_annotations.nl",
    "tests/notelist_examples/tied_notes.nl",
    "tests/notelist_examples/time_sig_changes.nl",
    "tests/notelist_examples/tuplet_quintuplet.nl",
    "tests/notelist_examples/tuplet_triplet.nl",
    "tests/notelist_examples/two_voices.nl",
    "tests/notelist_examples/Webern.Op5N3_22.nl",
    "tests/notelist_examples/whole_notes.nl",
    "tests/notelist_examples/wide_intervals.nl",
];

/// Derive a short test-friendly name from a notelist path.
fn short_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .replace(['.', '-'], "_")
}

/// Count render commands by name.
fn count_by_name(commands: &[RenderCommand], name: &str) -> usize {
    commands.iter().filter(|c| c.name() == name).count()
}

/// Compute a deterministic hash of the full render command stream.
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

    // Barlines
    lines.push("\n=== BARLINES ===".to_string());
    for cmd in commands {
        if let RenderCommand::BarLine { top, bottom, x, .. } = cmd {
            lines.push(format!("x={:.1} top={:.1} bottom={:.1}", x, top, bottom));
        }
    }

    // Beams (slope info)
    lines.push("\n=== BEAMS ===".to_string());
    for cmd in commands {
        if let RenderCommand::Beam {
            x0,
            y0,
            x1,
            y1,
            thickness,
            ..
        } = cmd
        {
            let slope = if (x1 - x0).abs() > 0.1 {
                (y1 - y0) / (x1 - x0)
            } else {
                0.0
            };
            lines.push(format!(
                "x=[{:.1}..{:.1}] y=[{:.1}..{:.1}] slope={:.3} thick={:.2}",
                x0, x1, y0, y1, slope, thickness
            ));
        }
    }

    // Stem count
    let stem_count = commands.iter().filter(|c| c.name() == "NoteStem").count();
    lines.push(format!("\n=== STEMS: {} total ===", stem_count));

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
// Parse + Convert: every file should produce a valid InterpretedScore
// ============================================================================

#[test]
fn test_all_notelists_parse_and_convert() {
    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file =
            fs::File::open(path).unwrap_or_else(|e| panic!("[{}] Failed to open: {}", name, e));
        let notelist =
            parse_notelist(file).unwrap_or_else(|e| panic!("[{}] Failed to parse: {:?}", name, e));

        assert!(
            !notelist.records.is_empty(),
            "[{}] Notelist should have records",
            name
        );

        let score = notelist_to_score(&notelist);

        assert!(
            !score.objects.is_empty(),
            "[{}] Score should have objects",
            name
        );

        // Every score needs at least one SYNC (note/rest container)
        let sync_count = score
            .objects
            .iter()
            .filter(|o| o.header.obj_type as u8 == nightingale_core::defs::SYNC_TYPE)
            .count();
        assert!(
            sync_count > 0,
            "[{}] Score should have SYNC objects, got 0",
            name
        );

        println!(
            "[{}] OK: {} records → {} objects ({} syncs)",
            name,
            notelist.records.len(),
            score.objects.len(),
            sync_count
        );
    }
}

// ============================================================================
// Render + Geometry: every file should produce renderable, sane output
// ============================================================================

#[test]
fn test_all_notelists_render_and_geometry() {
    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file = fs::File::open(path).unwrap();
        let notelist = parse_notelist(file).unwrap();
        let score = notelist_to_score(&notelist);

        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();

        let staff_count = count_by_name(&commands, "Staff");
        let barline_count = count_by_name(&commands, "BarLine");
        let music_char_count = count_by_name(&commands, "MusicChar");
        let stem_count = count_by_name(&commands, "NoteStem");

        // Basic content assertions
        assert!(staff_count > 0, "[{}] Should render at least 1 staff", name);
        assert!(barline_count > 0, "[{}] Should render barlines", name);
        assert!(
            music_char_count > 0,
            "[{}] Should render noteheads/rests",
            name
        );

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
                    assert_eq!(*n_lines, 5, "[{}] Staff should have 5 lines", name);
                    assert!(
                        *line_spacing > 3.0 && *line_spacing < 15.0,
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
                    // X should be within page width (612pt) with margin.
                    // Y can extend far beyond the page for scores with many
                    // systems — multi-page layout is future work.
                    assert!(
                        *x > -10.0 && *x < 700.0,
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
fn test_all_notelists_produce_valid_pdf() {
    let output_dir = Path::new("test-output/notelists");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let config = NotelistLayoutConfig::default();
    let font_path = Path::new("assets/fonts/Bravura.otf");

    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file = fs::File::open(path).unwrap();
        let notelist = parse_notelist(file).unwrap();
        let score = notelist_to_score(&notelist);

        let mut pdf_renderer =
            PdfRenderer::new(config.page_width as f32, config.page_height as f32);

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
            "[{}] PDF: {} bytes → {}",
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
fn test_all_notelists_regression_snapshots() {
    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file = fs::File::open(path).unwrap();
        let notelist = parse_notelist(file).unwrap();
        let score = notelist_to_score(&notelist);

        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();

        let summary = render_summary(&commands);
        insta::assert_snapshot!(format!("notelist_{}", name), summary);
    }
}

// ============================================================================
// Score content validation: check each file has expected musical content
// ============================================================================

#[test]
fn test_all_notelists_score_content() {
    use nightingale_core::notelist::parser::NotelistRecord;

    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file = fs::File::open(path).unwrap();
        let notelist = parse_notelist(file).unwrap();

        let note_count = notelist
            .records
            .iter()
            .filter(|r| matches!(r, NotelistRecord::Note { .. }))
            .count();
        let rest_count = notelist
            .records
            .iter()
            .filter(|r| matches!(r, NotelistRecord::Rest { .. }))
            .count();
        let barline_count = notelist
            .records
            .iter()
            .filter(|r| matches!(r, NotelistRecord::Barline { .. }))
            .count();
        let clef_count = notelist
            .records
            .iter()
            .filter(|r| matches!(r, NotelistRecord::Clef { .. }))
            .count();
        let keysig_count = notelist
            .records
            .iter()
            .filter(|r| matches!(r, NotelistRecord::KeySig { .. }))
            .count();
        let timesig_count = notelist
            .records
            .iter()
            .filter(|r| matches!(r, NotelistRecord::TimeSig { .. }))
            .count();

        // Every piece should have notes or rests
        assert!(
            note_count > 0 || rest_count > 0,
            "[{}] Should have notes or rests (notes={}, rests={})",
            name,
            note_count,
            rest_count
        );

        // Every piece should have at least one barline
        assert!(barline_count > 0, "[{}] Should have barlines", name);

        // Every piece should declare clefs
        assert!(clef_count > 0, "[{}] Should have clef declarations", name);

        // Staves should be declared in part_staves
        let total_staves: u8 = notelist.part_staves.iter().sum();
        assert!(
            total_staves > 0,
            "[{}] Should declare at least 1 staff in part_staves",
            name
        );

        println!(
            "[{}] Content: {} notes, {} rests, {} barlines, {} clefs, {} keysigs, {} timesigs, {} staves",
            name, note_count, rest_count, barline_count, clef_count, keysig_count, timesig_count, total_staves
        );
    }
}

// ============================================================================
// Beam rendering: every file with eighth notes or shorter should have beams
// ============================================================================

#[test]
fn test_all_notelists_beam_presence() {
    use nightingale_core::notelist::parser::NotelistRecord;

    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file = fs::File::open(path).unwrap();
        let notelist = parse_notelist(file).unwrap();

        // Check if the file has eighth notes or shorter (dur >= 5 in l_dur encoding)
        let has_beamable_notes = notelist.records.iter().any(|r| match r {
            NotelistRecord::Note { dur, .. } | NotelistRecord::Rest { dur, .. } => *dur >= 5,
            _ => false,
        });

        if !has_beamable_notes {
            println!("[{}] SKIP: no beamable durations", name);
            continue;
        }

        let score = notelist_to_score(&notelist);
        let mut cmd_renderer = CommandRenderer::new();
        render_score(&score, &mut cmd_renderer);
        let commands = cmd_renderer.take_commands();

        let beam_count = count_by_name(&commands, "Beam");

        // Files with eighth notes should produce beams
        assert!(
            beam_count > 0,
            "[{}] Has beamable notes but produced 0 beams",
            name
        );

        println!("[{}] Beams: {}", name, beam_count);
    }
}

// ============================================================================
// Command-stream hash: exact render output fingerprint for refactor safety
// ============================================================================

/// Exact render-command fingerprint for every Notelist fixture.
///
/// Same approach as the NGL hash test: captures the full sequence of render
/// commands. Any behavioral change will break the hash.
/// Use `REGENERATE_REFS=1 cargo test` to update baselines.
#[test]
fn test_all_notelists_command_stream_hashes() {
    let regenerate = std::env::var("REGENERATE_REFS").is_ok();

    let expected: std::collections::HashMap<&str, u64> = [
        ("accidentals", 15210232347600923921),
        ("BachEbSonata_20_2sizes", 11382956659039491826),
        ("BachEbSonata_20", 11382956659039491826),
        ("BachStAnne_63", 12511530941174060159),
        ("barline_types", 11826322165668403117),
        ("bass_clef_melody", 6143118881930782026),
        ("beamed_eighths", 16817476029656344587),
        ("BinchoisDePlus_17", 1516664038146987230),
        ("chord_seconds", 17547428817055251097),
        ("chromatic_scale", 13380106191125696871),
        ("clef_change", 13675144730715349278),
        ("compound_meter", 3448981925513776405),
        ("Debussy_Images_9", 5120217451688475872),
        ("dotted_notes", 15719983019693230465),
        ("GoodbyePorkPieHat", 8779202306363999964),
        ("grace_notes_test", 9369185261854780981),
        ("HBD_33", 13197508482154814260),
        ("keysig_d_major", 17637476394096891670),
        ("keysig_eb_major", 7140134166321384013),
        ("keysig_flats_all", 13184331472989445439),
        ("keysig_sharps_all", 10566261538645605626),
        ("KillingMe_36", 7968330356284126405),
        ("ledger_lines", 4530816715993400936),
        ("MahlerLiedVonDE_25", 144027592274616623),
        ("MendelssohnOp7N1_2", 2421060808097750609),
        ("mixed_durations", 12316681405162711887),
        ("RavelScarbo_15", 703212242279790912),
        ("rests_all_durations", 12343596771520387979),
        ("SchenkerDiagram_Chopin_6", 3968651988560332157),
        ("SchoenbergOp19N1_21", 1053409238410573459),
        ("sixteenths_32nds", 1022292751088294398),
        ("TestMIDIChannels_3", 15254790807438789485),
        ("text_annotations", 11807005625538175866),
        ("tied_notes", 5170243110835216819),
        ("time_sig_changes", 12261926148649077794),
        ("tuplet_quintuplet", 5735586467474035569),
        ("tuplet_triplet", 438869844570012267),
        ("two_voices", 8464170528683105269),
        ("Webern_Op5N3_22", 12484795250392454088),
        ("whole_notes", 203519566342081697),
        ("wide_intervals", 16876631074885473963),
    ]
    .into_iter()
    .collect();

    let mut all_ok = true;
    for path in ALL_NOTELISTS {
        let name = short_name(path);
        let file = fs::File::open(path).unwrap();
        let notelist = parse_notelist(file).unwrap();
        let score = notelist_to_score(&notelist);

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
         Run `REGENERATE_REFS=1 cargo test test_all_notelists_command_stream_hashes -- --nocapture` \
         to regenerate baselines after intentional changes."
    );
}

// ============================================================================
// 7. Bitmap regression (Notelist → BitmapRenderer → PNG → pixel diff against golden)
// ============================================================================

// save_bitmap_page and compare_images_and_diff are in tests/common/mod.rs

/// Visual regression test: BitmapRenderer → PNG → pixel diff against golden.
///
/// Uses pure-Rust BitmapRenderer (tiny-skia) — no external PDF-to-PNG tools needed.
/// On mismatch, generates a visual diff image (matching pixels dimmed, diffs in red).
///
/// Regenerate goldens: `REGENERATE_REFS=1 cargo test test_all_notelists_bitmap_regression`
#[test]
fn test_all_notelists_bitmap_regression() {
    let regenerate = std::env::var("REGENERATE_REFS").is_ok();
    let golden_dir = Path::new("tests/golden_bitmaps");
    let diff_dir = Path::new("test-output/notelist-bitmap-diff");
    fs::create_dir_all(diff_dir).unwrap();

    let font_dir = Path::new("assets/fonts");
    let font_path = font_dir.join("Bravura.otf");
    let font_data = fs::read(&font_path).ok();
    let config = NotelistLayoutConfig::default();
    let mut mismatches = Vec::new();

    for path in ALL_NOTELISTS {
        let name = short_name(path);

        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[{}] open error: {}", name, e);
                continue;
            }
        };
        let notelist = match parse_notelist(file) {
            Ok(nl) => nl,
            Err(e) => {
                eprintln!("[{}] parse error: {}", name, e);
                continue;
            }
        };
        let score = notelist_to_score_with_config(&notelist, &config);

        // Render directly to bitmap (no PDF intermediate)
        let mut bmp = BitmapRenderer::new(72.0); // 72 DPI = 1 pixel per point
        bmp.set_page_size(config.page_width as f32, config.page_height as f32);
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

            let golden_path = golden_dir.join(format!("nl_{}{}.png", name, page_suffix));

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
            "Notelist bitmap mismatches in {} fixture(s): {}\n\
             Visual diff images: open test-output/notelist-bitmap-diff/\n\
             Regenerate goldens: REGENERATE_REFS=1 cargo test test_all_notelists_bitmap_regression",
            mismatches.len(),
            mismatches.join(", ")
        );
    }
}
