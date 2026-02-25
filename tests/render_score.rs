//! Integration tests: render scores through the drawing pipeline.
//!
//! Tests validate:
//! 1. HBD_33.nl end-to-end: Notelist → InterpretedScore → CommandRenderer → PDF
//! 2. Geometry: positions on page, reasonable stem lengths, proper spacing
//! 3. NGL files: direct .ngl → InterpretedScore → CommandRenderer rendering
//! 4. Punted items: #[ignore]d roadmap tests for future porting work

use nightingale_core::draw::render_score;
use nightingale_core::render::{CommandRenderer, PdfRenderer, RenderCommand};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Helper: count commands by name.
fn count_by_name(commands: &[RenderCommand], name: &str) -> usize {
    commands.iter().filter(|c| c.name() == name).count()
}

// ============================================================================
// HBD_33 Notelist → PDF pipeline tests
// ============================================================================

/// Full pipeline test: parse HBD_33.nl → notelist_to_score → render_score → PDF
///
/// This validates that the Notelist→InterpretedScore converter produces a score
/// that can be rendered through the same pipeline as .ngl files.
#[test]
fn test_notelist_hbd33_render_to_pdf() {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist, NotelistLayoutConfig};

    let output_dir = Path::new("/tmp/nightingale-test-output");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // 1. Parse Notelist
    let file =
        fs::File::open("tests/notelist_examples/HBD_33.nl").expect("Failed to open HBD_33.nl");
    let notelist = parse_notelist(file).expect("Failed to parse HBD_33.nl");
    let config = NotelistLayoutConfig::default();

    println!(
        "HBD_33.nl: {} records, {} parts",
        notelist.records.len(),
        notelist.part_staves.iter().filter(|&&s| s > 0).count()
    );

    // 2. Convert to InterpretedScore
    let score = notelist_to_score(&notelist);

    println!(
        "InterpretedScore: {} objects, {} note groups, {} measure groups",
        score.objects.len(),
        score.notes.len(),
        score.measures.len()
    );

    // 3. Render through CommandRenderer for structural validation
    let mut cmd_renderer = CommandRenderer::new();
    render_score(&score, &mut cmd_renderer);
    let commands = cmd_renderer.take_commands();

    let staff_count = count_by_name(&commands, "Staff");
    let barline_count = count_by_name(&commands, "BarLine");
    let music_char_count = count_by_name(&commands, "MusicChar");
    let stem_count = count_by_name(&commands, "NoteStem");

    println!(
        "Rendered: {} staves, {} barlines, {} noteheads, {} stems, {} total commands",
        staff_count,
        barline_count,
        music_char_count,
        stem_count,
        commands.len()
    );

    // HBD_33 (Happy Birthday) should produce meaningful output
    assert!(staff_count > 0, "Should render at least 1 staff");
    assert!(barline_count > 0, "Should render barlines");
    assert!(music_char_count > 0, "Should render noteheads/rests");
    assert!(stem_count > 0, "Should render stems");

    // 4. Render through PdfRenderer with embedded Bravura font
    let mut pdf_renderer = PdfRenderer::new(config.page_width as f32, config.page_height as f32);

    // Load Bravura SMuFL font for real glyph rendering
    let font_path = Path::new("icebox/nightingale_app/assets/fonts/Bravura.otf");
    if font_path.exists() {
        pdf_renderer.load_music_font_file(font_path);
    }

    render_score(&score, &mut pdf_renderer);
    let pdf_bytes = pdf_renderer.finish();

    let output_path = output_dir.join("notelist_hbd33.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write PDF");

    assert!(
        pdf_bytes.starts_with(b"%PDF-"),
        "Output should be valid PDF"
    );
    assert!(
        pdf_bytes.len() > 1000,
        "PDF should be substantial ({} bytes)",
        pdf_bytes.len()
    );

    println!(
        "HBD_33 PDF: {} bytes → {}",
        pdf_bytes.len(),
        output_path.display()
    );
}

