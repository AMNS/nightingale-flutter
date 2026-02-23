// PdfRenderer — PDF output via pdf-writer
//
// This is the primary rendering backend for Nightingale. It maps almost 1:1
// from the original PS_Stdio.cp PostScript primitives to PDF content stream
// operators via the pdf-writer crate.
//
// PostScript → PDF operator mapping (the reason this backend exists):
//   moveto      → m   (Content::move_to)
//   lineto      → l   (Content::line_to)
//   curveto     → c   (Content::cubic_to)
//   stroke      → S   (Content::stroke)
//   fill        → f   (Content::fill_nonzero)
//   setlinewidth → w  (Content::set_line_width)
//   setrgbcolor → RG  (Content::set_stroke_rgb / set_fill_rgb)
//   gsave       → q   (Content::save_state)
//   grestore    → Q   (Content::restore_state)
//   setdash     → d   (Content::set_dash_pattern)
//   closepath   → h   (Content::close_path)
//   concat      → cm  (Content::transform)
//
// Reference: PS_Stdio.cp (2,388 lines), PDF Reference 1.7 §8-9

use std::mem;

use pdf_writer::{Content, Pdf, Rect, Ref, Str};

use super::types::{BarLineType, Color, MusicGlyph, Point, RenderRect, TextFont};
use super::MusicRenderer;

/// Graphics state for save/restore stack
#[derive(Clone, Debug)]
struct GraphicsState {
    stroke_color: Color,
    fill_color: Color,
    line_width: f32,
    /// Cumulative translation (simplified affine: translate only for now)
    translate_x: f32,
    translate_y: f32,
    scale_x: f32,
    scale_y: f32,
    /// Line widths set by set_widths()
    staff_line_width: f32,
    ledger_line_width: f32,
    stem_width: f32,
    bar_line_width: f32,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            stroke_color: Color::BLACK,
            fill_color: Color::BLACK,
            line_width: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            staff_line_width: 0.5,   // PS_Stdio.cp default
            ledger_line_width: 0.64, // PS_Stdio.cp default
            stem_width: 0.8,         // PS_Stdio.cp default
            bar_line_width: 1.0,     // PS_Stdio.cp default
        }
    }
}

/// PDF rendering backend implementing the MusicRenderer trait.
///
/// Accumulates drawing commands into a PDF content stream. When `finish()` is called,
/// produces a complete single-page PDF document.
///
/// # Coordinate System
///
/// PDF uses a bottom-left origin with Y increasing upward. Nightingale (and QuickDraw)
/// uses top-left origin with Y increasing downward. The renderer handles this by
/// flipping the Y axis: `pdf_y = page_height - input_y`.
///
/// All input coordinates are in points (1/72 inch), matching both PostScript and PDF
/// native units. No scaling is needed — coordinates pass through directly.
///
/// # Example
///
/// ```ignore
/// use nightingale_core::render::{PdfRenderer, MusicRenderer};
///
/// let mut r = PdfRenderer::new(612.0, 792.0); // US Letter
/// r.staff(100.0, 72.0, 540.0, 5, 10.0);
/// r.bar_line(100.0, 140.0, 72.0, BarLineType::Single);
/// let pdf_bytes = r.finish();
/// std::fs::write("output.pdf", pdf_bytes).unwrap();
/// ```
pub struct PdfRenderer {
    /// Accumulated PDF content stream operations
    content: Content,
    /// Page dimensions in points
    page_width: f32,
    page_height: f32,
    /// Current graphics state
    state: GraphicsState,
    /// State stack for save/restore
    state_stack: Vec<GraphicsState>,
    /// Music font size (points)
    music_size: f32,
    /// Whether we're inside a page
    in_page: bool,
    /// Multiple pages' content streams (each page is a separate Content)
    pages: Vec<Vec<u8>>,
}

