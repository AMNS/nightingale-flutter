// BitmapRenderer — Pure-Rust bitmap rendering via tiny-skia
//
// Implements MusicRenderer to produce per-page PNG bitmaps directly in
// `cargo test` without external PDF→PNG tooling. Uses tiny-skia for
// anti-aliased path rasterization and ttf-parser for font glyph outlines.
//
// Architecture:
//   - All coordinates arrive in points (1/72 inch), same as PdfRenderer
//   - Internal rendering uses pixel coordinates: px = pt * (dpi / 72.0)
//   - Music glyphs rendered via ttf-parser outline → tiny-skia path → fill
//   - Text rendered via loaded TTF/OTF fonts with glyph outline extraction
//   - Multi-page: Vec<Pixmap>, one per page

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke as SkStroke, Transform,
};

use super::types::{BarLineType, Color, MusicGlyph, Point, RenderRect, TextFont};
use super::MusicRenderer;

// ── Font outline → tiny-skia path conversion ────────────────────────────

/// Collects ttf-parser outline segments into a tiny-skia PathBuilder.
struct OutlineCollector {
    builder: PathBuilder,
}

impl OutlineCollector {
    fn new() -> Self {
        Self {
            builder: PathBuilder::new(),
        }
    }

    /// Consume the collector and return the built path (if non-empty).
    fn finish(self) -> Option<tiny_skia::Path> {
        self.builder.finish()
    }
}

impl ttf_parser::OutlineBuilder for OutlineCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

// ── Loaded font data ────────────────────────────────────────────────────

/// Pre-parsed font data with cmap, advance widths, and cached glyph paths.
struct LoadedFont {
    /// Raw font file bytes (re-parsed temporarily for outline extraction).
    data: Vec<u8>,
    /// Unicode codepoint → glyph ID.
    cmap: HashMap<u32, u16>,
    /// Glyph ID → horizontal advance width in font design units.
    advance_widths: HashMap<u16, u16>,
    /// Font design units per em.
    units_per_em: u16,
    /// Cached glyph outlines (glyph ID → path in font design units, Y-up).
    /// None value means "outline was requested but glyph has no outline".
    glyph_cache: HashMap<u16, Option<tiny_skia::Path>>,
}

impl LoadedFont {
    /// Parse a font file and extract cmap + metrics.
    fn load(data: Vec<u8>, scan_ranges: &[std::ops::RangeInclusive<u32>]) -> Option<Self> {
        let face = ttf_parser::Face::parse(&data, 0).ok()?;
        let units_per_em = face.units_per_em();

        let mut cmap = HashMap::new();
        let mut advance_widths = HashMap::new();

        for range in scan_ranges {
            for cp in range.clone() {
                if let Some(ch) = char::from_u32(cp) {
                    if let Some(gid) = face.glyph_index(ch) {
                        cmap.insert(cp, gid.0);
                        advance_widths
                            .entry(gid.0)
                            .or_insert_with(|| face.glyph_hor_advance(gid).unwrap_or(0));
                    }
                }
            }
        }

        Some(Self {
            data,
            cmap,
            advance_widths,
            units_per_em,
            glyph_cache: HashMap::new(),
        })
    }

    /// Get or cache the glyph outline path (in font design units, Y-up).
    fn get_glyph_path(&mut self, glyph_id: u16) -> Option<tiny_skia::Path> {
        if let Some(cached) = self.glyph_cache.get(&glyph_id) {
            return cached.clone();
        }

        let face = ttf_parser::Face::parse(&self.data, 0).ok()?;
        let gid = ttf_parser::GlyphId(glyph_id);

        let mut collector = OutlineCollector::new();
        let result = if face.outline_glyph(gid, &mut collector).is_some() {
            collector.finish()
        } else {
            None
        };

        self.glyph_cache.insert(glyph_id, result.clone());
        result
    }

    /// Get the horizontal advance width for a glyph, in font design units.
    fn advance_width(&self, glyph_id: u16) -> u16 {
        self.advance_widths.get(&glyph_id).copied().unwrap_or(0)
    }

    /// Look up glyph ID for a Unicode codepoint.
    fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        self.cmap.get(&codepoint).copied()
    }
}

// ── Graphics state ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct RenderState {
    color: Color,
    line_width: f32,
    staff_lw: f32,
    ledger_lw: f32,
    stem_lw: f32,
    bar_lw: f32,
    music_size: f32,
    /// Translation in points (applied before DPI scaling).
    translate_x: f32,
    translate_y: f32,
    /// Scale factors (user-space, applied before DPI scaling).
    scale_x: f32,
    scale_y: f32,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            line_width: 1.0,
            staff_lw: 0.4,
            ledger_lw: 0.64,
            stem_lw: 0.8,
            bar_lw: 1.0,
            music_size: 24.0,
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}