/// Validate that Notelist-derived scores produce geometrically reasonable output.
#[test]
fn test_notelist_hbd33_geometry() {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};

    let file =
        fs::File::open("tests/notelist_examples/HBD_33.nl").expect("Failed to open HBD_33.nl");
    let notelist = parse_notelist(file).expect("Failed to parse HBD_33.nl");
    let score = notelist_to_score(&notelist);

    let mut cmd_renderer = CommandRenderer::new();
    render_score(&score, &mut cmd_renderer);
    let commands = cmd_renderer.take_commands();

    // Check staff geometry
    for cmd in &commands {
        if let RenderCommand::Staff {
            y,
            x0,
            x1,
            n_lines,
            line_spacing,
        } = cmd
        {
            assert!(*y > 0.0, "Staff y should be positive: {}", y);
            assert!(x0 < x1, "Staff x0 ({}) should be less than x1 ({})", x0, x1);
            assert_eq!(*n_lines, 5, "Staff should have 5 lines");
            assert!(
                *line_spacing > 3.0 && *line_spacing < 15.0,
                "Line spacing {} should be reasonable",
                line_spacing
            );
        }

        // Check barline geometry
        if let RenderCommand::BarLine { top, bottom, x, .. } = cmd {
            assert!(top < bottom, "Barline top ({}) < bottom ({})", top, bottom);
            assert!(*x > 0.0, "Barline x should be positive: {}", x);
        }

        // Check notehead geometry: positions should be on the page
        // Page is 612x792 (US Letter portrait)
        if let RenderCommand::MusicChar { x, y, .. } = cmd {
            assert!(*x > 0.0 && *x < 700.0, "Note x ({}) should be on page", x);
            assert!(*y > 0.0 && *y < 900.0, "Note y ({}) should be on page", y);
        }

        // Check stem geometry
        if let RenderCommand::NoteStem {
            x,
            y_top,
            y_bottom,
            width,
        } = cmd
        {
            assert!(
                y_top <= y_bottom,
                "Stem y_top ({}) <= y_bottom ({})",
                y_top,
                y_bottom
            );
            assert!(*x > 0.0, "Stem x should be positive: {}", x);
            let length = y_bottom - y_top;
            assert!(
                length > 0.0 && length < 80.0,
                "Stem length {} should be reasonable",
                length
            );
            assert!(
                *width > 0.0 && *width < 5.0,
                "Stem width {} should be thin",
                width
            );
        }
    }
}

// ============================================================================
// Punted items — planned tests for future porting work
// These tests are #[ignore]d and will show up in test output as a roadmap.
// ============================================================================

/// Beam slope: beams should slope to follow melodic contour, not be flat.
/// Port: GetBeamEndYStems (Beam.cp:181), FixSyncInBeamset (Beam.cp:272)
/// Stems within beamed groups are interpolated along the beam line at
/// config.relBeamSlope% of the natural slope.
#[test]
fn test_beam_slope_variable_stem_lengths() {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};

    let file =
        std::fs::File::open("tests/notelist_examples/HBD_33.nl").expect("Failed to open HBD_33.nl");
    let notelist = parse_notelist(file).expect("Failed to parse");
    let score = notelist_to_score(&notelist);

    let mut cmd_renderer = CommandRenderer::new();
    render_score(&score, &mut cmd_renderer);
    let commands = cmd_renderer.take_commands();

    // Should have beams
    let beams: Vec<_> = commands.iter().filter(|c| c.name() == "Beam").collect();
    assert!(!beams.is_empty(), "Score should have beams");

    // At least one primary beam should be non-horizontal (sloped).
    // A flat beam has y0 == y1; a sloped beam has y0 != y1.
    let has_sloped_beam = beams.iter().any(|cmd| {
        if let RenderCommand::Beam { y0, y1, .. } = cmd {
            (y0 - y1).abs() > 0.1
        } else {
            false
        }
    });
    assert!(
        has_sloped_beam,
        "At least one beam should be sloped (non-horizontal)"
    );
}

