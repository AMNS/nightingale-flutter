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
// Font embedding:
//   SMuFL glyphs are rendered using Bravura.otf (OpenType/CFF), embedded as a
//   Type0 composite font with Identity-H encoding. Each music_char() call maps
//   a SMuFL codepoint (U+E000-U+F8FF) to a glyph ID via ttf-parser, then emits
//   the glyph as a 2-byte big-endian CID in a BT/ET text block.
//
// Reference: PS_Stdio.cp (2,388 lines), PDF Reference 1.7 §8-9

use std::collections::BTreeMap;
use std::mem;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

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
/// Embedded font data parsed from an OpenType/TrueType file.
///
/// Holds the raw font bytes and the codepoint-to-glyph-ID mapping needed for
/// PDF text rendering. The font is embedded once in finish() as a Type0
/// composite font.
struct EmbeddedFont {
    /// Raw .otf/.ttf file bytes (embedded verbatim in the PDF)
    data: Vec<u8>,
    /// Map from Unicode codepoint to glyph ID (u16)
    cmap: BTreeMap<u32, u16>,
    /// Map from glyph ID to Unicode codepoint (for ToUnicode)
    #[allow(dead_code)]
    gid_to_unicode: BTreeMap<u16, u32>,
    /// Glyph widths in PDF font units (1000 units/em)
    widths: Vec<f32>,
    /// Font metrics
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    bbox: (i16, i16, i16, i16), // xMin, yMin, xMax, yMax
    /// Whether this is a CFF font (OTTO) vs TrueType
    is_cff: bool,
}

impl EmbeddedFont {
    /// Load and parse a font file, building codepoint→glyph mapping.
    fn load(data: Vec<u8>) -> Option<Self> {
        // Parse font — the face borrows data, so we extract everything we need
        // before moving data into the struct.
        let (cmap, gid_to_unicode, widths, units_per_em, ascender, descender, bbox, is_cff) = {
            let face = ttf_parser::Face::parse(&data, 0).ok()?;
            let units_per_em = face.units_per_em();
            let num_glyphs = face.number_of_glyphs();

            let mut cmap = BTreeMap::new();
            let mut gid_to_unicode = BTreeMap::new();
            let mut widths = vec![0.0f32; num_glyphs as usize];

            // Build codepoint→glyph mapping using Face::glyph_index().
            // We scan the SMuFL Private Use Area (U+E000..U+F8FF) which is where
            // all music notation glyphs live in SMuFL-compatible fonts (Bravura,
            // Leland, Petaluma, etc). This approach is font-agnostic.
            for cp in 0xE000..=0xF8FFu32 {
                if let Some(ch) = char::from_u32(cp) {
                    if let Some(gid) = face.glyph_index(ch) {
                        cmap.insert(cp, gid.0);
                        gid_to_unicode.entry(gid.0).or_insert(cp);
                        let advance = face.glyph_hor_advance(gid).unwrap_or(0);
                        widths[gid.0 as usize] = advance as f32 / units_per_em as f32 * 1000.0;
                    }
                }
            }

            let bbox = face.global_bounding_box();
            let is_cff = data.starts_with(b"OTTO");

            (
                cmap,
                gid_to_unicode,
                widths,
                units_per_em,
                face.ascender(),
                face.descender(),
                (bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max),
                is_cff,
            )
        }; // face dropped here, releasing borrow on data

        Some(Self {
            data,
            cmap,
            gid_to_unicode,
            widths,
            units_per_em,
            ascender,
            descender,
            bbox,
            is_cff,
        })
    }

    /// Convert a value in font design units to PDF font units (1000/em).
    fn to_font_units(&self, v: i16) -> f32 {
        v as f32 / self.units_per_em as f32 * 1000.0
    }

