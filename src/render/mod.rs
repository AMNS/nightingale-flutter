//! Rendering abstraction for Nightingale music notation.
//!
//! This module defines the `MusicRenderer` trait, which provides a platform-agnostic
//! interface for drawing musical notation. The trait mirrors the 27 primitives from
//! the original C++ PS_Stdio.cp file, but using modern Rust types and conventions.
//!
//! # Architecture
//!
//! The rendering system has three layers:
//!
//! 1. **High-level drawing code** (DrawObject.cp, DrawNRGR.cp) — calls MusicRenderer methods
//! 2. **MusicRenderer trait** (this module) — platform-agnostic interface
//! 3. **Backend implementations** (PdfRenderer, FlutterRenderer) — concrete drawing
//!
//! # Coordinate System
//!
//! All coordinates are in **points** (1/72 inch) using f32. The caller converts from
//! DDIST to f32 using `ddist_to_render()` before calling renderer methods.
//!
//! # Reference
//!
//! - Original C++ code: PS_Stdio.cp (2,388 lines)
//! - Design notes: RENDERING_QUICK_REFERENCE.md

pub mod bitmap_renderer;
pub mod command;
pub mod command_renderer;
pub mod pdf_renderer;
pub mod types;

pub use bitmap_renderer::BitmapRenderer;
pub use command::RenderCommand;
pub use command_renderer::CommandRenderer;
pub use pdf_renderer::PdfRenderer;
pub use types::{
    ddist_to_render, render_to_ddist, BarLineType, Color, MusicGlyph, Point, RenderRect, Stroke,
    TextFont,
};

/// Platform-agnostic music rendering trait.
///
/// Implementations of this trait provide concrete drawing backends (Flutter, PDF, etc.).
/// The trait methods map 1:1 to the PS_Stdio.cp PostScript primitives, with cleaner
/// Rust-style signatures.
///
/// # Method Organization
///
/// Methods are grouped by category:
/// - Line drawing (6 methods)
/// - Staff & bars (6 methods)
/// - Musical elements (5 methods)
/// - Characters & text (4 methods)
/// - Configuration (4 methods)
/// - Page management (2 methods)
/// - State management (5 methods)
///
/// # Coordinate Convention
///
/// All coordinates are f32 points. Use `ddist_to_render(ddist)` to convert from DDIST.
///
/// # Example
///
/// ```ignore
/// use nightingale::render::{MusicRenderer, CommandRenderer, ddist_to_render};
///
/// let mut renderer = CommandRenderer::new();
///
/// // Draw a staff line from x=100 to x=500 at y=200 (in DDIST)
/// let y = ddist_to_render(200);
/// let x0 = ddist_to_render(100);
/// let x1 = ddist_to_render(500);
/// renderer.staff_line(y, x0, x1);
/// ```
pub trait MusicRenderer {
    // ================== Line Drawing (6 methods) ==================