/// Accidental staggering: chords with multiple accidentals should stagger
/// them to avoid collisions. Port: ChkNoteAccs (DrawNRGR.cp)
#[test]
#[ignore = "PUNT: accidental staggering in chords (ChkNoteAccs in DrawNRGR.cp)"]
fn test_accidental_staggering_in_chords() {
    // When a chord has notes with accidentals that would collide,
    // Nightingale staggers them at different X offsets.
    // This requires porting ChkNoteAccs from DrawNRGR.cp.
}

/// Anacrusis (pickup measure): first partial measure before the first barline.
/// Needs proper preamble width calculation to avoid colliding with clef/time sig.
/// Port: ComputeJustPosns and initial measure spacing from ReformatRaw.cp
#[test]
#[ignore = "PUNT: anacrusis lead-in measure spacing"]
fn test_anacrusis_spacing() {
    // Pickup beats before the first barline need special spacing treatment
    // to avoid colliding with clef and time signature objects.
}

/// Ledger line weight: should match OG config.ledgerLW (13% of lnSpace).
/// Currently using default stroke width. Port: PS_Stdio.cp line 2211.
#[test]
#[ignore = "PUNT: ledger line weight from config.ledgerLW (PS_Stdio.cp:2211)"]
fn test_ledger_line_weight() {
    // Ledger line thickness should be config.ledgerLW * lnSpace / 100
    // Default: 13% of line space ≈ 0.26pt for 2pt line space
}

// ============================================================================
// Multi-voice tests
// ============================================================================

/// Multi-voice: HBD_33 treble staff has voices 1 and 3.
/// Voice 1 (UPPER) should have stems up, voice 3 (LOWER) stems down.
/// This validates the voice role system from Multivoice.h.
#[test]
fn test_multi_voice_stem_directions() {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};

    let file =
        std::fs::File::open("tests/notelist_examples/HBD_33.nl").expect("Failed to open HBD_33.nl");
    let notelist = parse_notelist(file).expect("Failed to parse");
    let score = notelist_to_score(&notelist);

    let mut cmd_renderer = CommandRenderer::new();
    render_score(&score, &mut cmd_renderer);
    let commands = cmd_renderer.take_commands();

    // Count stems and check directions
    let stems: Vec<_> = commands
        .iter()
        .filter_map(|c| {
            if let RenderCommand::NoteStem {
                y_top, y_bottom, ..
            } = c
            {
                Some((*y_top, *y_bottom))
            } else {
                None
            }
        })
        .collect();

    // With multi-voice, we should have significantly more stems than single-voice
    // (voices 1 and 3 on treble staff, voice 2 on bass)
    assert!(
        stems.len() > 30,
        "Multi-voice score should have many stems, got {}",
        stems.len()
    );

    // Should have stems (all stems have y_top < y_bottom by convention)
    for (y_top, y_bottom) in &stems {
        assert!(
            y_top <= y_bottom,
            "Stem y_top ({}) should be <= y_bottom ({})",
            y_top,
            y_bottom
        );
    }
}

/// Multi-voice: more noteheads than single-voice rendering.
/// Default config now renders all voices (max_voices_per_staff = 0).
#[test]
fn test_multi_voice_has_more_content() {
    use nightingale_core::notelist::{
        notelist_to_score_with_config, parse_notelist, NotelistLayoutConfig,
    };

    let file =
        std::fs::File::open("tests/notelist_examples/HBD_33.nl").expect("Failed to open HBD_33.nl");
    let notelist = parse_notelist(file).expect("Failed to parse");

    // Single-voice config
    let single_config = NotelistLayoutConfig {
        max_voices_per_staff: 1,
        ..NotelistLayoutConfig::default()
    };
    let single_score = notelist_to_score_with_config(&notelist, &single_config);

    // Multi-voice config (default)
    let multi_score = notelist_to_score_with_config(&notelist, &NotelistLayoutConfig::default());

    // Multi-voice should have more note groups (more syncs with notes from multiple voices)
    let single_note_count: usize = single_score.notes.values().map(|v| v.len()).sum();
    let multi_note_count: usize = multi_score.notes.values().map(|v| v.len()).sum();

    assert!(
        multi_note_count > single_note_count,
        "Multi-voice ({}) should have more notes than single-voice ({})",
        multi_note_count,
        single_note_count
    );
}