impl PdfRenderer {
    /// Create a new PDF renderer with the given page dimensions (in points).
    ///
    /// Standard sizes: US Letter = 612×792, A4 = 595×842
    pub fn new(page_width: f32, page_height: f32) -> Self {
        let mut content = Content::new();

        // White background fill (before Y-flip so we use native PDF coords)
        content.save_state();
        content.set_fill_rgb(1.0, 1.0, 1.0);
        content.rect(0.0, 0.0, page_width, page_height);
        content.fill_nonzero();
        content.restore_state();

        // Set initial graphics state
        content.save_state();
        // Set up Y-flip transform: translate to top-left, flip Y
        // This gives us a top-left origin with Y increasing downward,
        // matching QuickDraw/Nightingale conventions.
        // PDF transform matrix: [sx 0 0 sy tx ty]
        // To flip Y: [1 0 0 -1 0 page_height]
        content.transform([1.0, 0.0, 0.0, -1.0, 0.0, page_height]);

        Self {
            content,
            page_width,
            page_height,
            state: GraphicsState::default(),
            state_stack: Vec::new(),
            music_size: 24.0, // Default music font size
            in_page: true,
            pages: Vec::new(),
        }
    }

    /// Finish the PDF document and return the raw PDF bytes.
    pub fn finish(mut self) -> Vec<u8> {
        // Close the initial save_state
        self.content.restore_state();
        // Store the final page
        self.pages.push(self.content.finish().to_vec());

        // Build the PDF document
        let mut pdf = Pdf::new();

        // Allocate refs: catalog, page_tree, then pairs of (page, content_stream) per page
        let catalog_id = Ref::new(1);
        let page_tree_id = Ref::new(2);
        let first_page_ref = 3;

        // Catalog
        pdf.catalog(catalog_id).pages(page_tree_id);

        // Collect page refs
        let page_refs: Vec<Ref> = (0..self.pages.len())
            .map(|i| Ref::new(first_page_ref + i as i32 * 2))
            .collect();

        // Page tree
        pdf.pages(page_tree_id)
            .kids(page_refs.iter().copied())
            .count(self.pages.len() as i32);

        // Each page + content stream
        for (i, page_content) in self.pages.iter().enumerate() {
            let page_id = Ref::new(first_page_ref + i as i32 * 2);
            let content_id = Ref::new(first_page_ref + i as i32 * 2 + 1);

            pdf.page(page_id)
                .parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, self.page_width, self.page_height))
                .contents(content_id);

            pdf.stream(content_id, page_content);
        }

        pdf.finish()
    }

    /// Apply translation to raw coordinates
    #[inline]
    fn tx(&self, x: f32) -> f32 {
        (x + self.state.translate_x) * self.state.scale_x
    }

    /// Apply translation to raw coordinates
    #[inline]
    fn ty(&self, y: f32) -> f32 {
        (y + self.state.translate_y) * self.state.scale_y
    }

    /// Sync the current stroke color to the PDF content stream
    fn sync_stroke_color(&mut self) {
        let c = self.state.stroke_color;
        self.content.set_stroke_rgb(c.r, c.g, c.b);
    }

    /// Sync the current fill color to the PDF content stream
    fn sync_fill_color(&mut self) {
        let c = self.state.fill_color;
        self.content.set_fill_rgb(c.r, c.g, c.b);
    }

    /// Draw a filled rectangle (no stroke) — helper used by many primitives
    #[allow(dead_code)]
    fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.sync_fill_color();
        self.content.rect(
            self.tx(x),
            self.ty(y),
            width * self.state.scale_x,
            height * self.state.scale_y,
        );
        self.content.fill_nonzero();
    }
}

impl MusicRenderer for PdfRenderer {
    // ================== Line Drawing (6 methods) ==================