// ── BitmapRenderer ──────────────────────────────────────────────────────

/// Pure-Rust bitmap renderer for visual regression testing.
///
/// Implements `MusicRenderer` using `tiny-skia` for rasterization and
/// `ttf-parser` for font glyph outline extraction. Produces per-page
/// RGBA bitmaps that can be saved as PNG or compared pixel-by-pixel.
///
/// # Example
///
/// ```ignore
/// use nightingale_core::render::{BitmapRenderer, MusicRenderer};
///
/// let mut r = BitmapRenderer::new(150.0); // 150 DPI
/// r.load_music_font_file(Path::new("fonts/Bravura.otf"));
/// r.set_page_size(612.0, 792.0);
/// r.begin_page(1);
/// r.staff(100.0, 72.0, 540.0, 5, 10.0);
/// r.end_page();
/// r.save_pages(Path::new("/tmp"), "test").unwrap();
/// ```
pub struct BitmapRenderer {
    /// Completed pages (RGBA bitmaps).
    pages: Vec<Pixmap>,
    /// Current page being drawn on.
    current_pixmap: Option<Pixmap>,
    /// Page dimensions in points.
    page_width_pt: f32,
    page_height_pt: f32,
    /// Rendering DPI (pixels per inch). Scale factor = dpi / 72.
    #[allow(dead_code)]
    dpi: f32,
    /// Precomputed scale factor: dpi / 72.0.
    scale: f32,
    /// Whether we're inside a page.
    in_page: bool,
    /// Whether any drawing has occurred on the current page.
    page_has_content: bool,

    /// Current graphics state.
    state: RenderState,
    /// State stack for save/restore.
    state_stack: Vec<RenderState>,

    /// Music font (Bravura or other SMuFL font).
    music_font: Option<LoadedFont>,
    /// Text fonts keyed by role: "sans", "sans-bold", "serif-italic", etc.
    text_fonts: HashMap<String, LoadedFont>,
}

impl BitmapRenderer {
    /// Create a new BitmapRenderer at the given DPI.
    ///
    /// Common DPI values: 72 (1:1 with points), 150 (good for tests), 300 (print).
    pub fn new(dpi: f32) -> Self {
        Self {
            pages: Vec::new(),
            current_pixmap: None,
            page_width_pt: 612.0, // US Letter default
            page_height_pt: 792.0,
            dpi,
            scale: dpi / 72.0,
            in_page: false,
            page_has_content: false,
            state: RenderState::default(),
            state_stack: Vec::new(),
            music_font: None,
            text_fonts: HashMap::new(),
        }
    }

    /// Load a SMuFL-compatible music font (e.g. Bravura.otf).
    pub fn load_music_font(&mut self, data: Vec<u8>) -> bool {
        // Scan SMuFL Private Use Area (U+E000..U+F8FF)
        if let Some(font) = LoadedFont::load(data, &[0xE000..=0xF8FF]) {
            self.music_font = Some(font);
            true
        } else {
            false
        }
    }

    /// Load a music font from a file path.
    pub fn load_music_font_file(&mut self, path: &Path) -> bool {
        if let Ok(data) = std::fs::read(path) {
            self.load_music_font(data)
        } else {
            false
        }
    }

    /// Load a text font for a given role (e.g. "sans", "serif-bold-italic").
    ///
    /// Role keys: "sans", "sans-bold", "sans-italic", "sans-bold-italic",
    ///            "serif", "serif-bold", "serif-italic", "serif-bold-italic"
    pub fn load_text_font(&mut self, data: Vec<u8>, role: &str) -> bool {
        // Scan Basic Latin + Latin-1 Supplement + Latin Extended-A
        let ranges = [0x20..=0x7E, 0xA0..=0xFF, 0x100..=0x17F];
        if let Some(font) = LoadedFont::load(data, &ranges) {
            self.text_fonts.insert(role.to_string(), font);
            true
        } else {
            false
        }
    }

    /// Load a text font from a file path for a given role.
    pub fn load_text_font_file(&mut self, path: &Path, role: &str) -> bool {
        if let Ok(data) = std::fs::read(path) {
            self.load_text_font(data, role)
        } else {
            false
        }
    }