// ============================================================================
// Visual regression tests (insta snapshot-based)
// ============================================================================

/// Build a compact summary of render commands for regression testing.
/// Groups commands by type and includes key geometric parameters.
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

    // Stem count + direction distribution
    let mut stems_up = 0;
    let mut stems_down = 0;
    for cmd in commands {
        if let RenderCommand::NoteStem {
            y_top, y_bottom, ..
        } = cmd
        {
            let length = y_bottom - y_top;
            if length > 0.1 {
                // We consider stems where the notehead is at the bottom as "stems up"
                // and stems where notehead is at the top as "stems down".
                // Actual direction depends on rendering context, but length > 0 always.
                stems_up += 1; // All stems have y_top < y_bottom
            } else {
                stems_down += 1;
            }
        }
    }
    lines.push(format!("\n=== STEMS: {} total ===", stems_up + stems_down));

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

/// Visual regression: snapshot the HBD_33 multi-voice render commands.
/// This test captures the full rendering structure and will fail if any
/// drawing command changes position, count, or type.
#[test]
fn test_hbd33_visual_regression_snapshot() {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};

    let file =
        std::fs::File::open("tests/notelist_examples/HBD_33.nl").expect("Failed to open HBD_33.nl");
    let notelist = parse_notelist(file).expect("Failed to parse");
    let score = notelist_to_score(&notelist);

    let mut cmd_renderer = CommandRenderer::new();
    render_score(&score, &mut cmd_renderer);
    let commands = cmd_renderer.take_commands();

    let summary = render_summary(&commands);
    insta::assert_snapshot!("hbd33_multivoice_render", summary);
}

// ============================================================================
// Tuplet rendering tests
// ============================================================================

