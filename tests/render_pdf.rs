// Integration tests for PDF rendering via PdfRenderer
//
// These tests verify that PdfRenderer (pdf-writer backend) correctly
// implements the MusicRenderer trait by generating real PDF files.
// PDFs are saved to test-output/ for visual inspection.
//
// For automated comparison, use `sips` (macOS) or Ghostscript to convert
// PDF→PNG, then compare against blessed bitmaps.

use nightingale_core::render::pdf_renderer::PdfRenderer;
use nightingale_core::render::types::{BarLineType, Color, MusicGlyph, Point, RenderRect};
use nightingale_core::render::MusicRenderer;
use std::fs;
use std::path::Path;

/// Helper to create test output directory
fn ensure_output_dir() -> String {
    let dir = "test-output";
    fs::create_dir_all(dir).expect("Failed to create test output directory");
    dir.to_string()
}

/// Helper to write PDF and verify it's valid
fn write_pdf(renderer: PdfRenderer, name: &str) -> Vec<u8> {
    let pdf_bytes = renderer.finish();
    assert!(
        pdf_bytes.starts_with(b"%PDF-"),
        "Output should be valid PDF"
    );
    assert!(pdf_bytes.len() > 100, "PDF should have meaningful content");

    let dir = ensure_output_dir();
    let path = format!("{}/{}.pdf", dir, name);
    fs::write(&path, &pdf_bytes).expect("Failed to write PDF");
    assert!(Path::new(&path).exists());

    pdf_bytes
}

// ========== Staff & Bar Line Tests ==========

#[test]
fn test_pdf_staff_5line() {
    let mut r = PdfRenderer::new(612.0, 792.0); // US Letter
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    // Staff at 1 inch from top, spanning full width with 1-inch margins
    r.staff(72.0, 72.0, 540.0, 5, 7.0);

    write_pdf(r, "pdf_staff_5line");
}

#[test]
fn test_pdf_staff_1line() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    // Percussion staff (1 line)
    r.staff(72.0, 72.0, 540.0, 1, 7.0);
    write_pdf(r, "pdf_staff_1line");
}

#[test]
fn test_pdf_bar_line_types() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;
    let bottom = staff_y + 4.0 * line_sp;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Single
    r.bar_line(staff_y, bottom, 120.0, BarLineType::Single, line_sp);
    // Double
    r.bar_line(staff_y, bottom, 220.0, BarLineType::Double, line_sp);
    // Final double
    r.bar_line(staff_y, bottom, 320.0, BarLineType::FinalDouble, line_sp);
    // Repeat left
    r.bar_line(staff_y, bottom, 420.0, BarLineType::RepeatLeft, line_sp);

    write_pdf(r, "pdf_bar_line_types");
}

#[test]
fn test_pdf_connector_line() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let line_sp = 7.0;
    // Two staves connected by a system connector
    r.staff(72.0, 72.0, 540.0, 5, line_sp);
    r.staff(72.0 + 60.0, 72.0, 540.0, 5, line_sp);
    // Connector from top of staff 1 to bottom of staff 2
    r.connector_line(72.0, 72.0 + 60.0 + 4.0 * line_sp, 72.0);

    write_pdf(r, "pdf_connector_line");
}

// ========== Note Element Tests ==========

#[test]
fn test_pdf_stems_and_noteheads() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Note on each staff line, alternating stem direction
    let positions = [
        (120.0, staff_y + 4.0 * line_sp, true), // Bottom line, stem up
        (180.0, staff_y + 3.0 * line_sp, true), // 2nd line, stem up
        (240.0, staff_y + 2.0 * line_sp, false), // Middle line, stem down
        (300.0, staff_y + 1.0 * line_sp, false), // 4th line, stem down
        (360.0, staff_y, false),                // Top line, stem down
    ];

    for (x, y, stem_up) in positions {
        // Placeholder notehead (will be real SMuFL glyph later)
        r.music_char(x, y, MusicGlyph::smufl(0xE0A3), 100.0);
        if stem_up {
            r.note_stem(x + 3.0, y, y - 25.0, 0.8); // Stem right side, going up
        } else {
            r.note_stem(x - 3.0, y, y + 25.0, 0.8); // Stem left side, going down
        }
    }

    write_pdf(r, "pdf_stems_noteheads");
}