    /// Look up the glyph ID for a Unicode codepoint. Returns None if not in font.
    fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        self.cmap.get(&codepoint).copied()
    }
}

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
    /// Whether any drawing has occurred on the current page.
    /// Used to avoid emitting blank pages when begin_page() is called
    /// before any content is drawn (e.g. NGL files with PAGE objects).
    page_has_content: bool,
    /// Multiple pages' content streams (each page is a separate Content)
    pages: Vec<Vec<u8>>,
    /// Embedded music font (Bravura), if loaded
    music_font: Option<EmbeddedFont>,
    /// Set of glyph IDs actually used (for width table and ToUnicode)
    used_glyphs: BTreeMap<u16, u32>,
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
            page_has_content: false,
            pages: Vec::new(),
            music_font: None,
            used_glyphs: BTreeMap::new(),
        }
    }

    /// Load a SMuFL-compatible music font (e.g. Bravura.otf) for glyph rendering.
    ///
    /// The font is parsed immediately to build a codepoint→glyph mapping.
    /// It will be embedded in the PDF when finish() is called.
    ///
    /// Works with any SMuFL font: Bravura, Leland, Petaluma, etc.
    pub fn load_music_font(&mut self, font_data: Vec<u8>) -> bool {
        if let Some(font) = EmbeddedFont::load(font_data) {
            self.music_font = Some(font);
            true
        } else {
            false
        }
    }

    /// Load a music font from a file path.
    pub fn load_music_font_file(&mut self, path: &std::path::Path) -> bool {
        if let Ok(data) = std::fs::read(path) {
            self.load_music_font(data)
        } else {
            false
        }
    }

    /// Finish the PDF document and return the raw PDF bytes.
    ///
    /// If a music font was loaded, it is embedded as a Type0 composite font
    /// with Identity-H encoding and a ToUnicode CMap for copy/paste support.
    pub fn finish(mut self) -> Vec<u8> {
        // Only save the current page if we're still in_page (i.e., end_page() wasn't called).
        // If end_page() was called, the page content was already saved and in_page=false.
        if self.in_page {
            // Close the initial save_state
            self.content.restore_state();
            // Store the final page
            self.pages.push(self.content.finish().to_vec());
        }

        // Build the PDF document
        let mut pdf = Pdf::new();

        // Allocate refs
        let catalog_id = Ref::new(1);
        let page_tree_id = Ref::new(2);
        // Reserve refs 3..N for pages/content, then font objects after
        let first_page_ref = 3;
        let num_pages = self.pages.len();
        let after_pages = first_page_ref + (num_pages as i32) * 2;

        // Font-related refs (only used if music_font is present)
        let type0_ref = Ref::new(after_pages);
        let cid_ref = Ref::new(after_pages + 1);
        let descriptor_ref = Ref::new(after_pages + 2);
        let tounicode_ref = Ref::new(after_pages + 3);
        let font_data_ref = Ref::new(after_pages + 4);

        let has_font = self.music_font.is_some();

        // Catalog
        pdf.catalog(catalog_id).pages(page_tree_id);

        // Collect page refs
        let page_refs: Vec<Ref> = (0..num_pages)
            .map(|i| Ref::new(first_page_ref + i as i32 * 2))
            .collect();

        // Page tree
        pdf.pages(page_tree_id)
            .kids(page_refs.iter().copied())
            .count(num_pages as i32);

        // Each page + content stream
        for (i, page_content) in self.pages.iter().enumerate() {
            let page_id = Ref::new(first_page_ref + i as i32 * 2);
            let content_id = Ref::new(first_page_ref + i as i32 * 2 + 1);

            let mut page = pdf.page(page_id);
            page.parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, self.page_width, self.page_height))
                .contents(content_id);

            // Add font resource to each page
            if has_font {
                page.resources().fonts().pair(Name(b"Bravura"), type0_ref);
            }
            page.finish();

            pdf.stream(content_id, page_content);
        }

        // Embed the music font if loaded
        if let Some(ref font) = self.music_font {
            let system_info = SystemInfo {
                registry: Str(b"Adobe"),
                ordering: Str(b"Identity"),
                supplement: 0,
            };

            // Type0 font (composite font root)
            pdf.type0_font(type0_ref)
                .base_font(Name(b"Bravura"))
                .encoding_predefined(Name(b"Identity-H"))
                .descendant_font(cid_ref)
                .to_unicode(tounicode_ref);

            // CIDFont (descendant)
            let cid_font_type = if font.is_cff {
                CidFontType::Type0 // CFF outlines
            } else {
                CidFontType::Type2 // TrueType outlines
            };
            let mut cid = pdf.cid_font(cid_ref);
            cid.subtype(cid_font_type);
            cid.base_font(Name(b"Bravura"));
            cid.system_info(system_info);
            cid.font_descriptor(descriptor_ref);
            cid.default_width(0.0);
            if !font.is_cff {
                cid.cid_to_gid_map_predefined(Name(b"Identity"));
            }

            // Write glyph widths for all used glyphs
            {
                let mut widths = cid.widths();
                for &gid in self.used_glyphs.keys() {
                    let w = if (gid as usize) < font.widths.len() {
                        font.widths[gid as usize]
                    } else {
                        0.0
                    };
                    widths.same(gid, gid, w);
                }
                widths.finish();
            }
            cid.finish();

            // Font descriptor
            let bbox = Rect::new(
                font.to_font_units(font.bbox.0),
                font.to_font_units(font.bbox.1),
                font.to_font_units(font.bbox.2),
                font.to_font_units(font.bbox.3),
            );

            let mut flags = FontFlags::empty();
            flags.insert(FontFlags::SYMBOLIC); // Music font = symbolic
                                               // Not serif, not fixed-pitch, not italic

            let mut desc = pdf.font_descriptor(descriptor_ref);
            desc.name(Name(b"Bravura"))
                .flags(flags)
                .bbox(bbox)
                .italic_angle(0.0)
                .ascent(font.to_font_units(font.ascender))
                .descent(font.to_font_units(font.descender))
                .cap_height(font.to_font_units(font.ascender))
                .stem_v(80.0); // Approximate stem width

            if font.is_cff {
                // CFF font: use FontFile3 with subtype OpenType
                desc.font_file3(font_data_ref);
            } else {
                // TrueType: use FontFile2
                desc.font_file2(font_data_ref);
            }
            desc.finish();

            // ToUnicode CMap (enables copy/paste of glyph names from PDF)
            let cmap_name = Name(b"Bravura-ToUnicode");
            let mut cmap = UnicodeCmap::new(cmap_name, system_info);
            for (&gid, &cp) in &self.used_glyphs {
                if let Some(ch) = char::from_u32(cp) {
                    cmap.pair_with_multiple(gid, [ch].into_iter());
                }
            }
            pdf.cmap(tounicode_ref, &cmap.finish());

            // Embed the raw font file
            let mut stream = pdf.stream(font_data_ref, &font.data);
            if font.is_cff {
                // For CFF (OTTO) fonts, set the subtype to OpenType
                stream.pair(Name(b"Subtype"), Name(b"OpenType"));
            }
            stream.pair(Name(b"Length1"), font.data.len() as i32);
            stream.finish();
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
        self.page_has_content = true;
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
        self.page_has_content = true;
        for i in 0..n_lines {
            let y = height_y + (i as f32) * line_spacing;
            self.staff_line(y, x0, x1);
        }
    }

    /// Draw a bar line.
    /// Reference: PS_Stdio.cp, PS_BarLine(), line 1473
    fn bar_line(&mut self, top_y: f32, bottom_y: f32, x: f32, bar_type: BarLineType) {
        self.page_has_content = true;
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
        self.page_has_content = true;
        self.sync_stroke_color();
        self.content.set_line_width(width * self.state.scale_x);
        self.content
            .set_line_cap(pdf_writer::types::LineCapStyle::ButtCap);
        self.content.move_to(self.tx(x), self.ty(y_top));
        self.content.line_to(self.tx(x), self.ty(y_bottom));
        self.content.stroke();
    }

    // ================== Characters & Text (4 methods) ==================

    /// Draw a music character (glyph) using the embedded SMuFL font.
    ///
    /// If a music font has been loaded, renders the actual glyph from the font.
    /// Falls back to a small filled square placeholder if no font is loaded
    /// or the glyph is not found in the font.
    ///
    /// The Y-flip transform is already in effect (top-left origin), but PDF text
    /// rendering with an embedded font requires flipping Y back for each glyph
    /// so the glyph outline isn't rendered upside-down.
    ///
    /// Reference: PS_Stdio.cp, PS_MusChar(), line 1834
    fn music_char(&mut self, x: f32, y: f32, glyph: MusicGlyph, size_percent: f32) {
        self.page_has_content = true;
        let codepoint = match glyph {
            MusicGlyph::Smufl(cp) => cp,
            MusicGlyph::Sonata(_) => {
                // Legacy Sonata glyphs not supported in font rendering
                return;
            }
        };

        // Try to render with embedded font
        if let Some(ref font) = self.music_font {
            if let Some(gid) = font.glyph_id(codepoint) {
                // Track this glyph for width table and ToUnicode
                self.used_glyphs.entry(gid).or_insert(codepoint);

                // Encode glyph ID as 2-byte big-endian
                let encoded = [(gid >> 8) as u8, (gid & 0xFF) as u8];

                let font_size = self.music_size * size_percent / 100.0;
                let tx = self.tx(x);
                let ty = self.ty(y);

                // We need to temporarily un-flip Y for text rendering because
                // the font outlines are designed for bottom-up Y coordinates.
                // The global transform is [1 0 0 -1 0 page_height] which flips Y.
                // For text: we use a text matrix that flips Y back.
                self.sync_fill_color();
                self.content.begin_text();
                // Text matrix: [sx 0 0 sy tx ty]
                // We need Y to go upward for the font, so use [1 0 0 -1 tx ty]
                // which flips the already-flipped Y back to normal.
                self.content.set_text_matrix([
                    font_size * self.state.scale_x,
                    0.0,
                    0.0,
                    -font_size * self.state.scale_y,
                    tx,
                    ty,
                ]);
                self.content.set_font(Name(b"Bravura"), 1.0);
                self.content.show(Str(&encoded));
                self.content.end_text();
                return;
            }
        }

        // Fallback: placeholder square (no font loaded or glyph not found)
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
        if self.in_page && self.page_has_content {
            // End current page and keep it
            self.content.restore_state();
            let old_content = mem::replace(&mut self.content, Content::new());
            self.pages.push(old_content.finish().to_vec());
        }
        // If in_page but no content was drawn, discard the empty page silently
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
        self.page_has_content = false;
    }

    /// End the current page.
    /// Reference: PS_Stdio.cp, PS_EndPage(), line 1036
    fn end_page(&mut self) {
        if self.in_page {
            self.content.restore_state();
            let old_content = mem::replace(&mut self.content, Content::new());
            self.pages.push(old_content.finish().to_vec());
            self.in_page = false;
            self.page_has_content = false;
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
        // Verify we have exactly 2 pages (not 3 with a blank page)
        let page_count = pdf_bytes
            .windows(12)
            .filter(|w| w == b"/Type /Page\n")
            .count();
        assert_eq!(
            page_count, 2,
            "Should have exactly 2 pages, not {}",
            page_count
        );
    }

    #[test]
    fn test_pdf_renderer_multipage_with_end_page() {
        let mut r = PdfRenderer::new(612.0, 792.0);
        r.staff(100.0, 72.0, 540.0, 5, 10.0);
        r.begin_page(2);
        r.staff(100.0, 72.0, 540.0, 5, 10.0);
        r.begin_page(3);
        r.staff(100.0, 72.0, 540.0, 5, 10.0);
        r.end_page(); // Explicitly end the last page
        let pdf_bytes = r.finish();
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        // Verify we have exactly 3 pages (not 4 with a blank page)
        let page_count = pdf_bytes
            .windows(12)
            .filter(|w| w == b"/Type /Page\n")
            .count();
        assert_eq!(
            page_count, 3,
            "Should have exactly 3 pages, not {}",
            page_count
        );
    }
}