    /// Load Liberation Sans + Serif text fonts from a directory.
    ///
    /// Expects the directory to contain:
    ///   LiberationSerif-{Regular,Bold,Italic,BoldItalic}.ttf
    ///   LiberationSans-{Regular,Bold,Italic,BoldItalic}.ttf
    ///
    /// These are metric-compatible replacements for Times New Roman and
    /// Helvetica/Arial, matching the fonts used in Nightingale NGL files.
    /// Returns the number of fonts successfully loaded (0..8).
    pub fn load_text_fonts_from_dir(&mut self, dir: &Path) -> usize {
        let variants: &[(&str, &str)] = &[
            ("serif", "LiberationSerif-Regular.ttf"),
            ("serif-bold", "LiberationSerif-Bold.ttf"),
            ("serif-italic", "LiberationSerif-Italic.ttf"),
            ("serif-bold-italic", "LiberationSerif-BoldItalic.ttf"),
            ("sans", "LiberationSans-Regular.ttf"),
            ("sans-bold", "LiberationSans-Bold.ttf"),
            ("sans-italic", "LiberationSans-Italic.ttf"),
            ("sans-bold-italic", "LiberationSans-BoldItalic.ttf"),
        ];
        let mut loaded = 0;
        for (role, filename) in variants {
            if self.load_text_font_file(&dir.join(filename), role) {
                loaded += 1;
            }
        }
        loaded
    }

    /// Get the completed pages as Pixmap references.
    pub fn pages(&self) -> &[Pixmap] {
        &self.pages
    }

    /// Get the number of completed pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Get raw RGBA pixel data for a page (premultiplied alpha).
    pub fn page_data(&self, page: usize) -> Option<&[u8]> {
        self.pages.get(page).map(|p| p.data())
    }

    /// Get pixel dimensions of a page.
    pub fn page_dimensions(&self, page: usize) -> Option<(u32, u32)> {
        self.pages.get(page).map(|p| (p.width(), p.height()))
    }

    /// Save all pages as PNG files.
    ///
    /// Files are named `{prefix}_page{N}.png` (1-indexed).
    /// Returns the list of written file paths.
    pub fn save_pages(&self, dir: &Path, prefix: &str) -> std::io::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for (i, pixmap) in self.pages.iter().enumerate() {
            let path = dir.join(format!("{}_page{}.png", prefix, i + 1));
            pixmap
                .save_png(&path)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            paths.push(path);
        }
        Ok(paths)
    }

    // ── Coordinate helpers ──────────────────────────────────────────────

    /// Convert point-space X to pixel-space X (applying state transform + DPI scaling).
    #[inline]
    fn px(&self, x: f32) -> f32 {
        (x + self.state.translate_x) * self.state.scale_x * self.scale
    }

    /// Convert point-space Y to pixel-space Y (applying state transform + DPI scaling).
    #[inline]
    fn py(&self, y: f32) -> f32 {
        (y + self.state.translate_y) * self.state.scale_y * self.scale
    }

    /// Scale a width value from points to pixels.
    #[inline]
    fn pw(&self, w: f32) -> f32 {
        (w * self.state.scale_x).abs() * self.scale
    }

    // ── Paint helpers ───────────────────────────────────────────────────

    /// Create a Paint with the current color.
    fn make_paint(&self) -> Paint<'static> {
        let c = self.state.color;
        let mut paint = Paint::default();
        paint.set_color_rgba8(
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            (c.a * 255.0) as u8,
        );
        paint.anti_alias = true;
        paint
    }

    /// Create a stroke with the given pixel width.
    fn make_stroke(&self, width_px: f32, cap: LineCap) -> SkStroke {
        SkStroke {
            width: width_px.max(0.1), // tiny-skia requires positive width
            line_cap: cap,
            line_join: LineJoin::Miter,
            ..SkStroke::default()
        }
    }

    // ── Drawing helpers ─────────────────────────────────────────────────

    /// Stroke a path on the current pixmap.
    fn stroke_path(&mut self, path: &tiny_skia::Path, width_px: f32, cap: LineCap) {
        let paint = self.make_paint();
        let stroke = self.make_stroke(width_px, cap);
        if let Some(ref mut pixmap) = self.current_pixmap {
            pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
        }
    }

    /// Fill a path on the current pixmap.
    fn fill_path(&mut self, path: &tiny_skia::Path) {
        let paint = self.make_paint();
        if let Some(ref mut pixmap) = self.current_pixmap {
            pixmap.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    /// Draw a filled circle (Bezier approximation).
    fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32) {
        let pcx = self.px(cx);
        let pcy = self.py(cy);
        let pr = self.pw(radius);
        let k = pr * 0.552_284_8; // (4/3) * tan(π/8)

        let mut pb = PathBuilder::new();
        pb.move_to(pcx + pr, pcy);
        pb.cubic_to(pcx + pr, pcy + k, pcx + k, pcy + pr, pcx, pcy + pr);
        pb.cubic_to(pcx - k, pcy + pr, pcx - pr, pcy + k, pcx - pr, pcy);
        pb.cubic_to(pcx - pr, pcy - k, pcx - k, pcy - pr, pcx, pcy - pr);
        pb.cubic_to(pcx + k, pcy - pr, pcx + pr, pcy - k, pcx + pr, pcy);
        pb.close();

        if let Some(path) = pb.finish() {
            self.fill_path(&path);
        }
    }

    /// Draw repeat dots for barlines (two filled circles at staff middle).
    fn draw_repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32, line_space: f32) {
        let staff_height = bottom_y - top_y;
        let dot_radius = (line_space * 0.25).max(1.0);
        let cy1 = top_y + staff_height / 2.0 - line_space / 2.0;
        let cy2 = top_y + staff_height / 2.0 + line_space / 2.0;
        self.draw_circle(x, cy1, dot_radius);
        self.draw_circle(x, cy2, dot_radius);
    }

    /// Render a single glyph from a font at the given position in pixel coords.
    ///
    /// `font_scale` = font_size_pt / units_per_em * dpi_scale
    /// The transform: scale by font_scale, negate Y (font Y-up → screen Y-down),
    /// translate to (px_x, px_y).
    fn render_glyph(
        pixmap: &mut Pixmap,
        path: &tiny_skia::Path,
        font_scale: f32,
        px_x: f32,
        px_y: f32,
        paint: &Paint<'_>,
    ) {
        // Transform from font units to pixels:
        // - Scale by font_scale in X
        // - Scale by -font_scale in Y (flip Y-up to Y-down)
        // - Translate to (px_x, px_y) — the baseline position
        let transform = Transform::from_row(font_scale, 0.0, 0.0, -font_scale, px_x, px_y);

        pixmap.fill_path(path, paint, FillRule::Winding, transform, None);
    }

    /// Resolve a TextFont to a text font role key ("sans", "serif-bold-italic", etc.).
    fn text_font_role(font: &TextFont) -> String {
        let family = if is_serif_font(&font.name) {
            "serif"
        } else {
            "sans"
        };
        match (font.bold, font.italic) {
            (false, false) => family.to_string(),
            (true, false) => format!("{}-bold", family),
            (false, true) => format!("{}-italic", family),
            (true, true) => format!("{}-bold-italic", family),
        }
    }
}