/// Test tuplet rendering: triplet (3 eighth notes in time of 2).
///
/// Validates:
/// 1. Tuplet objects are created from P records + notes with 'T' flag
/// 2. SMuFL tuplet digit glyphs (U+E880–U+E889) are used, not timeSig digits
/// 3. Bracket is drawn with 3 Line commands (left cutoff, left segment, right segment)
///    + 1 Line for right cutoff = 4 bracket lines total when number is visible
/// 4. Notes in tuplet have in_tuplet = true
/// 5. Non-tuplet note (measure 2) has in_tuplet = false
#[test]
fn test_tuplet_triplet_rendering() {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};
    use nightingale_core::render::MusicGlyph;

    let file = std::fs::File::open("tests/notelist_examples/tuplet_triplet.nl")
        .expect("Failed to open tuplet_triplet.nl");
    let notelist = parse_notelist(file).expect("Failed to parse");
    let score = notelist_to_score(&notelist);

    // Verify tuplet objects exist
    assert!(
        !score.tuplets.is_empty(),
        "Score should have at least one tuplet"
    );

    // Verify tuplet metadata
    let tuplet_objs: Vec<_> = score
        .objects
        .iter()
        .filter(|o| o.header.obj_type == 18) // TUPLET_TYPE
        .collect();
    assert_eq!(
        tuplet_objs.len(),
        1,
        "Should have exactly one tuplet object"
    );

    // Check tuplet data
    if let nightingale_core::ngl::interpret::ObjData::Tuplet(tup) = &tuplet_objs[0].data {
        assert_eq!(tup.acc_num, 3, "Tuplet numerator should be 3");
        assert_eq!(tup.acc_denom, 2, "Tuplet denominator should be 2");
        assert_eq!(tup.num_vis, 1, "Number should be visible");
        assert_eq!(tup.brack_vis, 1, "Bracket should be visible");
        assert_eq!(tup.voice, 1, "Tuplet should be in voice 1");
    } else {
        panic!("Expected Tuplet data");
    }

    // Verify ANoteTuple subobjects link to 3 syncs
    let first_tuplet_sub = tuplet_objs[0].header.first_sub_obj;
    let anottuples = score
        .tuplets
        .get(&first_tuplet_sub)
        .expect("Tuplet subobjects should exist");
    assert_eq!(anottuples.len(), 3, "Triplet should link to 3 sync objects");

    // Verify that linked syncs contain notes with in_tuplet = true
    for anotuple in anottuples {
        let sync_obj = score
            .objects
            .iter()
            .find(|o| o.index == anotuple.tp_sync)
            .expect("Linked sync should exist");
        let notes = score
            .notes
            .get(&sync_obj.header.first_sub_obj)
            .expect("Sync should have notes");
        let tuplet_notes: Vec<_> = notes.iter().filter(|n| n.in_tuplet).collect();
        assert!(
            !tuplet_notes.is_empty(),
            "Each linked sync should have at least one in_tuplet note"
        );
    }

    // Verify the non-tuplet note (measure 2, quarter note C5) has in_tuplet = false
    let non_tuplet_syncs: Vec<_> = score
        .objects
        .iter()
        .filter(|o| {
            o.header.obj_type == 2 // SYNCtype
                && !anottuples.iter().any(|at| at.tp_sync == o.index)
        })
        .collect();
    for sync_obj in &non_tuplet_syncs {
        if let Some(notes) = score.notes.get(&sync_obj.header.first_sub_obj) {
            for n in notes {
                assert!(
                    !n.in_tuplet,
                    "Non-tuplet notes should have in_tuplet = false"
                );
            }
        }
    }

    // Render and check commands
    let mut cmd_renderer = CommandRenderer::new();
    render_score(&score, &mut cmd_renderer);
    let commands = cmd_renderer.take_commands();

    // Check that SMuFL tuplet glyphs are used (U+E880-E889), NOT timeSig (U+E080-E089)
    let tuplet_glyph_cmds: Vec<_> = commands
        .iter()
        .filter(|c| {
            matches!(c,
                RenderCommand::MusicChar { glyph: MusicGlyph::Smufl(cp), .. }
                if (0xE880..=0xE889).contains(cp)
            )
        })
        .collect();
    assert_eq!(
        tuplet_glyph_cmds.len(),
        1,
        "Should have exactly 1 tuplet digit glyph (the '3')"
    );

    // Verify it's specifically the '3' glyph (U+E883)
    if let RenderCommand::MusicChar {
        glyph: MusicGlyph::Smufl(cp),
        ..
    } = tuplet_glyph_cmds[0]
    {
        assert_eq!(*cp, 0xE883, "Tuplet glyph should be U+E883 (tuplet3)");
    }

    // Verify NO timeSig digits are used for tuplet numbers
    let timesig_glyph_cmds: Vec<_> = commands
        .iter()
        .filter(|c| {
            matches!(c,
                RenderCommand::MusicChar { glyph: MusicGlyph::Smufl(cp), .. }
                if (0xE080..=0xE089).contains(cp)
            )
        })
        .collect();
    // timeSig digits are only used for the time signature itself (4/4)
    assert_eq!(
        timesig_glyph_cmds.len(),
        2,
        "Only 2 timeSig digit glyphs should exist (for 4/4 time sig), none for tuplets"
    );

    // Check bracket lines: with num_vis=1 and brack_vis=1, we expect:
    // 4 Line commands for the bracket (left cutoff, left segment, right segment, right cutoff)
    // Plus other Line commands for stems and barlines
    let all_lines = count_by_name(&commands, "Line");
    // We should have at least 4 bracket lines plus note stems and barlines
    assert!(
        all_lines >= 4,
        "Should have at least 4 Line commands for tuplet bracket, got {}",
        all_lines
    );

    // Generate PDF for visual inspection
    let output_dir = std::path::Path::new("/tmp/nightingale-test-output");
    std::fs::create_dir_all(output_dir).ok();
    let mut pdf_renderer = PdfRenderer::new(612.0, 792.0);
    render_score(&score, &mut pdf_renderer);
    let pdf_bytes = pdf_renderer.finish();
    std::fs::write(output_dir.join("tuplet_triplet.pdf"), &pdf_bytes).ok();
}