#[test]
fn test_pdf_ledger_lines() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 100.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Notes above staff with ledger lines (stems down = left side)
    for i in 1..=3 {
        let y = staff_y - (i as f32) * line_sp;
        let x = 120.0 + (i as f32) * 60.0;
        r.music_char(x, y, MusicGlyph::smufl(0xE0A3), 100.0);
        r.note_stem(x - 3.0, y, y + 25.0, 0.8);
        // Ledger lines from staff top down to the note
        for j in 1..=i {
            r.ledger_line(staff_y - (j as f32) * line_sp, x, 8.0);
        }
    }

    // Notes below staff with ledger lines
    let bottom = staff_y + 4.0 * line_sp;
    for i in 1..=3 {
        let y = bottom + (i as f32) * line_sp;
        let x = 350.0 + (i as f32) * 60.0;
        r.music_char(x, y, MusicGlyph::smufl(0xE0A3), 100.0);
        r.note_stem(x + 3.0, y, y - 25.0, 0.8);
        for j in 1..=i {
            r.ledger_line(bottom + (j as f32) * line_sp, x, 8.0);
        }
    }

    write_pdf(r, "pdf_ledger_lines");
}

// ========== Beam Tests ==========

#[test]
fn test_pdf_beam_level() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Level beam (both notes same height)
    let note_y = staff_y + 3.0 * line_sp;
    let beam_y = note_y - 25.0;
    r.music_char(120.0, note_y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.music_char(180.0, note_y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.note_stem(123.0, note_y, beam_y, 0.8);
    r.note_stem(183.0, note_y, beam_y, 0.8);
    r.beam(123.0, beam_y, 183.0, beam_y, 3.5, true, true);

    write_pdf(r, "pdf_beam_level");
}

#[test]
fn test_pdf_beam_ascending() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Ascending beam (notes going up)
    let y0 = staff_y + 4.0 * line_sp;
    let y1 = staff_y + 2.0 * line_sp;
    let beam_y0 = y0 - 25.0;
    let beam_y1 = y1 - 25.0;

    r.music_char(120.0, y0, MusicGlyph::smufl(0xE0A3), 100.0);
    r.music_char(200.0, y1, MusicGlyph::smufl(0xE0A3), 100.0);
    r.note_stem(123.0, y0, beam_y0, 0.8);
    r.note_stem(203.0, y1, beam_y1, 0.8);
    r.beam(123.0, beam_y0, 203.0, beam_y1, 3.5, true, true);

    write_pdf(r, "pdf_beam_ascending");
}

#[test]
fn test_pdf_beam_double() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Double beam (sixteenth notes)
    let note_y = staff_y + 3.0 * line_sp;
    let beam_y = note_y - 25.0;
    let beam2_y = beam_y - 4.0; // Second beam 4pt above first

    r.music_char(120.0, note_y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.music_char(180.0, note_y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.note_stem(123.0, note_y, beam_y, 0.8);
    r.note_stem(183.0, note_y, beam_y, 0.8);
    // Primary beam
    r.beam(123.0, beam_y, 183.0, beam_y, 3.5, true, true);
    // Secondary beam
    r.beam(123.0, beam2_y, 183.0, beam2_y, 3.5, true, true);

    write_pdf(r, "pdf_beam_double");
}

// ========== Slur Tests ==========

#[test]
fn test_pdf_slur_above() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 100.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Slur above (arc curves upward = smaller Y in top-left origin)
    let y = staff_y + 2.0 * line_sp;
    r.music_char(120.0, y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.music_char(300.0, y, MusicGlyph::smufl(0xE0A3), 100.0);

    r.slur(
        Point {
            x: 120.0,
            y: y - 5.0,
        },
        Point {
            x: 170.0,
            y: y - 20.0,
        },
        Point {
            x: 250.0,
            y: y - 20.0,
        },
        Point {
            x: 300.0,
            y: y - 5.0,
        },
        false,
    );

    write_pdf(r, "pdf_slur_above");
}

#[test]
fn test_pdf_slur_below() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Slur below (arc curves downward = larger Y)
    let y = staff_y + 2.0 * line_sp;
    r.music_char(120.0, y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.music_char(300.0, y, MusicGlyph::smufl(0xE0A3), 100.0);

    r.slur(
        Point {
            x: 120.0,
            y: y + 5.0,
        },
        Point {
            x: 170.0,
            y: y + 20.0,
        },
        Point {
            x: 250.0,
            y: y + 20.0,
        },
        Point {
            x: 300.0,
            y: y + 5.0,
        },
        false,
    );

    write_pdf(r, "pdf_slur_below");
}

#[test]
fn test_pdf_slur_dashed() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Dashed slur (editorial tie)
    let y = staff_y + 2.0 * line_sp;
    r.music_char(120.0, y, MusicGlyph::smufl(0xE0A3), 100.0);
    r.music_char(250.0, y, MusicGlyph::smufl(0xE0A3), 100.0);

    r.slur(
        Point {
            x: 120.0,
            y: y - 5.0,
        },
        Point {
            x: 160.0,
            y: y - 18.0,
        },
        Point {
            x: 210.0,
            y: y - 18.0,
        },
        Point {
            x: 250.0,
            y: y - 5.0,
        },
        true, // dashed
    );

    write_pdf(r, "pdf_slur_dashed");
}

