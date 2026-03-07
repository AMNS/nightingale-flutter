//! Core rendering types for the Nightingale rendering abstraction.
//!
//! These types form the vocabulary of the MusicRenderer trait, providing
//! platform-agnostic descriptions of musical elements to be drawn.
//!
//! Reference: PS_Stdio.cp (PostScript output primitives)

use crate::basic_types::Ddist;

/// RGBA color with floating-point components (0.0-1.0 range).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Create a new color from RGBA components.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create a color from RGB components with full opacity.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Black color (0, 0, 0, 1).
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);

    /// White color (1, 1, 1, 1).
    pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

    /// Gray color (0.5, 0.5, 0.5, 1).
    pub const GRAY: Color = Color::new(0.5, 0.5, 0.5, 1.0);

    /// Light gray color (0.75, 0.75, 0.75, 1).
    pub const LIGHT_GRAY: Color = Color::new(0.75, 0.75, 0.75, 1.0);

    /// Transparent color (0, 0, 0, 0).
    pub const TRANSPARENT: Color = Color::new(0.0, 0.0, 0.0, 0.0);
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Point in rendering coordinates (f32).
///
/// Coordinates are in points (1/72 inch), converted from DDIST before rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Create a new point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Rectangle in rendering coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RenderRect {
    /// Create a new rectangle.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Stroke style for line drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color,
    pub dash_pattern: Option<Vec<f32>>,
}

impl Stroke {
    /// Create a solid stroke with the given width and color.
    pub fn solid(width: f32, color: Color) -> Self {
        Self {
            width,
            color,
            dash_pattern: None,
        }
    }

    /// Create a dashed stroke with the given width, color, and dash pattern.
    pub fn dashed(width: f32, color: Color, dash_pattern: Vec<f32>) -> Self {
        Self {
            width,
            color,
            dash_pattern: Some(dash_pattern),
        }
    }
}

/// Bar line types (from defs.h).
///
/// Reference: defs.h, lines ~400-410
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarLineType {
    /// Single bar line (|).
    Single,
    /// Double bar line (||).
    Double,
    /// Final double bar line with thick second line (||).
    FinalDouble,
    /// Repeat sign opening (|:).
    RepeatLeft,
    /// Repeat sign closing (:|).
    RepeatRight,
    /// Repeat both directions (:|:).
    RepeatBoth,
    /// Dotted bar line (PSM_DOTTED pseudo-measure).
    Dotted,
}

/// Music glyph identifier.
///
/// Can represent either a SMuFL Unicode code point (U+E000-U+F8FF private use area)
/// or a legacy Sonata character code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicGlyph {
    /// SMuFL Unicode code point (e.g., U+E050 for gClef).
    Smufl(u32),
    /// Legacy Sonata character code (e.g., '&' for treble clef).
    Sonata(u8),
}

impl MusicGlyph {
    /// Create a SMuFL glyph from a Unicode code point.
    pub const fn smufl(code_point: u32) -> Self {
        Self::Smufl(code_point)
    }

    /// Create a Sonata glyph from a character code.
    pub const fn sonata(char_code: u8) -> Self {
        Self::Sonata(char_code)
    }

    /// Create a Sonata glyph from an ASCII character.
    pub const fn from_char(ch: char) -> Self {
        Self::Sonata(ch as u8)
    }
}

/// Font specification for text (non-music) drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct TextFont {
    pub name: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
}

impl TextFont {
    /// Create a new text font.
    pub fn new(name: impl Into<String>, size: f32) -> Self {
        Self {
            name: name.into(),
            size,
            bold: false,
            italic: false,
        }
    }

    /// Set the bold flag.
    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Set the italic flag.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
}

/// Convert DDIST to rendering coordinates (points, f32).
///
/// DDIST has 1/16 point resolution. This function converts to floating-point points.
///
/// Reference: PS_Stdio.cp, lines 48, 1351-1356
/// Original formula: `d2pt(d) = (d+8)>>4` (integer division with rounding)
/// Float version: `d / 16.0` (no rounding offset needed for float conversion)
pub fn ddist_to_render(d: Ddist) -> f32 {
    d as f32 / 16.0
}