    /// Draw a line from (x0, y0) to (x1, y1) with perpendicular thickening.
    ///
    /// Reference: PS_Stdio.cp, PS_Line(), line 1351
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32);

    /// Draw a line with vertical thickening (for beams).
    ///
    /// Unlike `line()`, the width extends vertically regardless of line angle.
    ///
    /// Reference: PS_Stdio.cp, PS_LineVT(), line 1358
    fn line_vertical_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32);

    /// Draw a line with horizontal thickening.
    ///
    /// Unlike `line()`, the width extends horizontally regardless of line angle.
    ///
    /// Reference: PS_Stdio.cp, PS_LineHT(), line 1367
    fn line_horizontal_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32);

    /// Draw a horizontal dashed line.
    ///
    /// Reference: PS_Stdio.cp, PS_HDashedLine(), line 1387
    fn hdashed_line(&mut self, x0: f32, y: f32, x1: f32, width: f32, dash_len: f32);

    /// Draw a vertical dashed line.
    ///
    /// Reference: PS_Stdio.cp, PS_VDashedLine(), line 1404
    fn vdashed_line(&mut self, x: f32, y0: f32, y1: f32, width: f32, dash_len: f32);

    /// Draw a rectangle outline.
    ///
    /// Reference: PS_Stdio.cp, PS_FrameRect(), line 1422
    fn frame_rect(&mut self, rect: &RenderRect, width: f32);

    // ================== Staff & Bars (6 methods) ==================

    /// Draw a single staff line at the given height.
    ///
    /// Reference: PS_Stdio.cp, PS_StaffLine(), line 1437
    fn staff_line(&mut self, height_y: f32, x0: f32, x1: f32);

    /// Draw a complete N-line staff.
    ///
    /// `n_lines` is typically 5 for standard notation.
    /// `line_spacing` is the distance between adjacent staff lines (in points).
    ///
    /// Reference: PS_Stdio.cp, PS_Staff(), line 1454
    fn staff(&mut self, height_y: f32, x0: f32, x1: f32, n_lines: u8, line_spacing: f32);

    /// Draw a bar line.
    ///
    /// `line_space` is the staff interline distance in render coordinates (pt).
    /// Used for double/final/repeat barline proportions (OG: INTERLNSPACE, THICKBARLINE).
    /// Reference: PS_Stdio.cp, PS_BarLine(), line 1473; PS_Repeat(), line 1521
    fn bar_line(
        &mut self,
        top_y: f32,
        bottom_y: f32,
        x: f32,
        bar_type: BarLineType,
        line_space: f32,
    );

    /// Draw a system connector line (vertical line connecting multiple staves).
    ///
    /// Reference: PS_Stdio.cp, PS_ConLine(), line 1504
    fn connector_line(&mut self, top_y: f32, bottom_y: f32, x: f32);

    /// Draw a ledger line centered at (x_center, height_y).
    ///
    /// `half_width` is the distance from center to each end.
    ///
    /// Reference: PS_Stdio.cp, PS_LedgerLine(), line 1610
    fn ledger_line(&mut self, height_y: f32, x_center: f32, half_width: f32);

    /// Draw repeat dots for a repeat bar line.
    ///
    /// Draws two dots vertically centered between top_y and bottom_y.
    ///
    /// Reference: PS_Stdio.cp, PS_Repeat(), line 1521 (dots only variant)
    fn repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32);

    // ================== Musical Elements (5 methods) ==================

    /// Draw a beam segment.
    ///
    /// `thickness` is the beam height.
    /// `up0` and `up1` specify whether stem is up at each end (affects beam placement).
    ///
    /// Reference: PS_Stdio.cp, PS_Beam(), line 1625
    #[allow(clippy::too_many_arguments)]
    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, up0: bool, up1: bool);

    /// Draw a slur or tie as a cubic Bezier curve.
    ///
    /// `p0` and `p3` are endpoints, `c1` and `c2` are control points.
    /// `dashed` creates a dashed slur (for editorial ties).
    ///
    /// Reference: PS_Stdio.cp, PS_Slur(), line 1933
    fn slur(&mut self, p0: Point, c1: Point, c2: Point, p3: Point, dashed: bool);

    /// Draw a system bracket (square bracket on left of grand staff).
    ///
    /// Reference: PS_Stdio.cp, PS_Bracket(), line 1966
    fn bracket(&mut self, x: f32, y_top: f32, y_bottom: f32);

    /// Draw a system brace (curly brace on left of piano grand staff).
    ///
    /// Reference: PS_Stdio.cp, PS_Brace(), line 1980
    fn brace(&mut self, x: f32, y_top: f32, y_bottom: f32);

    /// Draw a note stem.
    ///
    /// Reference: PS_Stdio.cp, PS_NoteStem(), line 1657
    fn note_stem(&mut self, x: f32, y_top: f32, y_bottom: f32, width: f32);

    // ================== Characters & Text (4 methods) ==================

    /// Draw a single music character (glyph).
    ///
    /// `size_percent` scales the glyph (100 = normal size).
    ///
    /// Reference: PS_Stdio.cp, PS_MusChar(), line 1834
    fn music_char(&mut self, x: f32, y: f32, glyph: MusicGlyph, size_percent: f32);

    /// Draw a string of music characters.
    ///
    /// Reference: PS_Stdio.cp, PS_MusString(), line 1897
    fn music_string(&mut self, x: f32, y: f32, glyphs: &[MusicGlyph], size_percent: f32);

    /// Draw a text string (non-music font).
    ///
    /// Reference: PS_Stdio.cp, PS_FontString(), line 1855
    fn text_string(&mut self, x: f32, y: f32, text: &str, font: &TextFont);

    /// Draw a music colon (two vertically stacked dots).
    ///
    /// Used for repeat signs and other notation.
    /// `line_space` is the distance between staff lines.
    ///
    /// Reference: PS_Stdio.cp, PS_MusColon(), line 1909
    fn music_colon(&mut self, x: f32, y: f32, size_percent: f32, line_space: f32);

    // ================== Configuration (4 methods) ==================

    /// Set the default line width for subsequent drawing operations.
    ///
    /// Reference: PS_Stdio.cp, PS_SetLineWidth(), line 1335
    fn set_line_width(&mut self, width: f32);

    /// Set various line widths: staff lines, ledger lines, stems, and bar lines.
    ///
    /// Reference: PS_Stdio.cp, PS_SetWidths(), line 1323
    fn set_widths(&mut self, staff: f32, ledger: f32, stem: f32, bar: f32);

    /// Set the music font size (in points).
    ///
    /// Reference: PS_Stdio.cp, PS_MusSize(), line 1775
    fn set_music_size(&mut self, point_size: f32);

    /// Set the page size (in points).
    ///
    /// Reference: PS_Stdio.cp, PS_PageSize(), line 1312
    fn set_page_size(&mut self, width: f32, height: f32);

    // ================== Page Management (2 methods) ==================

    /// Begin a new page.
    ///
    /// Reference: PS_Stdio.cp, PS_NewPage(), line 1016
    fn begin_page(&mut self, page_num: u32);

    /// End the current page.
    ///
    /// Reference: PS_Stdio.cp, PS_EndPage(), line 1036
    fn end_page(&mut self);

    // ================== State Management (5 methods) ==================
    // These methods are additions not present in PS_Stdio.cp, but necessary
    // for modern rendering backends (Canvas2D, Skia, etc.).

    /// Push the current graphics state onto a stack.
    ///
    /// Saves color, line width, transformation matrix, etc.
    fn save_state(&mut self);

    /// Pop the most recent graphics state from the stack.
    ///
    /// Restores color, line width, transformation matrix, etc.
    fn restore_state(&mut self);

    /// Apply a translation to the current transformation matrix.
    fn translate(&mut self, dx: f32, dy: f32);

    /// Apply a scale to the current transformation matrix.
    fn scale(&mut self, sx: f32, sy: f32);

    /// Set the current drawing color.
    fn set_color(&mut self, color: Color);

    // ================== Text Measurement (optional) ==================

    /// Measure the width of a text string in points.
    ///
    /// Returns `None` if the renderer does not support text measurement
    /// (e.g., CommandRenderer has no font loaded). Renderers with font access
    /// (BitmapRenderer) should override this.
    fn measure_text_width(&self, _text: &str, _font: &TextFont) -> Option<f32> {
        None
    }
}