// ========== System Furniture Tests ==========

#[test]
fn test_pdf_bracket_and_brace() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let line_sp = 7.0;
    let staff1_y = 72.0;
    let staff2_y = 72.0 + 60.0;
    let staff1_bottom = staff1_y + 4.0 * line_sp;
    let staff2_bottom = staff2_y + 4.0 * line_sp;

    // Two staves
    r.staff(staff1_y, 80.0, 540.0, 5, line_sp);
    r.staff(staff2_y, 80.0, 540.0, 5, line_sp);

    // Bracket on the left
    r.bracket(72.0, staff1_y, staff2_bottom);

    // Brace for piano (further left)
    r.brace(64.0, staff1_y, staff2_bottom);

    // Connector line
    r.connector_line(staff1_y, staff2_bottom, 80.0);

    // Bar lines across both staves
    r.bar_line(staff1_y, staff1_bottom, 80.0, BarLineType::Single, line_sp);
    r.bar_line(staff2_y, staff2_bottom, 80.0, BarLineType::Single, line_sp);
    r.bar_line(
        staff1_y,
        staff1_bottom,
        540.0,
        BarLineType::FinalDouble,
        line_sp,
    );
    r.bar_line(
        staff2_y,
        staff2_bottom,
        540.0,
        BarLineType::FinalDouble,
        line_sp,
    );

    write_pdf(r, "pdf_bracket_brace");
}

// ========== Line Drawing Tests ==========

#[test]
fn test_pdf_line_types() {
    let mut r = PdfRenderer::new(612.0, 792.0);

    // Regular line
    r.line(72.0, 72.0, 540.0, 72.0, 1.0);

    // Vertical thick line
    r.line_vertical_thick(72.0, 100.0, 540.0, 100.0, 4.0);

    // Horizontal thick line
    r.line_horizontal_thick(72.0, 128.0, 540.0, 128.0, 4.0);

    // Horizontal dashed line
    r.hdashed_line(72.0, 156.0, 540.0, 1.0, 6.0);

    // Vertical dashed line
    r.vdashed_line(300.0, 180.0, 300.0, 1.0, 4.0);

    // Frame rect
    r.frame_rect(
        &RenderRect {
            x: 72.0,
            y: 200.0,
            width: 468.0,
            height: 40.0,
        },
        1.0,
    );

    write_pdf(r, "pdf_line_types");
}

// ========== Color & State Tests ==========

#[test]
fn test_pdf_colors() {
    let mut r = PdfRenderer::new(612.0, 792.0);

    // Red staff
    r.set_color(Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    r.staff(72.0, 72.0, 540.0, 5, 7.0);

    // Green staff
    r.set_color(Color {
        r: 0.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    });
    r.staff(132.0, 72.0, 540.0, 5, 7.0);

    // Blue staff
    r.set_color(Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    });
    r.staff(192.0, 72.0, 540.0, 5, 7.0);

    // Back to black
    r.set_color(Color::BLACK);
    r.staff(252.0, 72.0, 540.0, 5, 7.0);

    write_pdf(r, "pdf_colors");
}

#[test]
fn test_pdf_save_restore_state() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    // Draw in black
    r.set_color(Color::BLACK);
    r.staff(72.0, 72.0, 540.0, 5, 7.0);

    // Save state, draw red, then restore
    r.save_state();
    r.set_color(Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    r.staff(132.0, 72.0, 540.0, 5, 7.0);
    r.restore_state();

    // Should be back to black
    r.staff(192.0, 72.0, 540.0, 5, 7.0);

    write_pdf(r, "pdf_save_restore");
}