// ── MusicRenderer implementation ────────────────────────────────────────

impl MusicRenderer for BitmapRenderer {
    // ================== Line Drawing ==================

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        self.page_has_content = true;
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x0), self.py(y0));
        pb.line_to(self.px(x1), self.py(y1));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(width), LineCap::Round);
        }
    }

    fn line_vertical_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        let half_w = width / 2.0;
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x0), self.py(y0 - half_w));
        pb.line_to(self.px(x1), self.py(y1 - half_w));
        pb.line_to(self.px(x1), self.py(y1 + half_w));
        pb.line_to(self.px(x0), self.py(y0 + half_w));
        pb.close();
        if let Some(path) = pb.finish() {
            self.fill_path(&path);
        }
    }

    fn line_horizontal_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        let half_w = width / 2.0;
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x0 - half_w), self.py(y0));
        pb.line_to(self.px(x1 - half_w), self.py(y1));
        pb.line_to(self.px(x1 + half_w), self.py(y1));
        pb.line_to(self.px(x0 + half_w), self.py(y0));
        pb.close();
        if let Some(path) = pb.finish() {
            self.fill_path(&path);
        }
    }

    fn hdashed_line(&mut self, x0: f32, y: f32, x1: f32, width: f32, dash_len: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x0), self.py(y));
        pb.line_to(self.px(x1), self.py(y));
        if let Some(path) = pb.finish() {
            let paint = self.make_paint();
            let pw = self.pw(width);
            let pd = self.pw(dash_len);
            let mut stroke = self.make_stroke(pw, LineCap::Butt);
            stroke.dash = tiny_skia::StrokeDash::new(vec![pd, pd], 0.0);
            if let Some(ref mut pixmap) = self.current_pixmap {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    fn vdashed_line(&mut self, x: f32, y0: f32, y1: f32, width: f32, dash_len: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y0));
        pb.line_to(self.px(x), self.py(y1));
        if let Some(path) = pb.finish() {
            let paint = self.make_paint();
            let pw = self.pw(width);
            let pd = self.pw(dash_len);
            let mut stroke = self.make_stroke(pw, LineCap::Butt);
            stroke.dash = tiny_skia::StrokeDash::new(vec![pd, pd], 0.0);
            if let Some(ref mut pixmap) = self.current_pixmap {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    fn frame_rect(&mut self, rect: &RenderRect, width: f32) {
        let x = self.px(rect.x);
        let y = self.py(rect.y);
        let w = self.pw(rect.width);
        let h = self.pw(rect.height); // use pw for consistency
        if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
            let mut pb = PathBuilder::new();
            pb.push_rect(r);
            if let Some(path) = pb.finish() {
                self.stroke_path(&path, self.pw(width), LineCap::Butt);
            }
        }
    }

    // ================== Staff & Bars ==================

    fn staff_line(&mut self, height_y: f32, x0: f32, x1: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x0), self.py(height_y));
        pb.line_to(self.px(x1), self.py(height_y));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(self.state.staff_lw), LineCap::Butt);
        }
    }

    fn staff(&mut self, height_y: f32, x0: f32, x1: f32, n_lines: u8, line_spacing: f32) {
        self.page_has_content = true;
        for i in 0..n_lines {
            let y = height_y + (i as f32) * line_spacing;
            self.staff_line(y, x0, x1);
        }
    }

    fn bar_line(
        &mut self,
        top_y: f32,
        bottom_y: f32,
        x: f32,
        bar_type: BarLineType,
        line_space: f32,
    ) {
        self.page_has_content = true;
        let inter = (line_space / 2.0).max(2.0);
        let thick_w = (line_space / 2.0).max(1.5);
        let blw = self.state.bar_lw;

        match bar_type {
            BarLineType::Single => {
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(x), self.py(top_y));
                pb.line_to(self.px(x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(blw), LineCap::Butt);
                }
            }
            BarLineType::Double => {
                for dx in [0.0, inter] {
                    let mut pb = PathBuilder::new();
                    pb.move_to(self.px(x + dx), self.py(top_y));
                    pb.line_to(self.px(x + dx), self.py(bottom_y));
                    if let Some(path) = pb.finish() {
                        self.stroke_path(&path, self.pw(blw), LineCap::Butt);
                    }
                }
            }
            BarLineType::FinalDouble => {
                // Thin bar
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(x), self.py(top_y));
                pb.line_to(self.px(x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(blw), LineCap::Butt);
                }
                // Thick bar
                let thick_x = x + inter + thick_w / 2.0;
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(thick_x), self.py(top_y));
                pb.line_to(self.px(thick_x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(thick_w), LineCap::Butt);
                }
            }
            BarLineType::RepeatLeft => {
                // Thick bar at x
                let thick_x = x - thick_w / 2.0;
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(thick_x), self.py(top_y));
                pb.line_to(self.px(thick_x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(thick_w), LineCap::Butt);
                }
                // Thin bar
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(x + inter), self.py(top_y));
                pb.line_to(self.px(x + inter), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(blw), LineCap::Butt);
                }
                // Dots
                let dot_x = x + inter + 0.4 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_x, line_space);
            }
            BarLineType::RepeatRight => {
                // Thin bar at x
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(x), self.py(top_y));
                pb.line_to(self.px(x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(blw), LineCap::Butt);
                }
                // Thick bar
                let thick_x = x + inter + thick_w / 2.0;
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(thick_x), self.py(top_y));
                pb.line_to(self.px(thick_x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(thick_w), LineCap::Butt);
                }
                // Dots left of thin bar
                let dot_x = x - 0.8 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_x, line_space);
            }
            BarLineType::RepeatBoth => {
                let tw = thick_w * 0.7;
                // Left thick bar
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(x), self.py(top_y));
                pb.line_to(self.px(x), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(tw), LineCap::Butt);
                }
                // Right thick bar
                let x2 = x + inter + tw / 4.0;
                let mut pb = PathBuilder::new();
                pb.move_to(self.px(x2), self.py(top_y));
                pb.line_to(self.px(x2), self.py(bottom_y));
                if let Some(path) = pb.finish() {
                    self.stroke_path(&path, self.pw(tw), LineCap::Butt);
                }
                // Both-side dots
                let dot_xl = x + inter + 0.4 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_xl, line_space);
                let dot_xr = x - 0.8 * line_space;
                self.draw_repeat_dots(top_y, bottom_y, dot_xr, line_space);
            }
        }
    }

    fn connector_line(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(top_y));
        pb.line_to(self.px(x), self.py(bottom_y));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(self.state.bar_lw), LineCap::Butt);
        }
    }

    fn ledger_line(&mut self, height_y: f32, x_center: f32, half_width: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x_center - half_width), self.py(height_y));
        pb.line_to(self.px(x_center + half_width), self.py(height_y));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(self.state.ledger_lw), LineCap::Butt);
        }
    }

    fn repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        let line_space = (bottom_y - top_y) / 4.0;
        self.draw_repeat_dots(top_y, bottom_y, x, line_space);
    }

    // ================== Musical Elements ==================

    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, up0: bool, up1: bool) {
        let offset0 = if up0 { thickness } else { 0.0 };
        let offset1 = if up1 { thickness } else { 0.0 };

        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x0), self.py(y0 - offset0 + thickness));
        pb.line_to(self.px(x1), self.py(y1 - offset1 + thickness));
        pb.line_to(self.px(x1), self.py(y1 - offset1));
        pb.line_to(self.px(x0), self.py(y0 - offset0));
        pb.close();
        if let Some(path) = pb.finish() {
            self.fill_path(&path);
        }
    }

    fn slur(&mut self, p0: Point, c1: Point, c2: Point, p3: Point, dashed: bool) {
        if dashed {
            let mut pb = PathBuilder::new();
            pb.move_to(self.px(p0.x), self.py(p0.y));
            pb.cubic_to(
                self.px(c1.x),
                self.py(c1.y),
                self.px(c2.x),
                self.py(c2.y),
                self.px(p3.x),
                self.py(p3.y),
            );
            if let Some(path) = pb.finish() {
                let paint = self.make_paint();
                let pw = self.pw(0.8);
                let d = self.pw(3.0);
                let mut stroke = self.make_stroke(pw, LineCap::Round);
                stroke.dash = tiny_skia::StrokeDash::new(vec![d, d], 0.0);
                if let Some(ref mut pixmap) = self.current_pixmap {
                    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                }
            }
        } else {
            // Solid slur: filled region between two offset Bezier curves
            let ln_space = self.state.music_size / 4.0;
            let slur_mid_lw: f32 = 30.0;
            let up = c1.y < p0.y;
            let thick = if up {
                slur_mid_lw * ln_space / 100.0
            } else {
                -(slur_mid_lw * ln_space / 100.0)
            };

            let mut pb = PathBuilder::new();
            // Outer curve
            pb.move_to(self.px(p0.x), self.py(p0.y));
            pb.cubic_to(
                self.px(c1.x),
                self.py(c1.y),
                self.px(c2.x),
                self.py(c2.y),
                self.px(p3.x),
                self.py(p3.y),
            );
            // Inner curve (reversed, control points offset by thick)
            pb.cubic_to(
                self.px(c2.x),
                self.py(c2.y + thick),
                self.px(c1.x),
                self.py(c1.y + thick),
                self.px(p0.x),
                self.py(p0.y),
            );
            pb.close();
            if let Some(path) = pb.finish() {
                self.fill_path(&path);
            }
        }
    }

    fn bracket(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        // Vertical line
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y_top));
        pb.line_to(self.px(x), self.py(y_bottom));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(2.0), LineCap::Butt);
        }
        // Top serif
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y_top));
        pb.line_to(self.px(x + 4.0), self.py(y_top));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(1.5), LineCap::Butt);
        }
        // Bottom serif
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y_bottom));
        pb.line_to(self.px(x + 4.0), self.py(y_bottom));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(1.5), LineCap::Butt);
        }
    }

    fn brace(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        let mid_y = (y_top + y_bottom) / 2.0;
        let depth = 6.0;

        // Upper half
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y_top));
        pb.cubic_to(
            self.px(x - depth),
            self.py(y_top + (mid_y - y_top) * 0.3),
            self.px(x - depth),
            self.py(mid_y - (mid_y - y_top) * 0.3),
            self.px(x - depth * 1.5),
            self.py(mid_y),
        );
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(1.5), LineCap::Round);
        }
        // Lower half
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x - depth * 1.5), self.py(mid_y));
        pb.cubic_to(
            self.px(x - depth),
            self.py(mid_y + (y_bottom - mid_y) * 0.3),
            self.px(x - depth),
            self.py(y_bottom - (y_bottom - mid_y) * 0.3),
            self.px(x),
            self.py(y_bottom),
        );
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(1.5), LineCap::Round);
        }
    }

    fn note_stem(&mut self, x: f32, y_top: f32, y_bottom: f32, width: f32) {
        self.page_has_content = true;
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y_top));
        pb.line_to(self.px(x), self.py(y_bottom));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(width), LineCap::Butt);
        }
    }

    // ================== Characters & Text ==================

    fn music_char(&mut self, x: f32, y: f32, glyph: MusicGlyph, size_percent: f32) {
        self.page_has_content = true;
        let codepoint = match glyph {
            MusicGlyph::Smufl(cp) => cp,
            MusicGlyph::Sonata(_) => return,
        };

        if let Some(ref mut font) = self.music_font {
            if let Some(gid) = font.glyph_id(codepoint) {
                if let Some(glyph_path) = font.get_glyph_path(gid) {
                    let font_size = self.state.music_size * size_percent / 100.0;
                    let upm = font.units_per_em as f32;
                    let font_scale = font_size / upm * self.state.scale_x * self.scale;
                    let px_x = self.px(x);
                    let px_y = self.py(y);
                    let paint = self.make_paint();
                    if let Some(ref mut pixmap) = self.current_pixmap {
                        Self::render_glyph(pixmap, &glyph_path, font_scale, px_x, px_y, &paint);
                    }
                    return;
                }
            }
        }

        // Fallback: placeholder square
        let half = 3.0;
        let r = tiny_skia::Rect::from_xywh(
            self.px(x - half),
            self.py(y - half),
            self.pw(half * 2.0),
            self.pw(half * 2.0),
        );
        if let Some(rect) = r {
            let paint = self.make_paint();
            if let Some(ref mut pixmap) = self.current_pixmap {
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
    }

    fn music_string(&mut self, x: f32, y: f32, glyphs: &[MusicGlyph], size_percent: f32) {
        let spacing = self.state.music_size * 0.6;
        for (i, glyph) in glyphs.iter().enumerate() {
            self.music_char(x + i as f32 * spacing, y, *glyph, size_percent);
        }
    }

    fn text_string(&mut self, x: f32, y: f32, text: &str, font: &TextFont) {
        let role = Self::text_font_role(font);
        let font_size = font.size.max(4.0);

        // Pre-compute values before borrowing text_fonts mutably
        let cursor_x_start = self.px(x);
        let baseline_y = self.py(y);
        let paint = self.make_paint();
        let sx = self.state.scale_x;
        let dpi_scale = self.scale;

        // Try loaded text font first
        if let Some(loaded) = self.text_fonts.get_mut(&role) {
            let upm = loaded.units_per_em as f32;
            let font_scale = font_size / upm * sx * dpi_scale;
            let mut cursor_x = cursor_x_start;

            for ch in text.chars() {
                let cp = ch as u32;
                if let Some(gid) = loaded.glyph_id(cp) {
                    if let Some(glyph_path) = loaded.get_glyph_path(gid) {
                        if let Some(ref mut pixmap) = self.current_pixmap {
                            Self::render_glyph(
                                pixmap,
                                &glyph_path,
                                font_scale,
                                cursor_x,
                                baseline_y,
                                &paint,
                            );
                        }
                    }
                    let advance = loaded.advance_width(gid);
                    cursor_x += advance as f32 * font_scale;
                }
            }
            return;
        }

        // Fallback: no text font loaded — render thin underline as placeholder.
        // This makes text positions visible without actual glyphs.
        let approx_width = text.len() as f32 * font_size * 0.5; // rough estimate
        let mut pb = PathBuilder::new();
        pb.move_to(self.px(x), self.py(y + 1.0));
        pb.line_to(self.px(x + approx_width), self.py(y + 1.0));
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, self.pw(0.5), LineCap::Butt);
        }
    }

    fn music_colon(&mut self, x: f32, y: f32, _size_percent: f32, line_space: f32) {
        let dot_radius = 1.2;
        self.draw_circle(x, y - line_space * 0.5, dot_radius);
        self.draw_circle(x, y + line_space * 0.5, dot_radius);
    }

    // ================== Configuration ==================

    fn set_line_width(&mut self, width: f32) {
        self.state.line_width = width;
    }

    fn set_widths(&mut self, staff: f32, ledger: f32, stem: f32, bar: f32) {
        self.state.staff_lw = staff;
        self.state.ledger_lw = ledger;
        self.state.stem_lw = stem;
        self.state.bar_lw = bar;
    }

    fn set_music_size(&mut self, point_size: f32) {
        self.state.music_size = point_size;
    }

    fn set_page_size(&mut self, width: f32, height: f32) {
        self.page_width_pt = width;
        self.page_height_pt = height;
    }

    // ================== Page Management ==================

    fn begin_page(&mut self, _page_num: u32) {
        // Save current page if it has content
        if self.in_page && self.page_has_content {
            if let Some(pixmap) = self.current_pixmap.take() {
                self.pages.push(pixmap);
            }
        }

        // Create new pixmap for this page
        let pw = (self.page_width_pt * self.scale).ceil() as u32;
        let ph = (self.page_height_pt * self.scale).ceil() as u32;
        let mut pixmap = Pixmap::new(pw.max(1), ph.max(1)).expect("failed to create pixmap");
        // Fill with white background
        pixmap.fill(tiny_skia::Color::WHITE);

        self.current_pixmap = Some(pixmap);
        self.in_page = true;
        self.page_has_content = false;
        // Reset state for new page (keep fonts and settings, clear transform)
        self.state.translate_x = 0.0;
        self.state.translate_y = 0.0;
        self.state.scale_x = 1.0;
        self.state.scale_y = 1.0;
        self.state_stack.clear();
    }

    fn end_page(&mut self) {
        if self.in_page {
            if let Some(pixmap) = self.current_pixmap.take() {
                self.pages.push(pixmap);
            }
            self.in_page = false;
            self.page_has_content = false;
        }
    }

    // ================== State Management ==================

    fn save_state(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    fn restore_state(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.state = state;
        }
    }

    fn translate(&mut self, dx: f32, dy: f32) {
        self.state.translate_x += dx;
        self.state.translate_y += dy;
    }

    fn scale(&mut self, sx: f32, sy: f32) {
        self.state.scale_x *= sx;
        self.state.scale_y *= sy;
    }

    fn set_color(&mut self, color: Color) {
        self.state.color = color;
    }
}