    /// Draw a line with perpendicular thickening.
    /// Reference: PS_Stdio.cp, PS_Line(), line 1351
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        self.sync_stroke_color();
        self.content.set_line_width(width * self.state.scale_x);
        self.content
            .set_line_cap(pdf_writer::types::LineCapStyle::RoundCap);
        self.content.move_to(self.tx(x0), self.ty(y0));
        self.content.line_to(self.tx(x1), self.ty(y1));
        self.content.stroke();
    }

    /// Draw a line with vertical thickening (for beams).
    /// The width extends vertically regardless of line angle.
    /// Implemented as a filled parallelogram.
    /// Reference: PS_Stdio.cp, PS_LineVT(), line 1358
    fn line_vertical_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        let half_w = width / 2.0;
        self.sync_fill_color();
        // Four corners of parallelogram with vertical thickening
        self.content.move_to(self.tx(x0), self.ty(y0 - half_w));
        self.content.line_to(self.tx(x1), self.ty(y1 - half_w));
        self.content.line_to(self.tx(x1), self.ty(y1 + half_w));
        self.content.line_to(self.tx(x0), self.ty(y0 + half_w));
        self.content.close_path();
        self.content.fill_nonzero();
    }

    /// Draw a line with horizontal thickening.
    /// Reference: PS_Stdio.cp, PS_LineHT(), line 1367
    fn line_horizontal_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        let half_w = width / 2.0;
        self.sync_fill_color();
        self.content.move_to(self.tx(x0 - half_w), self.ty(y0));
        self.content.line_to(self.tx(x1 - half_w), self.ty(y1));
        self.content.line_to(self.tx(x1 + half_w), self.ty(y1));
        self.content.line_to(self.tx(x0 + half_w), self.ty(y0));
        self.content.close_path();
        self.content.fill_nonzero();
    }

    /// Draw a horizontal dashed line.
    /// Reference: PS_Stdio.cp, PS_HDashedLine(), line 1387
    fn hdashed_line(&mut self, x0: f32, y: f32, x1: f32, width: f32, dash_len: f32) {
        self.sync_stroke_color();
        self.content.set_line_width(width * self.state.scale_x);
        let scaled_dash = dash_len * self.state.scale_x;
        self.content
            .set_dash_pattern([scaled_dash, scaled_dash], 0.0);
        self.content.move_to(self.tx(x0), self.ty(y));
        self.content.line_to(self.tx(x1), self.ty(y));
        self.content.stroke();
        // Reset dash pattern to solid
        self.content.set_dash_pattern([], 0.0);
    }

    /// Draw a vertical dashed line.
    /// Reference: PS_Stdio.cp, PS_VDashedLine(), line 1404
    fn vdashed_line(&mut self, x: f32, y0: f32, y1: f32, width: f32, dash_len: f32) {
        self.sync_stroke_color();
        self.content.set_line_width(width * self.state.scale_x);
        let scaled_dash = dash_len * self.state.scale_x;
        self.content
            .set_dash_pattern([scaled_dash, scaled_dash], 0.0);
        self.content.move_to(self.tx(x), self.ty(y0));
        self.content.line_to(self.tx(x), self.ty(y1));
        self.content.stroke();
        self.content.set_dash_pattern([], 0.0);
    }

    /// Draw a rectangle outline.
    /// Reference: PS_Stdio.cp, PS_FrameRect(), line 1422
    fn frame_rect(&mut self, rect: &RenderRect, width: f32) {
        self.sync_stroke_color();
        self.content.set_line_width(width * self.state.scale_x);
        self.content.rect(
            self.tx(rect.x),
            self.ty(rect.y),
            rect.width * self.state.scale_x,
            rect.height * self.state.scale_y,
        );
        self.content.stroke();
    }

    // ================== Staff & Bars (6 methods) ==================

    /// Draw a single staff line.
    /// Reference: PS_Stdio.cp, PS_StaffLine(), line 1437
    fn staff_line(&mut self, height_y: f32, x0: f32, x1: f32) {
        self.sync_stroke_color();
        self.content
            .set_line_width(self.state.staff_line_width * self.state.scale_x);
        self.content
            .set_line_cap(pdf_writer::types::LineCapStyle::ButtCap);
        self.content.move_to(self.tx(x0), self.ty(height_y));
        self.content.line_to(self.tx(x1), self.ty(height_y));
        self.content.stroke();
    }

    /// Draw a complete N-line staff.
    /// Reference: PS_Stdio.cp, PS_Staff(), line 1454
    fn staff(&mut self, height_y: f32, x0: f32, x1: f32, n_lines: u8, line_spacing: f32) {
        for i in 0..n_lines {
            let y = height_y + (i as f32) * line_spacing;
            self.staff_line(y, x0, x1);
        }
    }

    /// Draw a bar line.
    /// Reference: PS_Stdio.cp, PS_BarLine(), line 1473
    fn bar_line(&mut self, top_y: f32, bottom_y: f32, x: f32, bar_type: BarLineType) {
        self.sync_stroke_color();
        match bar_type {
            BarLineType::Single => {
                self.content
                    .set_line_width(self.state.bar_line_width * self.state.scale_x);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
            }
            BarLineType::Double => {
                // Two thin lines, 3pt apart (PS_Stdio.cp convention)
                let spacing = 3.0;
                self.content
                    .set_line_width(self.state.bar_line_width * self.state.scale_x);
                self.content
                    .move_to(self.tx(x - spacing / 2.0), self.ty(top_y));
                self.content
                    .line_to(self.tx(x - spacing / 2.0), self.ty(bottom_y));
                self.content.stroke();
                self.content
                    .move_to(self.tx(x + spacing / 2.0), self.ty(top_y));
                self.content
                    .line_to(self.tx(x + spacing / 2.0), self.ty(bottom_y));
                self.content.stroke();
            }
            BarLineType::FinalDouble => {
                // Thin line + thick line (PS_Stdio.cp PS_BarLine final variant)
                let spacing = 3.0;
                self.content
                    .set_line_width(self.state.bar_line_width * self.state.scale_x);
                self.content.move_to(self.tx(x - spacing), self.ty(top_y));
                self.content
                    .line_to(self.tx(x - spacing), self.ty(bottom_y));
                self.content.stroke();
                // Thick bar: 3× normal width
                self.content
                    .set_line_width(self.state.bar_line_width * 3.0 * self.state.scale_x);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
            }
            BarLineType::RepeatLeft | BarLineType::RepeatRight | BarLineType::RepeatBoth => {
                // Thick + thin + dots (simplified: just double for now)
                // TODO: Add proper repeat bar with dots via repeat_dots()
                self.content
                    .set_line_width(self.state.bar_line_width * self.state.scale_x);
                self.content.move_to(self.tx(x - 1.5), self.ty(top_y));
                self.content.line_to(self.tx(x - 1.5), self.ty(bottom_y));
                self.content.stroke();
                self.content.move_to(self.tx(x + 1.5), self.ty(top_y));
                self.content.line_to(self.tx(x + 1.5), self.ty(bottom_y));
                self.content.stroke();
            }
        }
    }

    /// Draw a system connector line.
    /// Reference: PS_Stdio.cp, PS_ConLine(), line 1504
    fn connector_line(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        self.sync_stroke_color();
        self.content
            .set_line_width(self.state.bar_line_width * self.state.scale_x);
        self.content.move_to(self.tx(x), self.ty(top_y));
        self.content.line_to(self.tx(x), self.ty(bottom_y));
        self.content.stroke();
    }

    /// Draw a ledger line.
    /// Reference: PS_Stdio.cp, PS_LedgerLine(), line 1610
    fn ledger_line(&mut self, height_y: f32, x_center: f32, half_width: f32) {
        self.sync_stroke_color();
        self.content
            .set_line_width(self.state.ledger_line_width * self.state.scale_x);
        self.content
            .set_line_cap(pdf_writer::types::LineCapStyle::ButtCap);
        self.content
            .move_to(self.tx(x_center - half_width), self.ty(height_y));
        self.content
            .line_to(self.tx(x_center + half_width), self.ty(height_y));
        self.content.stroke();
    }

    /// Draw repeat dots.
    /// Reference: PS_Stdio.cp, PS_Repeat(), line 1521
    fn repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        // Two dots placed at 1/3 and 2/3 of the staff height
        let third = (bottom_y - top_y) / 3.0;
        let dot_radius = 1.5; // points

        self.sync_fill_color();
        // Upper dot — approximate circle with 4 Bezier curves
        let cy1 = top_y + third;
        self.draw_circle(x, cy1, dot_radius);
        // Lower dot
        let cy2 = top_y + 2.0 * third;
        self.draw_circle(x, cy2, dot_radius);
    }

    // ================== Musical Elements (5 methods) ==================

    /// Draw a beam segment.
    /// Reference: PS_Stdio.cp, PS_Beam(), line 1625
    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, up0: bool, up1: bool) {
        // PS_Stdio.cp draws beams as filled parallelograms with vertical thickening.
        // The beam extends downward from the given Y coordinates when stems are up,
        // and upward when stems are down.
        let offset0 = if up0 { thickness } else { 0.0 };
        let offset1 = if up1 { thickness } else { 0.0 };

        self.sync_fill_color();
        self.content
            .move_to(self.tx(x0), self.ty(y0 - offset0 + thickness));
        self.content
            .line_to(self.tx(x1), self.ty(y1 - offset1 + thickness));
        self.content.line_to(self.tx(x1), self.ty(y1 - offset1));
        self.content.line_to(self.tx(x0), self.ty(y0 - offset0));
        self.content.close_path();
        self.content.fill_nonzero();
    }

    /// Draw a slur or tie as a cubic Bezier curve.
    /// Reference: PS_Stdio.cp, PS_Slur(), line 1933
    fn slur(&mut self, p0: Point, c1: Point, c2: Point, p3: Point, dashed: bool) {
        self.sync_stroke_color();
        self.content.set_line_width(1.0 * self.state.scale_x);
        self.content
            .set_line_cap(pdf_writer::types::LineCapStyle::RoundCap);

        if dashed {
            let d = 3.0 * self.state.scale_x;
            self.content.set_dash_pattern([d, d], 0.0);
        }

        self.content.move_to(self.tx(p0.x), self.ty(p0.y));
        self.content.cubic_to(
            self.tx(c1.x),
            self.ty(c1.y),
            self.tx(c2.x),
            self.ty(c2.y),
            self.tx(p3.x),
            self.ty(p3.y),
        );
        self.content.stroke();

        if dashed {
            self.content.set_dash_pattern([], 0.0);
        }
    }

    /// Draw a system bracket.
    /// Reference: PS_Stdio.cp, PS_Bracket(), line 1966
    fn bracket(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        // Simplified bracket: vertical line with serifs
        self.sync_stroke_color();
        self.content.set_line_width(2.0 * self.state.scale_x);
        self.content.move_to(self.tx(x), self.ty(y_top));
        self.content.line_to(self.tx(x), self.ty(y_bottom));
        self.content.stroke();

        // Top serif
        self.content.set_line_width(1.5 * self.state.scale_x);
        self.content.move_to(self.tx(x), self.ty(y_top));
        self.content.line_to(self.tx(x + 4.0), self.ty(y_top));
        self.content.stroke();

        // Bottom serif
        self.content.move_to(self.tx(x), self.ty(y_bottom));
        self.content.line_to(self.tx(x + 4.0), self.ty(y_bottom));
        self.content.stroke();
    }

    /// Draw a system brace.
    /// Reference: PS_Stdio.cp, PS_Brace(), line 1980
    fn brace(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        // Brace drawn as two cubic Bezier curves meeting at the midpoint
        let mid_y = (y_top + y_bottom) / 2.0;
        let curve_depth = 6.0; // How far left the brace extends

        self.sync_stroke_color();
        self.content.set_line_width(1.5 * self.state.scale_x);

        // Upper half: top to midpoint
        self.content.move_to(self.tx(x), self.ty(y_top));
        self.content.cubic_to(
            self.tx(x - curve_depth),
            self.ty(y_top + (mid_y - y_top) * 0.3),
            self.tx(x - curve_depth),
            self.ty(mid_y - (mid_y - y_top) * 0.3),
            self.tx(x - curve_depth * 1.5),
            self.ty(mid_y),
        );
        self.content.stroke();

        // Lower half: midpoint to bottom
        self.content
            .move_to(self.tx(x - curve_depth * 1.5), self.ty(mid_y));
        self.content.cubic_to(
            self.tx(x - curve_depth),
            self.ty(mid_y + (y_bottom - mid_y) * 0.3),
            self.tx(x - curve_depth),
            self.ty(y_bottom - (y_bottom - mid_y) * 0.3),
            self.tx(x),
            self.ty(y_bottom),
        );
        self.content.stroke();
    }

    /// Draw a note stem.
    /// Reference: PS_Stdio.cp, PS_NoteStem(), line 1657
    fn note_stem(&mut self, x: f32, y_top: f32, y_bottom: f32, width: f32) {
        self.sync_stroke_color();
        self.content.set_line_width(width * self.state.scale_x);
        self.content
            .set_line_cap(pdf_writer::types::LineCapStyle::ButtCap);
        self.content.move_to(self.tx(x), self.ty(y_top));
        self.content.line_to(self.tx(x), self.ty(y_bottom));
        self.content.stroke();
    }

    // ================== Characters & Text (4 methods) ==================

    /// Draw a music character (glyph).
    ///
    /// Currently renders a placeholder rectangle. Will use real SMuFL/Bravura
    /// glyphs once font embedding is implemented.
    ///
    /// Reference: PS_Stdio.cp, PS_MusChar(), line 1834
    fn music_char(&mut self, x: f32, y: f32, _glyph: MusicGlyph, _size_percent: f32) {
        // Placeholder: draw a small filled square to mark glyph position
        let half_size = 3.0;
        self.sync_fill_color();
        self.content.rect(
            self.tx(x - half_size),
            self.ty(y - half_size),
            half_size * 2.0 * self.state.scale_x,
            half_size * 2.0 * self.state.scale_y,
        );
        self.content.fill_nonzero();
    }

    /// Draw a string of music characters.
    /// Reference: PS_Stdio.cp, PS_MusString(), line 1897
    fn music_string(&mut self, x: f32, y: f32, glyphs: &[MusicGlyph], size_percent: f32) {
        // Draw each glyph with horizontal spacing
        let spacing = self.music_size * 0.6; // Approximate glyph advance
        for (i, glyph) in glyphs.iter().enumerate() {
            self.music_char(x + i as f32 * spacing, y, *glyph, size_percent);
        }
    }

    /// Draw a text string.
    /// Reference: PS_Stdio.cp, PS_FontString(), line 1855
    fn text_string(&mut self, x: f32, y: f32, text: &str, _font: &TextFont) {
        // PDF text rendering requires a font resource. For now, use the built-in
        // Helvetica font which is available in all PDF readers without embedding.
        self.sync_fill_color();
        self.content.begin_text();
        self.content.set_font(pdf_writer::Name(b"F1"), 12.0);
        self.content.next_line(self.tx(x), self.ty(y));
        self.content.show(Str(text.as_bytes()));
        self.content.end_text();
    }

    /// Draw a music colon (two stacked dots for repeat signs).
    /// Reference: PS_Stdio.cp, PS_MusColon(), line 1909
    fn music_colon(&mut self, x: f32, y: f32, _size_percent: f32, line_space: f32) {
        let dot_radius = 1.2;
        self.sync_fill_color();
        // Upper dot
        self.draw_circle(x, y - line_space * 0.5, dot_radius);
        // Lower dot
        self.draw_circle(x, y + line_space * 0.5, dot_radius);
    }

    // ================== Configuration (4 methods) ==================

    /// Set the default line width.
    /// Reference: PS_Stdio.cp, PS_SetLineWidth(), line 1335
    fn set_line_width(&mut self, width: f32) {
        self.state.line_width = width;
    }

    /// Set specific line widths for different musical elements.
    /// Reference: PS_Stdio.cp, PS_SetWidths(), line 1323
    fn set_widths(&mut self, staff: f32, ledger: f32, stem: f32, bar: f32) {
        self.state.staff_line_width = staff;
        self.state.ledger_line_width = ledger;
        self.state.stem_width = stem;
        self.state.bar_line_width = bar;
    }

    /// Set the music font size.
    /// Reference: PS_Stdio.cp, PS_MusSize(), line 1775
    fn set_music_size(&mut self, point_size: f32) {
        self.music_size = point_size;
    }

    /// Set the page size.
    /// Reference: PS_Stdio.cp, PS_PageSize(), line 1312
    fn set_page_size(&mut self, width: f32, height: f32) {
        self.page_width = width;
        self.page_height = height;
    }

    // ================== Page Management (2 methods) ==================

    /// Begin a new page.
    /// Reference: PS_Stdio.cp, PS_NewPage(), line 1016
    fn begin_page(&mut self, _page_num: u32) {
        if self.in_page {
            // End current page first
            self.content.restore_state();
            let old_content = mem::replace(&mut self.content, Content::new());
            self.pages.push(old_content.finish().to_vec());
        }
        // Start new page with white background
        self.content = Content::new();
        self.content.save_state();
        self.content.set_fill_rgb(1.0, 1.0, 1.0);
        self.content
            .rect(0.0, 0.0, self.page_width, self.page_height);
        self.content.fill_nonzero();
        self.content.restore_state();

        self.content.save_state();
        self.content
            .transform([1.0, 0.0, 0.0, -1.0, 0.0, self.page_height]);
        self.in_page = true;
    }

    /// End the current page.
    /// Reference: PS_Stdio.cp, PS_EndPage(), line 1036
    fn end_page(&mut self) {
        if self.in_page {
            self.content.restore_state();
            let old_content = mem::replace(&mut self.content, Content::new());
            self.pages.push(old_content.finish().to_vec());
            self.in_page = false;
        }
    }

    // ================== State Management (5 methods) ==================

    /// Push current graphics state.
    fn save_state(&mut self) {
        self.content.save_state();
        self.state_stack.push(self.state.clone());
    }

    /// Pop graphics state.
    fn restore_state(&mut self) {
        self.content.restore_state();
        if let Some(state) = self.state_stack.pop() {
            self.state = state;
        }
    }

    /// Apply translation.
    fn translate(&mut self, dx: f32, dy: f32) {
        self.state.translate_x += dx;
        self.state.translate_y += dy;
        // Note: We apply translation manually via tx()/ty() rather than
        // using PDF's cm operator, to keep coordinate tracking simple.
    }

    /// Apply scale.
    fn scale(&mut self, sx: f32, sy: f32) {
        self.state.scale_x *= sx;
        self.state.scale_y *= sy;
    }

    /// Set drawing color.
    fn set_color(&mut self, color: Color) {
        self.state.stroke_color = color;
        self.state.fill_color = color;
    }
}