#[test]
fn test_pdf_translate() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    // Draw staff at origin-relative position
    r.staff(72.0, 72.0, 400.0, 5, 7.0);

    // Translate and draw another (should appear shifted)
    r.save_state();
    r.translate(100.0, 60.0);
    r.staff(72.0, 72.0, 400.0, 5, 7.0);
    r.restore_state();

    write_pdf(r, "pdf_translate");
}

// ========== Multi-page Test ==========

#[test]
fn test_pdf_multipage() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let line_sp = 7.0;

    // Page 1: treble staff with notes
    r.staff(72.0, 72.0, 540.0, 5, line_sp);
    r.bar_line(
        72.0,
        72.0 + 4.0 * line_sp,
        72.0,
        BarLineType::Single,
        line_sp,
    );
    r.music_char(
        150.0,
        72.0 + 3.0 * line_sp,
        MusicGlyph::smufl(0xE0A3),
        100.0,
    );
    r.music_char(
        250.0,
        72.0 + 2.0 * line_sp,
        MusicGlyph::smufl(0xE0A3),
        100.0,
    );
    r.bar_line(
        72.0,
        72.0 + 4.0 * line_sp,
        540.0,
        BarLineType::Single,
        line_sp,
    );

    // Page 2
    r.begin_page(2);
    r.staff(72.0, 72.0, 540.0, 5, line_sp);
    r.bar_line(
        72.0,
        72.0 + 4.0 * line_sp,
        72.0,
        BarLineType::Single,
        line_sp,
    );
    r.music_char(
        150.0,
        72.0 + 1.0 * line_sp,
        MusicGlyph::smufl(0xE0A3),
        100.0,
    );
    r.music_char(
        250.0,
        72.0 + 4.0 * line_sp,
        MusicGlyph::smufl(0xE0A3),
        100.0,
    );
    r.bar_line(
        72.0,
        72.0 + 4.0 * line_sp,
        540.0,
        BarLineType::FinalDouble,
        line_sp,
    );

    write_pdf(r, "pdf_multipage");
}

// ========== Comprehensive Score Layout ==========

#[test]
fn test_pdf_complete_measure() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;
    let bottom = staff_y + 4.0 * line_sp;

    // Staff
    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Opening bar line
    r.bar_line(staff_y, bottom, 72.0, BarLineType::Single, line_sp);

    // Four quarter notes (C5, D5, E5, F5 — ascending from middle line)
    let notes = [
        (140.0, staff_y + 2.0 * line_sp), // B (middle line)
        (220.0, staff_y + 1.5 * line_sp), // C (space above middle)
        (300.0, staff_y + 1.0 * line_sp), // D (4th line)
        (380.0, staff_y + 0.5 * line_sp), // E (space above 4th)
    ];

    for (x, y) in notes {
        r.music_char(x, y, MusicGlyph::smufl(0xE0A3), 100.0);
        r.note_stem(x + 3.0, y, y - 25.0, 0.8); // Stems up
    }

    // Beam the first pair
    r.beam(
        143.0,
        notes[0].1 - 25.0,
        223.0,
        notes[1].1 - 25.0,
        3.5,
        true,
        true,
    );

    // Slur over the last pair
    r.slur(
        Point {
            x: 300.0,
            y: notes[2].1 - 5.0,
        },
        Point {
            x: 320.0,
            y: notes[2].1 - 18.0,
        },
        Point {
            x: 360.0,
            y: notes[3].1 - 18.0,
        },
        Point {
            x: 380.0,
            y: notes[3].1 - 5.0,
        },
        false,
    );

    // Final bar line
    r.bar_line(staff_y, bottom, 460.0, BarLineType::FinalDouble, line_sp);

    write_pdf(r, "pdf_complete_measure");
}

// ========== Repeat Dots Test ==========

#[test]
fn test_pdf_repeat_dots() {
    let mut r = PdfRenderer::new(612.0, 792.0);
    r.set_widths(0.5, 0.64, 0.8, 1.0);

    let staff_y = 72.0;
    let line_sp = 7.0;
    let bottom = staff_y + 4.0 * line_sp;

    r.staff(staff_y, 72.0, 540.0, 5, line_sp);

    // Repeat dots at the right side
    r.repeat_dots(staff_y, bottom, 520.0);

    // Repeat bar line + dots
    r.bar_line(staff_y, bottom, 530.0, BarLineType::RepeatRight, line_sp);

    write_pdf(r, "pdf_repeat_dots");
}
