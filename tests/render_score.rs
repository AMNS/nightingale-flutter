//! Integration tests: render Notelist-derived scores through the drawing pipeline.
//!
//! Tests validate:
//! 1. HBD_33.nl end-to-end: Notelist → InterpretedScore → CommandRenderer → PDF
//! 2. Geometry: positions on page, reasonable stem lengths, proper spacing
//! 3. Punted items: #[ignore]d roadmap tests for future porting work

use nightingale_core::draw::render_score;
use nightingale_core::render::{CommandRenderer, PdfRenderer, RenderCommand};
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