// Helper methods (not part of MusicRenderer trait)
impl PdfRenderer {
    /// Draw an approximate circle using 4 cubic Bezier curves.
    ///
    /// Uses the standard Bezier circle approximation:
    /// control point offset = radius × 0.5522847498 (4/3 × (√2 − 1))
    fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32) {
        let k = radius * 0.552_284_8;

        let tcx = self.tx(cx);
        let tcy = self.ty(cy);
        let tr = radius * self.state.scale_x;
        let tk = k * self.state.scale_x;

        self.content.move_to(tcx + tr, tcy);
        self.content
            .cubic_to(tcx + tr, tcy + tk, tcx + tk, tcy + tr, tcx, tcy + tr);
        self.content
            .cubic_to(tcx - tk, tcy + tr, tcx - tr, tcy + tk, tcx - tr, tcy);
        self.content
            .cubic_to(tcx - tr, tcy - tk, tcx - tk, tcy - tr, tcx, tcy - tr);
        self.content
            .cubic_to(tcx + tk, tcy - tr, tcx + tr, tcy - tk, tcx + tr, tcy);
        self.content.close_path();
        self.content.fill_nonzero();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_renderer_creates_valid_pdf() {
        let mut r = PdfRenderer::new(612.0, 792.0);
        r.staff(100.0, 72.0, 540.0, 5, 10.0);
        let pdf_bytes = r.finish();

        // Check PDF header
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        // Check PDF has content
        assert!(pdf_bytes.len() > 100);
    }

    #[test]
    fn test_pdf_renderer_line_drawing() {
        let mut r = PdfRenderer::new(200.0, 200.0);
        r.line(10.0, 10.0, 190.0, 10.0, 1.0);
        r.line(10.0, 50.0, 190.0, 50.0, 2.0);
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_pdf_renderer_bar_lines() {
        let mut r = PdfRenderer::new(200.0, 200.0);
        r.bar_line(20.0, 60.0, 50.0, BarLineType::Single);
        r.bar_line(20.0, 60.0, 100.0, BarLineType::Double);
        r.bar_line(20.0, 60.0, 150.0, BarLineType::FinalDouble);
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_pdf_renderer_beam() {
        let mut r = PdfRenderer::new(200.0, 200.0);
        r.beam(20.0, 50.0, 100.0, 45.0, 3.0, true, true);
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_pdf_renderer_slur() {
        let mut r = PdfRenderer::new(200.0, 200.0);
        r.slur(
            Point { x: 20.0, y: 100.0 },
            Point { x: 60.0, y: 70.0 },
            Point { x: 140.0, y: 70.0 },
            Point { x: 180.0, y: 100.0 },
            false,
        );
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_pdf_renderer_state_management() {
        let mut r = PdfRenderer::new(200.0, 200.0);
        r.set_color(Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        r.save_state();
        r.set_color(Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        });
        r.line(10.0, 10.0, 190.0, 10.0, 1.0);
        r.restore_state();
        // Should be back to red
        assert_eq!(r.state.stroke_color.r, 1.0);
        assert_eq!(r.state.stroke_color.g, 0.0);
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_pdf_renderer_multipage() {
        let mut r = PdfRenderer::new(612.0, 792.0);
        r.staff(100.0, 72.0, 540.0, 5, 10.0);
        r.begin_page(2);
        r.staff(100.0, 72.0, 540.0, 5, 10.0);
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        // Should have 2 pages worth of content
        assert!(pdf_bytes.len() > 200);
    }

    #[test]
    fn test_pdf_renderer_full_score() {
        let mut r = PdfRenderer::new(612.0, 792.0);

        // Set standard widths
        r.set_widths(0.5, 0.64, 0.8, 1.0);

        // Draw a staff
        let staff_y = 100.0;
        let line_sp = 7.0; // ~7pt between staff lines (standard for 24pt music)
        r.staff(staff_y, 72.0, 540.0, 5, line_sp);

        // Bar lines
        let bottom_y = staff_y + 4.0 * line_sp;
        r.bar_line(staff_y, bottom_y, 72.0, BarLineType::Single);
        r.bar_line(staff_y, bottom_y, 540.0, BarLineType::FinalDouble);

        // A few notes with stems
        // SMuFL U+E0A4 = noteheadWhole, U+E0A3 = noteheadBlack
        r.music_char(
            120.0,
            staff_y + 2.0 * line_sp,
            MusicGlyph::smufl(0xE0A4),
            100.0,
        );
        r.note_stem(
            200.0,
            staff_y + 4.0 * line_sp,
            staff_y + 4.0 * line_sp - 25.0,
            0.8,
        );
        r.music_char(
            200.0,
            staff_y + 4.0 * line_sp,
            MusicGlyph::smufl(0xE0A3),
            100.0,
        );
        r.note_stem(
            280.0,
            staff_y + 3.0 * line_sp,
            staff_y + 3.0 * line_sp - 25.0,
            0.8,
        );
        r.music_char(
            280.0,
            staff_y + 3.0 * line_sp,
            MusicGlyph::smufl(0xE0A3),
            100.0,
        );

        // Beam
        r.beam(
            200.0,
            staff_y + 4.0 * line_sp - 25.0,
            280.0,
            staff_y + 3.0 * line_sp - 25.0,
            3.0,
            true,
            true,
        );

        // Save PDF
        let pdf_bytes = r.finish();
        let output_dir = "/tmp/nightingale-test-output";
        std::fs::create_dir_all(output_dir).ok();
        std::fs::write(format!("{}/test_score.pdf", output_dir), &pdf_bytes).unwrap();

        assert!(pdf_bytes.starts_with(b"%PDF-"));
        assert!(pdf_bytes.len() > 500);
    }
}