/// Like `ddist_to_render` but takes i32 to handle intermediate sums that
/// may exceed the i16 (Ddist) range. Used when adding staff_top + offset
/// for scores with many systems.
pub fn ddist_wide_to_render(d: i32) -> f32 {
    d as f32 / 16.0
}

/// Convert rendering coordinates (points, f32) to DDIST.
///
/// Inverse of `ddist_to_render`. Rounds to nearest DDIST value.
pub fn render_to_ddist(p: f32) -> Ddist {
    (p * 16.0).round() as Ddist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK, Color::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(Color::WHITE, Color::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(Color::GRAY, Color::new(0.5, 0.5, 0.5, 1.0));
        assert_eq!(Color::LIGHT_GRAY, Color::new(0.75, 0.75, 0.75, 1.0));
        assert_eq!(Color::TRANSPARENT, Color::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_color_default() {
        assert_eq!(Color::default(), Color::BLACK);
    }

    #[test]
    fn test_color_rgb() {
        let color = Color::rgb(0.1, 0.2, 0.3);
        assert_eq!(color.r, 0.1);
        assert_eq!(color.g, 0.2);
        assert_eq!(color.b, 0.3);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_point() {
        let p = Point::new(10.0, 20.0);
        assert_eq!(p.x, 10.0);
        assert_eq!(p.y, 20.0);
    }

    #[test]
    fn test_render_rect() {
        let r = RenderRect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 20.0);
        assert_eq!(r.width, 30.0);
        assert_eq!(r.height, 40.0);
    }

    #[test]
    fn test_stroke_solid() {
        let stroke = Stroke::solid(2.0, Color::BLACK);
        assert_eq!(stroke.width, 2.0);
        assert_eq!(stroke.color, Color::BLACK);
        assert_eq!(stroke.dash_pattern, None);
    }

    #[test]
    fn test_stroke_dashed() {
        let stroke = Stroke::dashed(2.0, Color::BLACK, vec![4.0, 2.0]);
        assert_eq!(stroke.width, 2.0);
        assert_eq!(stroke.color, Color::BLACK);
        assert_eq!(stroke.dash_pattern, Some(vec![4.0, 2.0]));
    }

    #[test]
    fn test_music_glyph() {
        let smufl = MusicGlyph::smufl(0xE050);
        assert_eq!(smufl, MusicGlyph::Smufl(0xE050));

        let sonata = MusicGlyph::sonata(b'&');
        assert_eq!(sonata, MusicGlyph::Sonata(b'&'));

        let from_char = MusicGlyph::from_char('&');
        assert_eq!(from_char, MusicGlyph::Sonata(b'&'));
    }

    #[test]
    fn test_text_font() {
        let font = TextFont::new("Times New Roman", 12.0);
        assert_eq!(font.name, "Times New Roman");
        assert_eq!(font.size, 12.0);
        assert!(!font.bold);
        assert!(!font.italic);

        let font_bold = font.clone().bold(true);
        assert!(font_bold.bold);

        let font_italic = font.clone().italic(true);
        assert!(font_italic.italic);
    }

    #[test]
    fn test_ddist_to_render() {
        // DDIST 0 → 0.0 points
        assert_eq!(ddist_to_render(0), 0.0);

        // DDIST 16 → 1.0 points (16/16 = 1)
        assert_eq!(ddist_to_render(16), 1.0);

        // DDIST 160 → 10.0 points (160/16 = 10)
        assert_eq!(ddist_to_render(160), 10.0);

        // DDIST -16 → -1.0 points
        assert_eq!(ddist_to_render(-16), -1.0);
    }

    #[test]
    fn test_render_to_ddist() {
        // Round-trip test
        let ddist = 160; // 10 points
        let render = ddist_to_render(ddist);
        let back = render_to_ddist(render);
        assert_eq!(back, ddist);
    }

    #[test]
    fn test_ddist_render_roundtrip() {
        // Test various DDIST values
        for ddist in &[0, 16, 32, 160, -16, -32, 1000, -1000] {
            let render = ddist_to_render(*ddist);
            let back = render_to_ddist(render);
            assert_eq!(back, *ddist);
        }
    }
}