// ── Font classification (shared with pdf_renderer) ──────────────────────

/// Classify a font name as serif or sans-serif.
fn is_serif_font(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
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
    true
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap_renderer_creates_page() {
        let mut r = BitmapRenderer::new(72.0);
        r.set_page_size(200.0, 100.0);
        r.begin_page(1);
        r.staff(20.0, 10.0, 190.0, 5, 8.0);
        r.end_page();

        assert_eq!(r.page_count(), 1);
        let (w, h) = r.page_dimensions(0).unwrap();
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }

    #[test]
    fn test_bitmap_renderer_multipage() {
        let mut r = BitmapRenderer::new(72.0);
        r.set_page_size(100.0, 50.0);
        r.begin_page(1);
        r.staff(10.0, 5.0, 95.0, 5, 6.0);
        r.begin_page(2);
        r.staff(10.0, 5.0, 95.0, 5, 6.0);
        r.end_page();

        assert_eq!(r.page_count(), 2);
    }

    #[test]
    fn test_bitmap_renderer_geometric_primitives() {
        let mut r = BitmapRenderer::new(72.0);
        r.set_page_size(200.0, 200.0);
        r.begin_page(1);

        r.line(10.0, 10.0, 190.0, 10.0, 1.0);
        r.line_vertical_thick(10.0, 50.0, 190.0, 55.0, 4.0);
        r.line_horizontal_thick(10.0, 80.0, 190.0, 85.0, 4.0);
        r.note_stem(50.0, 20.0, 60.0, 0.8);
        r.beam(60.0, 30.0, 120.0, 35.0, 3.5, true, true);
        r.bar_line(20.0, 60.0, 150.0, BarLineType::Single, 8.0);
        r.connector_line(20.0, 60.0, 10.0);
        r.ledger_line(70.0, 100.0, 5.0);

        r.end_page();
        assert_eq!(r.page_count(), 1);

        // Verify pixels were actually drawn (not all white)
        let data = r.page_data(0).unwrap();
        let non_white = data
            .chunks(4)
            .filter(|px| px[0] < 255 || px[1] < 255 || px[2] < 255)
            .count();
        assert!(
            non_white > 100,
            "Expected visible drawing, got {} non-white pixels",
            non_white
        );
    }

    #[test]
    fn test_bitmap_renderer_state_management() {
        let mut r = BitmapRenderer::new(72.0);
        r.set_page_size(100.0, 100.0);
        r.begin_page(1);

        r.set_color(Color::rgb(1.0, 0.0, 0.0));
        r.save_state();
        r.set_color(Color::rgb(0.0, 1.0, 0.0));
        r.translate(10.0, 10.0);
        assert_eq!(r.state.color.g, 1.0);
        assert_eq!(r.state.translate_x, 10.0);

        r.restore_state();
        assert_eq!(r.state.color.r, 1.0);
        assert_eq!(r.state.color.g, 0.0);
        assert_eq!(r.state.translate_x, 0.0);

        r.end_page();
    }
}
