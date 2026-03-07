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
            staff_line_width: 0.4, // OG default 8% of lnSpace ≈ 0.48; slightly thinner per user pref
            ledger_line_width: 0.64, // PS_Stdio.cp default
            stem_width: 0.8,       // PS_Stdio.cp default
            bar_line_width: 1.0,   // PS_Stdio.cp default
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
/// r.bar_line(100.0, 140.0, 72.0, BarLineType::Single, 10.0);
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
        // Text font refs — standard PDF Type 1 fonts (always available without embedding).
        // Sans-serif (Helvetica): F1, F1B, F1I, F1BI
        let text_font_ref = Ref::new(after_pages + 5);
        let text_font_bold_ref = Ref::new(after_pages + 6);
        let text_font_italic_ref = Ref::new(after_pages + 7);
        let text_font_bold_italic_ref = Ref::new(after_pages + 8);
        // Serif (Times-Roman): F2, F2B, F2I, F2BI
        let serif_font_ref = Ref::new(after_pages + 9);
        let serif_font_bold_ref = Ref::new(after_pages + 10);
        let serif_font_italic_ref = Ref::new(after_pages + 11);
        let serif_font_bold_italic_ref = Ref::new(after_pages + 12);

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

            // Add font resources to each page
            {
                let mut resources = page.resources();
                let mut fonts = resources.fonts();
                if has_font {
                    fonts.pair(Name(b"Bravura"), type0_ref);
                }
                // Built-in PDF text fonts (always available without embedding)
                // Sans-serif (Helvetica)
                fonts.pair(Name(b"F1"), text_font_ref);
                fonts.pair(Name(b"F1B"), text_font_bold_ref);
                fonts.pair(Name(b"F1I"), text_font_italic_ref);
                fonts.pair(Name(b"F1BI"), text_font_bold_italic_ref);
                // Serif (Times-Roman)
                fonts.pair(Name(b"F2"), serif_font_ref);
                fonts.pair(Name(b"F2B"), serif_font_bold_ref);
                fonts.pair(Name(b"F2I"), serif_font_italic_ref);
                fonts.pair(Name(b"F2BI"), serif_font_bold_italic_ref);
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

        // Define built-in text fonts — standard PDF Type 1 fonts,
        // available in all PDF readers without embedding.
        // Reference: PDF spec 1.7, Table 5.17 — Standard Type 1 Fonts
        //
        // Sans-serif (Helvetica family): F1, F1B, F1I, F1BI
        pdf.type1_font(text_font_ref).base_font(Name(b"Helvetica"));
        pdf.type1_font(text_font_bold_ref)
            .base_font(Name(b"Helvetica-Bold"));
        pdf.type1_font(text_font_italic_ref)
            .base_font(Name(b"Helvetica-Oblique"));
        pdf.type1_font(text_font_bold_italic_ref)
            .base_font(Name(b"Helvetica-BoldOblique"));
        // Serif (Times-Roman family): F2, F2B, F2I, F2BI
        pdf.type1_font(serif_font_ref)
            .base_font(Name(b"Times-Roman"));
        pdf.type1_font(serif_font_bold_ref)
            .base_font(Name(b"Times-Bold"));
        pdf.type1_font(serif_font_italic_ref)
            .base_font(Name(b"Times-Italic"));
        pdf.type1_font(serif_font_bold_italic_ref)
            .base_font(Name(b"Times-BoldItalic"));

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
    fn bar_line(
        &mut self,
        top_y: f32,
        bottom_y: f32,
        x: f32,
        bar_type: BarLineType,
        line_space: f32,
    ) {
        self.page_has_content = true;
        self.sync_stroke_color();
        // OG proportions from PS_Stdio.cp:
        //   INTERLNSPACE(lnSpace) = lnSpace/2  — spacing between double-bar components
        //   THICKBARLINE(lnSpace) = lnSpace/2  — thickness of thick barline
        let inter = (line_space / 2.0).max(2.0); // INTERLNSPACE, min 2pt
        let thick_w = (line_space / 2.0).max(1.5); // THICKBARLINE, min 1.5pt
        let blw = self.state.bar_line_width * self.state.scale_x;

        match bar_type {
            BarLineType::Single => {
                self.content.set_line_width(blw);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
            }
            BarLineType::Double => {
                // OG: BL at x, BL at x+INTERLNSPACE
                self.content.set_line_width(blw);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
                self.content.move_to(self.tx(x + inter), self.ty(top_y));
                self.content.line_to(self.tx(x + inter), self.ty(bottom_y));
                self.content.stroke();
            }
            BarLineType::FinalDouble => {
                // OG: thin BL at x, thick ML at x+INTERLNSPACE+thickWidth/2
                self.content.set_line_width(blw);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
                let thick_x = x + inter + thick_w / 2.0;
                self.content.set_line_width(thick_w * self.state.scale_x);
                self.content.move_to(self.tx(thick_x), self.ty(top_y));
                self.content.line_to(self.tx(thick_x), self.ty(bottom_y));
                self.content.stroke();
            }
            BarLineType::RepeatLeft => {
                // OG PS_Repeat RPT_L: thick at left, thin at right, dots right of thin
                // Thick bar at x - thickWidth/2
                let thick_x = x - thick_w / 2.0;
                self.content.set_line_width(thick_w * self.state.scale_x);
                self.content.move_to(self.tx(thick_x), self.ty(top_y));
                self.content.line_to(self.tx(thick_x), self.ty(bottom_y));
                self.content.stroke();
                // Thin bar at x + INTERLNSPACE
                self.content.set_line_width(blw);
                self.content.move_to(self.tx(x + inter), self.ty(top_y));
                self.content.line_to(self.tx(x + inter), self.ty(bottom_y));
                self.content.stroke();
                // Dots right of thin bar: x + INTERLNSPACE + 0.4*lineSpace
                let dot_x = x + inter + 0.4 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_x, line_space);
            }
            BarLineType::RepeatRight => {
                // OG PS_Repeat RPT_R: thin at left, thick at right, dots left of thin
                // Thin bar at x
                self.content.set_line_width(blw);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
                // Thick bar at x + INTERLNSPACE + thickWidth/2
                let thick_x = x + inter + thick_w / 2.0;
                self.content.set_line_width(thick_w * self.state.scale_x);
                self.content.move_to(self.tx(thick_x), self.ty(top_y));
                self.content.line_to(self.tx(thick_x), self.ty(bottom_y));
                self.content.stroke();
                // Dots left of thin bar: x - 0.8*lineSpace
                let dot_x = x - 0.8 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_x, line_space);
            }
            BarLineType::RepeatBoth => {
                // OG PS_Repeat RPT_LR: two thick bars, dots on both sides
                // OG uses 70% thick width to avoid dot collision
                let tw = thick_w * 0.7;
                // Left thick bar at x
                self.content.set_line_width(tw * self.state.scale_x);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
                // Right thick bar at x + INTERLNSPACE + thickWidth/4
                let x2 = x + inter + tw / 4.0;
                self.content.move_to(self.tx(x2), self.ty(top_y));
                self.content.line_to(self.tx(x2), self.ty(bottom_y));
                self.content.stroke();
                // Left dots (to the right of the bar group)
                let dot_xl = x + inter + 0.4 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_xl, line_space);
                // Right dots (to the left of the bar group)
                let dot_xr = x - 0.8 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_xr, line_space);
            }
            BarLineType::Dotted => {
                // Dotted barline: dashed vertical line
                // Reference: DrawObject.cp DrawPSMEAS() — PSM_DOTTED
                let dash = line_space * 0.4 * self.state.scale_x;
                self.content.set_line_width(blw);
                self.content.set_dash_pattern([dash, dash], 0.0);
                self.content.move_to(self.tx(x), self.ty(top_y));
                self.content.line_to(self.tx(x), self.ty(bottom_y));
                self.content.stroke();
                // Reset dash pattern
                self.content
                    .set_dash_pattern(std::iter::empty::<f32>(), 0.0);
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

    /// Draw repeat dots (trait method — delegates to draw_repeat_dots).
    /// Reference: PS_Stdio.cp, PS_Repeat(), line 1521
    fn repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        // Use default line_space based on staff height (4 lines → 3 spaces)
        let line_space = (bottom_y - top_y) / 4.0;
        self.draw_repeat_dots(top_y, bottom_y, x, line_space);
    }

    // ================== Musical Elements (5 methods) ==================

    /// Draw a beam segment.
    /// Reference: PS_Stdio.cp, PS_Beam(), line 1625 + PostScript BM procedure
    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, _up0: bool, _up1: bool) {
        // The OG PostScript BM procedure draws beams with NO stem-direction logic:
        // It always creates a parallelogram from (x0,y0) → (x1,y1) → (x1,y1+th) → (x0,y0+th).
        // The y0/y1 coordinates are positioned by the caller to be flush with stem endpoints.
        self.sync_fill_color();
        self.content.move_to(self.tx(x1), self.ty(y1));
        self.content.line_to(self.tx(x1), self.ty(y1 + thickness));
        self.content.line_to(self.tx(x0), self.ty(y0 + thickness));
        self.content.line_to(self.tx(x0), self.ty(y0));
        self.content.close_path();
        self.content.fill_nonzero();
    }

    /// Draw a slur or tie as a filled Bezier shape.
    ///
    /// Port of PS_Stdio.cp PS_Slur() (line 1933).
    ///
    /// Solid slurs: filled region between two offset cubic Bezier curves,
    /// creating a natural taper (zero thickness at endpoints, maximum at
    /// midpoint). The inner curve has control points offset by SLURTHICK
    /// in the Y direction.
    ///
    /// Dashed slurs: single stroked Bezier curve with dash pattern.
    ///
    /// OG formula: SLURTHICK(lnSpace) = config.slurMidLW * lnSpace / 100
    /// where slurMidLW defaults to ~25 (25% of staff line spacing).
    ///
    /// Reference: PS_Stdio.cp:1931-1959, MC/SL PostScript operators
    fn slur(&mut self, p0: Point, c1: Point, c2: Point, p3: Point, dashed: bool) {
        if dashed {
            // Dashed slurs: single stroked Bezier curve
            // Reference: PS_Stdio.cp:1938-1945
            self.sync_stroke_color();
            self.content.set_line_width(0.8 * self.state.scale_x);
            self.content
                .set_line_cap(pdf_writer::types::LineCapStyle::RoundCap);
            let d = 3.0 * self.state.scale_x;
            self.content.set_dash_pattern([d, d], 0.0);

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
            self.content.set_dash_pattern([], 0.0);
        } else {
            // Solid slurs: filled region between two offset Bezier curves.
            // Reference: PS_Stdio.cp:1947-1956
            //
            // SLURTHICK(lnSpace) = config.slurMidLW * lnSpace / 100
            // lnSpace = music_size / 4 (4 staff spaces in music font size)
            let ln_space = self.music_size / 4.0;
            let slur_mid_lw: f32 = 30.0; // OG SLURMIDLW_DFLT (Initialize.cp:975)

            // Direction: determine whether slur curves up or down.
            // OG (PS_Stdio.cp:1951): up = c1y < p0y (QD coords, Y↓)
            // Our coords: same convention (Y increases downward).
            let up = c1.y < p0.y;
            let thick = if up {
                slur_mid_lw * ln_space / 100.0
            } else {
                -(slur_mid_lw * ln_space / 100.0)
            };

            self.sync_fill_color();

            // Outer curve: p0 → p3 via (c1, c2)
            // OG: MC operator = moveto p0, curveto c1 c2 p3
            self.content.move_to(self.tx(p0.x), self.ty(p0.y));
            self.content.cubic_to(
                self.tx(c1.x),
                self.ty(c1.y),
                self.tx(c2.x),
                self.ty(c2.y),
                self.tx(p3.x),
                self.ty(p3.y),
            );

            // Inner curve (reversed): p3 → p0 via (c2+thick, c1+thick)
            // OG: SL operator = curveto c2+thick c1+thick p0, closepath fill
            // Endpoints NOT offset → taper to zero at ends.
            // Control points offset by thick → maximum width at midpoint.
            self.content.cubic_to(
                self.tx(c2.x),
                self.ty(c2.y + thick),
                self.tx(c1.x),
                self.ty(c1.y + thick),
                self.tx(p0.x),
                self.ty(p0.y),
            );

            self.content.close_path();
            self.content.fill_nonzero();
        }
    }

    /// Draw a system bracket using SMuFL glyph U+E002.
    ///
    /// Ported from OG BK PostScript operator (PS_Stdio.cp, PS_Bracket() line 1966).
    /// OG draws: top serif glyph + filled vertical rectangle + bottom serif glyph.
    /// Bar width = topBracketCharWidth × 0.36275 (from the BK prolog definition).
    ///
    /// We draw bracketTop (U+E003) and bracketBottom (U+E004) as serif glyphs,
    /// with a filled rectangle connecting them for the vertical bar.
    ///
    /// Reference: DrawObject.cp line 807, PS_Stdio.cp BK operator definition
    fn bracket(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        let height = (y_bottom - y_top).abs();
        if height < 1.0 {
            return;
        }
        self.page_has_content = true;

        // OG: bar_width = CharWidth(MCH_topbracket) * 0.36275
        // At music_size, the Sonata top bracket char is roughly 0.5em wide,
        // so bar_width ≈ music_size * 0.5 * 0.36275 ≈ music_size * 0.18.
        // We derive from the actual glyph advance width if available.
        let mut bar_width = self.music_size * 0.18;
        let mut drew_top_glyph = false;
        let mut drew_bottom_glyph = false;

        // Draw top serif glyph (bracketTop U+E003)
        if let Some(ref font) = self.music_font {
            let top_cp: u32 = 0xE003;
            if let Some(gid) = font.glyph_id(top_cp) {
                // Use glyph advance width to compute bar_width.
                // widths[] is in PDF font units (1000/em), so convert to points.
                let w1000 = font.widths.get(gid as usize).copied().unwrap_or(0.0);
                if w1000 > 0.0 {
                    let char_width_pt = (w1000 / 1000.0) * self.music_size;
                    bar_width = char_width_pt * 0.36275;
                    if bar_width < 1.0 {
                        bar_width = 1.0;
                    }
                }
                self.used_glyphs.entry(gid).or_insert(top_cp);
                let encoded = [(gid >> 8) as u8, (gid & 0xFF) as u8];

                let font_scale = self.music_size;
                let tx = self.tx(x);
                let ty = self.ty(y_top);

                self.sync_fill_color();
                self.content.begin_text();
                self.content
                    .set_text_matrix([font_scale, 0.0, 0.0, -font_scale, tx, ty]);
                self.content.set_font(Name(b"Bravura"), 1.0);
                self.content.show(Str(&encoded));
                self.content.end_text();
                drew_top_glyph = true;
            }
        }

        // Draw bottom serif glyph (bracketBottom U+E004)
        if let Some(ref font) = self.music_font {
            let bottom_cp: u32 = 0xE004;
            if let Some(gid) = font.glyph_id(bottom_cp) {
                self.used_glyphs.entry(gid).or_insert(bottom_cp);
                let encoded = [(gid >> 8) as u8, (gid & 0xFF) as u8];

                let font_scale = self.music_size;
                let tx = self.tx(x);
                let ty = self.ty(y_bottom);

                self.sync_fill_color();
                self.content.begin_text();
                self.content
                    .set_text_matrix([font_scale, 0.0, 0.0, -font_scale, tx, ty]);
                self.content.set_font(Name(b"Bravura"), 1.0);
                self.content.show(Str(&encoded));
                self.content.end_text();
                drew_bottom_glyph = true;
            }
        }

        // Draw filled vertical bar connecting top and bottom serifs.
        // OG BK: x y moveto x y+h lineto x+bkw y+h lineto x+bkw y lineto closepath fill
        let bx = self.tx(x);
        let by_top = self.ty(y_top);
        let by_bot = self.ty(y_bottom);
        let bw = bar_width * self.state.scale_x;

        self.sync_fill_color();
        self.content.move_to(bx, by_top);
        self.content.line_to(bx, by_bot);
        self.content.line_to(bx + bw, by_bot);
        self.content.line_to(bx + bw, by_top);
        self.content.close_path();
        self.content.fill_nonzero();

        // If we couldn't draw font glyphs, draw geometric serifs as fallback
        if !drew_top_glyph {
            let serif_len = bar_width * 2.5;
            self.content.set_line_width(1.5 * self.state.scale_x);
            self.content.move_to(bx, by_top);
            self.content
                .line_to(bx + serif_len * self.state.scale_x, by_top);
            self.content.stroke();
        }
        if !drew_bottom_glyph {
            let serif_len = bar_width * 2.5;
            self.content.set_line_width(1.5 * self.state.scale_x);
            self.content.move_to(bx, by_bot);
            self.content
                .line_to(bx + serif_len * self.state.scale_x, by_bot);
            self.content.stroke();
        }
    }

    /// Draw a brace connecting staves using the OG Nightingale homebrew Bezier path.
    ///
    /// Ported from the PostScript prolog in OG EPSF output (03b.ng.EPSF).
    /// The path was "sucked out of a FH 3.1 EPS drawing of half brace" and
    /// consists of 4 cubic Bezier segments forming a filled teardrop shape.
    /// Two mirrored halves are drawn (upper + lower) to form the complete brace.
    ///
    /// Path constants: bracePWidth=20, braceHalfHt=213
    /// Scale: X = music_size / (bracePWidth * 16), Y = halfHeight / braceHalfHt
    /// The factor of 16 converts from the OG DDIST coordinate system to points.
    ///
    /// Reference: PS_Stdio.cp, PS_Brace() line 1980; BR operator in PS prolog
    fn brace(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        let height = (y_bottom - y_top).abs();
        if height < 1.0 {
            return;
        }

        self.page_has_content = true;
        let half_height = height / 2.0;
        let center_y = (y_top + y_bottom) / 2.0;

        // OG scale factors convert path units to render-coordinate points:
        // X = MFS / (bracePWidth * 16), Y = halfHeight / braceHalfHt
        // bracePWidth=20 (path designed for 20pt font), braceHalfHt=213
        // The 16 converts from DDIST to points in the OG coordinate system.
        // Bravura brace glyphs are slightly thinner than Sonata's — boost x_scale
        // by ~30% to match the OG brace weight.
        let x_scale = self.music_size / 245.0;
        let y_scale = half_height / 213.0;

        self.sync_fill_color();

        // Draw both halves: upper (y_sign=-1, extends upward) and lower (y_sign=+1)
        for y_sign in [-1.0_f32, 1.0_f32] {
            // Pre-compute all path points to avoid borrow conflicts with self.content.
            // OG ourBrace path (after pT origin transform: x-211, y-346):
            // Outer edge: (67,213) → (21,62) → (0,0) tip
            // Inner edge: (0,0) tip → (56,78) → (67,213) arm
            let bx = |px: f32| -> f32 { x + px * x_scale };
            let by = |py: f32| -> f32 { center_y + py * y_scale * y_sign };

            let pts: [(f32, f32); 13] = [
                (self.tx(bx(67.0)), self.ty(by(213.0))),     // move_to
                (self.tx(bx(31.0)), self.ty(by(197.0))),     // cubic 1 cp1
                (self.tx(bx(-33.0)), self.ty(by(139.0))),    // cubic 1 cp2
                (self.tx(bx(21.0)), self.ty(by(62.0))),      // cubic 1 end
                (self.tx(bx(37.769)), self.ty(by(38.0886))), // cubic 2 cp1
                (self.tx(bx(44.0)), self.ty(by(9.0))),       // cubic 2 cp2
                (self.tx(bx(0.0)), self.ty(by(0.0))),        // cubic 2 end (tip)
                (self.tx(bx(58.0)), self.ty(by(4.0))),       // cubic 3 cp1
                (self.tx(bx(79.47)), self.ty(by(38.8833))),  // cubic 3 cp2
                (self.tx(bx(56.0)), self.ty(by(78.0))),      // cubic 3 end
                (self.tx(bx(35.0)), self.ty(by(113.0))),     // cubic 4 cp1
                (self.tx(bx(14.0)), self.ty(by(164.0))),     // cubic 4 cp2
                (self.tx(bx(67.0)), self.ty(by(213.0))),     // cubic 4 end (= start)
            ];

            self.content.move_to(pts[0].0, pts[0].1);
            self.content
                .cubic_to(pts[1].0, pts[1].1, pts[2].0, pts[2].1, pts[3].0, pts[3].1);
            self.content
                .cubic_to(pts[4].0, pts[4].1, pts[5].0, pts[5].1, pts[6].0, pts[6].1);
            self.content
                .cubic_to(pts[7].0, pts[7].1, pts[8].0, pts[8].1, pts[9].0, pts[9].1);
            self.content.cubic_to(
                pts[10].0, pts[10].1, pts[11].0, pts[11].1, pts[12].0, pts[12].1,
            );
            self.content.close_path();
            self.content.fill_nonzero();
        }
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
    fn text_string(&mut self, x: f32, y: f32, text: &str, font: &TextFont) {
        // Select the correct standard PDF Type1 font family based on font name.
        // Serif fonts (Times, Palatino, Briard, etc.) → Times-Roman family (F2*)
        // Sans-serif fonts (Helvetica, Arial, etc.) → Helvetica family (F1*)
        let is_serif = is_serif_font(&font.name);
        let font_name = match (is_serif, font.bold, font.italic) {
            (false, false, false) => pdf_writer::Name(b"F1"),
            (false, true, false) => pdf_writer::Name(b"F1B"),
            (false, false, true) => pdf_writer::Name(b"F1I"),
            (false, true, true) => pdf_writer::Name(b"F1BI"),
            (true, false, false) => pdf_writer::Name(b"F2"),
            (true, true, false) => pdf_writer::Name(b"F2B"),
            (true, false, true) => pdf_writer::Name(b"F2I"),
            (true, true, true) => pdf_writer::Name(b"F2BI"),
        };
        let size = font.size.max(4.0); // minimum 4pt to avoid invisible text
        let tx = self.tx(x);
        let ty = self.ty(y);
        self.sync_fill_color();
        self.content.begin_text();
        self.content.set_font(font_name, 1.0); // size applied via text matrix
                                               // The global CTM has Y-flip [1 0 0 -1 0 page_height], which makes text
                                               // render upside-down. Use a text matrix to flip Y back for text rendering,
                                               // same approach as music_char().
                                               // Text matrix: [sx 0 0 -sy tx ty] un-flips the Y axis for this text.
        let scaled_size = size * self.state.scale_x;
        self.content
            .set_text_matrix([scaled_size, 0.0, 0.0, -scaled_size, tx, ty]);
        // With text matrix set, show at origin (position is in the matrix)
        // PDF standard Type1 fonts (Helvetica, Times-Roman) use WinAnsiEncoding
        // (Windows-1252), a single-byte encoding. We must transcode the UTF-8
        // string to WinAnsi bytes before passing it to Str(). Passing raw UTF-8
        // bytes would produce garbled output for non-ASCII characters (e.g.,
        // German ä = UTF-8 0xC3 0xA4 would be read as two wrong characters
        // instead of the correct single byte 0xE4).
        let win_ansi = utf8_to_win_ansi(text);
        self.content.show(Str(&win_ansi));
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
    /// Draw two repeat dots vertically centered on the staff middle two spaces.
    ///
    /// OG Nightingale uses MCH_dot (augmentation dot) at 150% size for fonts without
    /// MCH_rptDots. We use filled circles positioned at staff spaces 2 and 3
    /// (counting from top, for a 5-line staff: spaces between lines 2-3 and 3-4).
    ///
    /// Reference: PS_Stdio.cp PS_Repeat() lines 1530-1545
    fn draw_repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32, line_space: f32) {
        let staff_height = bottom_y - top_y;
        // Dot radius: about 25% of line_space (similar to augmentation dot at 150%)
        let dot_radius = (line_space * 0.25).max(1.0);
        self.sync_fill_color();
        // Upper dot: 1.5 spaces below top = between lines 2-3
        let cy1 = top_y + staff_height / 2.0 - line_space / 2.0;
        self.draw_circle(x, cy1, dot_radius);
        // Lower dot: 2.5 spaces below top = between lines 3-4
        let cy2 = top_y + staff_height / 2.0 + line_space / 2.0;
        self.draw_circle(x, cy2, dot_radius);
    }

    /// Draw an approximate circle using 4 cubic Bezier curves.
    ///
    /// Uses the standard Bezier circle approximation:
    /// control point offset = radius * 0.5522847498 (4/3 * (sqrt(2) - 1))
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

/// Classify a font name as serif or sans-serif for PDF standard font selection.
///
/// Serif fonts map to Times-Roman family (F2*). Sans-serif map to Helvetica (F1*).
/// Unknown fonts default to serif (more common in music engraving).
fn is_serif_font(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    // Explicitly sans-serif fonts
    if lower.contains("helvetica")
        || lower.contains("arial")
        || lower.contains("sans")
        || lower.contains("gill")
        || lower.contains("futura")
        || lower.contains("avenir")
        || lower.contains("verdana")
        || lower.contains("tahoma")
        || lower.contains("calibri")
    {
        return false;
    }
    // Everything else is treated as serif (Times, Palatino, Briard, Georgia,
    // Garamond, Caslon, etc.). This is the safer default for music scores.
    true
}

/// Convert a UTF-8 string to Windows-1252 (WinAnsiEncoding) bytes for use
/// in PDF standard Type1 font strings.
///
/// PDF standard Type1 fonts (Helvetica, Times-Roman, etc.) are encoded with
/// WinAnsiEncoding (Windows-1252), a single-byte encoding. Multi-byte UTF-8
/// sequences must be transcoded to the corresponding single byte, or the PDF
/// reader will misinterpret them.
///
/// Encoding rules:
/// - U+0000–U+007F: pass through (ASCII, same in all encodings)
/// - U+00A0–U+00FF: direct cast — Latin-1 Supplement maps 1:1 to WinAnsi
///   0xA0–0xFF (e.g., ä=U+00E4→0xE4, ö=U+00F6→0xF6, ü=U+00FC→0xFC)
/// - U+0080–U+009F: Windows-1252 special characters (€, smart quotes, etc.)
///   occupy WinAnsi bytes 0x80–0x9F via a lookup table
/// - All other codepoints: substitute '?' (0x3F)
fn utf8_to_win_ansi(text: &str) -> Vec<u8> {
    // Windows-1252 special characters in range 0x80–0x9F.
    // Index 0 → byte 0x80, index 31 → byte 0x9F.
    // Entries of 0 indicate undefined positions in Windows-1252.
    const WIN1252_SPECIAL: [(char, u8); 27] = [
        ('\u{20AC}', 0x80), // €
        ('\u{201A}', 0x82), // ‚
        ('\u{0192}', 0x83), // ƒ
        ('\u{201E}', 0x84), // „
        ('\u{2026}', 0x85), // …
        ('\u{2020}', 0x86), // †
        ('\u{2021}', 0x87), // ‡
        ('\u{02C6}', 0x88), // ˆ
        ('\u{2030}', 0x89), // ‰
        ('\u{0160}', 0x8A), // Š
        ('\u{2039}', 0x8B), // ‹
        ('\u{0152}', 0x8C), // Œ
        ('\u{017D}', 0x8E), // Ž
        ('\u{2018}', 0x91), // '
        ('\u{2019}', 0x92), // '
        ('\u{201C}', 0x93), // "
        ('\u{201D}', 0x94), // "
        ('\u{2022}', 0x95), // •
        ('\u{2013}', 0x96), // –
        ('\u{2014}', 0x97), // —
        ('\u{02DC}', 0x98), // ˜
        ('\u{2122}', 0x99), // ™
        ('\u{0161}', 0x9A), // š
        ('\u{203A}', 0x9B), // ›
        ('\u{0153}', 0x9C), // œ
        ('\u{017E}', 0x9E), // ž
        ('\u{0178}', 0x9F), // Ÿ
    ];

    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let cp = ch as u32;
        if cp <= 0x7F {
            // ASCII: direct pass-through
            out.push(cp as u8);
        } else if (0x00A0..=0x00FF).contains(&cp) {
            // Latin-1 Supplement: codepoint == WinAnsi byte
            out.push(cp as u8);
        } else {
            // Check Windows-1252 special range (0x80–0x9F)
            let mut found = false;
            for &(c, b) in &WIN1252_SPECIAL {
                if ch == c {
                    out.push(b);
                    found = true;
                    break;
                }
            }
            if !found {
                out.push(b'?'); // Substitute for unmappable codepoints
            }
        }
    }
    out
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
        r.bar_line(20.0, 60.0, 50.0, BarLineType::Single, 10.0);
        r.bar_line(20.0, 60.0, 100.0, BarLineType::Double, 10.0);
        r.bar_line(20.0, 60.0, 150.0, BarLineType::FinalDouble, 10.0);
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

    #[test]
    fn test_utf8_to_win_ansi_ascii() {
        // ASCII passthrough
        assert_eq!(utf8_to_win_ansi("Hello"), b"Hello");
        assert_eq!(utf8_to_win_ansi("abc 123"), b"abc 123");
    }

    #[test]
    fn test_utf8_to_win_ansi_german_umlauts() {
        // German umlauts: ä=U+00E4→0xE4, ö=U+00F6→0xF6, ü=U+00FC→0xFC
        // Uppercase: Ä=U+00C4→0xC4, Ö=U+00D6→0xD6, Ü=U+00DC→0xDC
        assert_eq!(utf8_to_win_ansi("ä"), &[0xE4]);
        assert_eq!(utf8_to_win_ansi("ö"), &[0xF6]);
        assert_eq!(utf8_to_win_ansi("ü"), &[0xFC]);
        assert_eq!(utf8_to_win_ansi("Ä"), &[0xC4]);
        assert_eq!(utf8_to_win_ansi("Ö"), &[0xD6]);
        assert_eq!(utf8_to_win_ansi("Ü"), &[0xDC]);
        // Mixed: "für" → [0x66, 0xFC, 0x72]
        assert_eq!(utf8_to_win_ansi("für"), &[0x66, 0xFC, 0x72]);
    }

    #[test]
    fn test_utf8_to_win_ansi_smart_quotes() {
        // Smart quotes are in Windows-1252 special range (0x80–0x9F)
        // Left double quote U+201C → 0x93, right double quote U+201D → 0x94
        assert_eq!(utf8_to_win_ansi("\u{201C}"), &[0x93]);
        assert_eq!(utf8_to_win_ansi("\u{201D}"), &[0x94]);
        // Euro sign U+20AC → 0x80
        assert_eq!(utf8_to_win_ansi("\u{20AC}"), &[0x80]);
    }

    #[test]
    fn test_utf8_to_win_ansi_unmappable() {
        // Characters outside Windows-1252 should become '?'
        assert_eq!(utf8_to_win_ansi("\u{4E2D}"), b"?"); // CJK character
        assert_eq!(utf8_to_win_ansi("A\u{4E2D}B"), b"A?B");
    }
}
