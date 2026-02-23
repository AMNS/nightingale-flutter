//! Serializable rendering commands.
//!
//! The `RenderCommand` enum captures all rendering operations in a serializable form.
//! This is useful for:
//! - Testing (record and verify draw calls)
//! - Flutter bridge (serialize and send to Dart)
//! - Debugging (print command stream)
//! - Replay (record once, render many times)

use super::types::{BarLineType, Color, MusicGlyph, Point, RenderRect, TextFont};

/// A single rendering command.
///
/// Each variant corresponds to a method in the `MusicRenderer` trait.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderCommand {
    // ================== Line Drawing ==================
    /// Draw a line with perpendicular thickening.
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
    },

    /// Draw a line with vertical thickening.
    LineVerticalThick {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
    },

    /// Draw a line with horizontal thickening.
    LineHorizontalThick {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
    },

    /// Draw a horizontal dashed line.
    HDashedLine {
        x0: f32,
        y: f32,
        x1: f32,
        width: f32,
        dash_len: f32,
    },

    /// Draw a vertical dashed line.
    VDashedLine {
        x: f32,
        y0: f32,
        y1: f32,
        width: f32,
        dash_len: f32,
    },

    /// Draw a rectangle outline.
    FrameRect { rect: RenderRect, width: f32 },

    // ================== Staff & Bars ==================
    /// Draw a single staff line.
    StaffLine { y: f32, x0: f32, x1: f32 },

    /// Draw a complete N-line staff.
    Staff {
        y: f32,
        x0: f32,
        x1: f32,
        n_lines: u8,
        line_spacing: f32,
    },

    /// Draw a bar line.
    BarLine {
        top: f32,
        bottom: f32,
        x: f32,
        bar_type: BarLineType,
    },

    /// Draw a system connector line.
    ConnectorLine { top: f32, bottom: f32, x: f32 },

    /// Draw a ledger line.
    LedgerLine {
        y: f32,
        x_center: f32,
        half_width: f32,
    },

    /// Draw repeat dots.
    RepeatDots { top: f32, bottom: f32, x: f32 },

    // ================== Musical Elements ==================
    /// Draw a beam segment.
    Beam {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thickness: f32,
        up0: bool,
        up1: bool,
    },

    /// Draw a slur or tie.
    Slur {
        p0: Point,
        c1: Point,
        c2: Point,
        p3: Point,
        dashed: bool,
    },

    /// Draw a system bracket.
    Bracket { x: f32, y_top: f32, y_bottom: f32 },

    /// Draw a system brace.
    Brace { x: f32, y_top: f32, y_bottom: f32 },

    /// Draw a note stem.
    NoteStem {
        x: f32,
        y_top: f32,
        y_bottom: f32,
        width: f32,
    },

    // ================== Characters & Text ==================
    /// Draw a single music glyph.
    MusicChar {
        x: f32,
        y: f32,
        glyph: MusicGlyph,
        size_percent: f32,
    },

    /// Draw a string of music glyphs.
    MusicString {
        x: f32,
        y: f32,
        glyphs: Vec<MusicGlyph>,
        size_percent: f32,
    },

    /// Draw a text string.
    TextString {
        x: f32,
        y: f32,
        text: String,
        font: TextFont,
    },

    /// Draw a music colon.
    MusicColon {
        x: f32,
        y: f32,
        size_percent: f32,
        line_space: f32,
    },

    // ================== Configuration ==================
    /// Set the default line width.
    SetLineWidth(f32),

    /// Set various line widths.
    SetWidths {
        staff: f32,
        ledger: f32,
        stem: f32,
        bar: f32,
    },

    /// Set the music font size.
    SetMusicSize(f32),

    /// Set the page size.
    SetPageSize { width: f32, height: f32 },

    // ================== Page Management ==================
    /// Begin a new page.
    BeginPage(u32),

    /// End the current page.
    EndPage,

    // ================== State Management ==================
    /// Save the graphics state.
    SaveState,

    /// Restore the graphics state.
    RestoreState,

    /// Apply a translation.
    Translate { dx: f32, dy: f32 },

    /// Apply a scale.
    Scale { sx: f32, sy: f32 },

    /// Set the drawing color.
    SetColor(Color),
}

impl RenderCommand {
    /// Get a human-readable name for this command (for debugging).
    pub fn name(&self) -> &'static str {
        match self {
            RenderCommand::Line { .. } => "Line",
            RenderCommand::LineVerticalThick { .. } => "LineVerticalThick",
            RenderCommand::LineHorizontalThick { .. } => "LineHorizontalThick",
            RenderCommand::HDashedLine { .. } => "HDashedLine",
            RenderCommand::VDashedLine { .. } => "VDashedLine",
            RenderCommand::FrameRect { .. } => "FrameRect",
            RenderCommand::StaffLine { .. } => "StaffLine",
            RenderCommand::Staff { .. } => "Staff",
            RenderCommand::BarLine { .. } => "BarLine",
            RenderCommand::ConnectorLine { .. } => "ConnectorLine",
            RenderCommand::LedgerLine { .. } => "LedgerLine",
            RenderCommand::RepeatDots { .. } => "RepeatDots",
            RenderCommand::Beam { .. } => "Beam",
            RenderCommand::Slur { .. } => "Slur",
            RenderCommand::Bracket { .. } => "Bracket",
            RenderCommand::Brace { .. } => "Brace",
            RenderCommand::NoteStem { .. } => "NoteStem",
            RenderCommand::MusicChar { .. } => "MusicChar",
            RenderCommand::MusicString { .. } => "MusicString",
            RenderCommand::TextString { .. } => "TextString",
            RenderCommand::MusicColon { .. } => "MusicColon",
            RenderCommand::SetLineWidth(_) => "SetLineWidth",
            RenderCommand::SetWidths { .. } => "SetWidths",
            RenderCommand::SetMusicSize(_) => "SetMusicSize",
            RenderCommand::SetPageSize { .. } => "SetPageSize",
            RenderCommand::BeginPage(_) => "BeginPage",
            RenderCommand::EndPage => "EndPage",
            RenderCommand::SaveState => "SaveState",
            RenderCommand::RestoreState => "RestoreState",
            RenderCommand::Translate { .. } => "Translate",
            RenderCommand::Scale { .. } => "Scale",
            RenderCommand::SetColor(_) => "SetColor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_name() {
        let cmd = RenderCommand::StaffLine {
            y: 100.0,
            x0: 0.0,
            x1: 200.0,
        };
        assert_eq!(cmd.name(), "StaffLine");

        let cmd = RenderCommand::BeginPage(1);
        assert_eq!(cmd.name(), "BeginPage");

        let cmd = RenderCommand::SetColor(Color::BLACK);
        assert_eq!(cmd.name(), "SetColor");
    }

    #[test]
    fn test_command_clone() {
        let cmd = RenderCommand::Line {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 100.0,
            width: 2.0,
        };
        let cmd2 = cmd.clone();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_command_debug() {
        let cmd = RenderCommand::Staff {
            y: 100.0,
            x0: 0.0,
            x1: 500.0,
            n_lines: 5,
            line_spacing: 8.0,
        };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Staff"));
        assert!(debug_str.contains("100.0"));
    }
}
