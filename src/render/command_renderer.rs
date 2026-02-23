//! Command-recording renderer implementation.
//!
//! `CommandRenderer` implements `MusicRenderer` by recording commands into a vector.
//! This is useful for:
//! - Testing: verify that the correct draw calls were made
//! - Flutter bridge: serialize commands and send to Dart
//! - Debugging: inspect the command stream
//! - Replay: record once, render many times with different backends

use super::command::RenderCommand;
use super::types::{BarLineType, Color, MusicGlyph, Point, RenderRect, TextFont};
use super::MusicRenderer;

/// A renderer that records commands into a vector.
///
/// # Example
///
/// ```
/// use nightingale_core::render::{CommandRenderer, MusicRenderer, ddist_to_render};
///
/// let mut renderer = CommandRenderer::new();
///
/// // Draw a staff line
/// renderer.staff_line(100.0, 0.0, 500.0);
///
/// // Verify the command was recorded
/// assert_eq!(renderer.commands().len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct CommandRenderer {
    commands: Vec<RenderCommand>,
}

impl CommandRenderer {
    /// Create a new command renderer.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Get a reference to the recorded commands.
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }

    /// Get a mutable reference to the recorded commands.
    pub fn commands_mut(&mut self) -> &mut Vec<RenderCommand> {
        &mut self.commands
    }

    /// Take ownership of the recorded commands, leaving an empty vector.
    pub fn take_commands(&mut self) -> Vec<RenderCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Clear all recorded commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Get the number of recorded commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if any commands have been recorded.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Count commands of a specific type.
    pub fn count_commands(&self, name: &str) -> usize {
        self.commands.iter().filter(|c| c.name() == name).count()
    }
}

impl MusicRenderer for CommandRenderer {
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        self.commands.push(RenderCommand::Line {
            x0,
            y0,
            x1,
            y1,
            width,
        });
    }

    fn line_vertical_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        self.commands.push(RenderCommand::LineVerticalThick {
            x0,
            y0,
            x1,
            y1,
            width,
        });
    }

    fn line_horizontal_thick(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        self.commands.push(RenderCommand::LineHorizontalThick {
            x0,
            y0,
            x1,
            y1,
            width,
        });
    }

    fn hdashed_line(&mut self, x0: f32, y: f32, x1: f32, width: f32, dash_len: f32) {
        self.commands.push(RenderCommand::HDashedLine {
            x0,
            y,
            x1,
            width,
            dash_len,
        });
    }

    fn vdashed_line(&mut self, x: f32, y0: f32, y1: f32, width: f32, dash_len: f32) {
        self.commands.push(RenderCommand::VDashedLine {
            x,
            y0,
            y1,
            width,
            dash_len,
        });
    }

    fn frame_rect(&mut self, rect: &RenderRect, width: f32) {
        self.commands
            .push(RenderCommand::FrameRect { rect: *rect, width });
    }

    fn staff_line(&mut self, height_y: f32, x0: f32, x1: f32) {
        self.commands.push(RenderCommand::StaffLine {
            y: height_y,
            x0,
            x1,
        });
    }

    fn staff(&mut self, height_y: f32, x0: f32, x1: f32, n_lines: u8, line_spacing: f32) {
        self.commands.push(RenderCommand::Staff {
            y: height_y,
            x0,
            x1,
            n_lines,
            line_spacing,
        });
    }

    fn bar_line(&mut self, top_y: f32, bottom_y: f32, x: f32, bar_type: BarLineType) {
        self.commands.push(RenderCommand::BarLine {
            top: top_y,
            bottom: bottom_y,
            x,
            bar_type,
        });
    }

    fn connector_line(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        self.commands.push(RenderCommand::ConnectorLine {
            top: top_y,
            bottom: bottom_y,
            x,
        });
    }

    fn ledger_line(&mut self, height_y: f32, x_center: f32, half_width: f32) {
        self.commands.push(RenderCommand::LedgerLine {
            y: height_y,
            x_center,
            half_width,
        });
    }

    fn repeat_dots(&mut self, top_y: f32, bottom_y: f32, x: f32) {
        self.commands.push(RenderCommand::RepeatDots {
            top: top_y,
            bottom: bottom_y,
            x,
        });
    }

    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, up0: bool, up1: bool) {
        self.commands.push(RenderCommand::Beam {
            x0,
            y0,
            x1,
            y1,
            thickness,
            up0,
            up1,
        });
    }

    fn slur(&mut self, p0: Point, c1: Point, c2: Point, p3: Point, dashed: bool) {
        self.commands.push(RenderCommand::Slur {
            p0,
            c1,
            c2,
            p3,
            dashed,
        });
    }

    fn bracket(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        self.commands
            .push(RenderCommand::Bracket { x, y_top, y_bottom });
    }

    fn brace(&mut self, x: f32, y_top: f32, y_bottom: f32) {
        self.commands
            .push(RenderCommand::Brace { x, y_top, y_bottom });
    }

    fn note_stem(&mut self, x: f32, y_top: f32, y_bottom: f32, width: f32) {
        self.commands.push(RenderCommand::NoteStem {
            x,
            y_top,
            y_bottom,
            width,
        });
    }

    fn music_char(&mut self, x: f32, y: f32, glyph: MusicGlyph, size_percent: f32) {
        self.commands.push(RenderCommand::MusicChar {
            x,
            y,
            glyph,
            size_percent,
        });
    }

    fn music_string(&mut self, x: f32, y: f32, glyphs: &[MusicGlyph], size_percent: f32) {
        self.commands.push(RenderCommand::MusicString {
            x,
            y,
            glyphs: glyphs.to_vec(),
            size_percent,
        });
    }

    fn text_string(&mut self, x: f32, y: f32, text: &str, font: &TextFont) {
        self.commands.push(RenderCommand::TextString {
            x,
            y,
            text: text.to_string(),
            font: font.clone(),
        });
    }

    fn music_colon(&mut self, x: f32, y: f32, size_percent: f32, line_space: f32) {
        self.commands.push(RenderCommand::MusicColon {
            x,
            y,
            size_percent,
            line_space,
        });
    }

    fn set_line_width(&mut self, width: f32) {
        self.commands.push(RenderCommand::SetLineWidth(width));
    }

    fn set_widths(&mut self, staff: f32, ledger: f32, stem: f32, bar: f32) {
        self.commands.push(RenderCommand::SetWidths {
            staff,
            ledger,
            stem,
            bar,
        });
    }

    fn set_music_size(&mut self, point_size: f32) {
        self.commands.push(RenderCommand::SetMusicSize(point_size));
    }

    fn set_page_size(&mut self, width: f32, height: f32) {
        self.commands
            .push(RenderCommand::SetPageSize { width, height });
    }

    fn begin_page(&mut self, page_num: u32) {
        self.commands.push(RenderCommand::BeginPage(page_num));
    }

    fn end_page(&mut self) {
        self.commands.push(RenderCommand::EndPage);
    }

    fn save_state(&mut self) {
        self.commands.push(RenderCommand::SaveState);
    }

    fn restore_state(&mut self) {
        self.commands.push(RenderCommand::RestoreState);
    }

    fn translate(&mut self, dx: f32, dy: f32) {
        self.commands.push(RenderCommand::Translate { dx, dy });
    }

    fn scale(&mut self, sx: f32, sy: f32) {
        self.commands.push(RenderCommand::Scale { sx, sy });
    }

    fn set_color(&mut self, color: Color) {
        self.commands.push(RenderCommand::SetColor(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_renderer() {
        let renderer = CommandRenderer::new();
        assert_eq!(renderer.len(), 0);
        assert!(renderer.is_empty());
    }

    #[test]
    fn test_staff_line() {
        let mut renderer = CommandRenderer::new();
        renderer.staff_line(100.0, 0.0, 500.0);

        assert_eq!(renderer.len(), 1);
        assert_eq!(
            renderer.commands()[0],
            RenderCommand::StaffLine {
                y: 100.0,
                x0: 0.0,
                x1: 500.0
            }
        );
    }

    #[test]
    fn test_multiple_commands() {
        let mut renderer = CommandRenderer::new();

        renderer.begin_page(1);
        renderer.staff_line(100.0, 0.0, 500.0);
        renderer.staff_line(108.0, 0.0, 500.0);
        renderer.end_page();

        assert_eq!(renderer.len(), 4);
        assert_eq!(renderer.count_commands("BeginPage"), 1);
        assert_eq!(renderer.count_commands("StaffLine"), 2);
        assert_eq!(renderer.count_commands("EndPage"), 1);
    }

    #[test]
    fn test_clear() {
        let mut renderer = CommandRenderer::new();
        renderer.staff_line(100.0, 0.0, 500.0);
        assert_eq!(renderer.len(), 1);

        renderer.clear();
        assert_eq!(renderer.len(), 0);
        assert!(renderer.is_empty());
    }

    #[test]
    fn test_take_commands() {
        let mut renderer = CommandRenderer::new();
        renderer.staff_line(100.0, 0.0, 500.0);
        renderer.staff_line(108.0, 0.0, 500.0);

        let commands = renderer.take_commands();
        assert_eq!(commands.len(), 2);
        assert_eq!(renderer.len(), 0);
    }

    #[test]
    fn test_bar_line() {
        let mut renderer = CommandRenderer::new();
        renderer.bar_line(50.0, 100.0, 200.0, BarLineType::Double);

        assert_eq!(
            renderer.commands()[0],
            RenderCommand::BarLine {
                top: 50.0,
                bottom: 100.0,
                x: 200.0,
                bar_type: BarLineType::Double
            }
        );
    }

    #[test]
    fn test_beam() {
        let mut renderer = CommandRenderer::new();
        renderer.beam(0.0, 100.0, 50.0, 95.0, 3.0, true, false);

        assert_eq!(
            renderer.commands()[0],
            RenderCommand::Beam {
                x0: 0.0,
                y0: 100.0,
                x1: 50.0,
                y1: 95.0,
                thickness: 3.0,
                up0: true,
                up1: false
            }
        );
    }

    #[test]
    fn test_slur() {
        let mut renderer = CommandRenderer::new();
        let p0 = Point::new(0.0, 100.0);
        let c1 = Point::new(20.0, 90.0);
        let c2 = Point::new(80.0, 90.0);
        let p3 = Point::new(100.0, 100.0);

        renderer.slur(p0, c1, c2, p3, false);

        assert_eq!(
            renderer.commands()[0],
            RenderCommand::Slur {
                p0,
                c1,
                c2,
                p3,
                dashed: false
            }
        );
    }

    #[test]
    fn test_music_char() {
        let mut renderer = CommandRenderer::new();
        let glyph = MusicGlyph::sonata(b'&');
        renderer.music_char(100.0, 200.0, glyph, 100.0);

        assert_eq!(
            renderer.commands()[0],
            RenderCommand::MusicChar {
                x: 100.0,
                y: 200.0,
                glyph,
                size_percent: 100.0
            }
        );
    }

    #[test]
    fn test_text_string() {
        let mut renderer = CommandRenderer::new();
        let font = TextFont::new("Arial", 12.0);
        renderer.text_string(50.0, 60.0, "Hello", &font);

        assert_eq!(
            renderer.commands()[0],
            RenderCommand::TextString {
                x: 50.0,
                y: 60.0,
                text: "Hello".to_string(),
                font: font.clone()
            }
        );
    }

    #[test]
    fn test_state_management() {
        let mut renderer = CommandRenderer::new();

        renderer.save_state();
        renderer.translate(10.0, 20.0);
        renderer.scale(1.5, 1.5);
        renderer.set_color(Color::BLACK);
        renderer.restore_state();

        assert_eq!(renderer.len(), 5);
        assert_eq!(renderer.commands()[0], RenderCommand::SaveState);
        assert_eq!(
            renderer.commands()[1],
            RenderCommand::Translate { dx: 10.0, dy: 20.0 }
        );
        assert_eq!(
            renderer.commands()[2],
            RenderCommand::Scale { sx: 1.5, sy: 1.5 }
        );
        assert_eq!(
            renderer.commands()[3],
            RenderCommand::SetColor(Color::BLACK)
        );
        assert_eq!(renderer.commands()[4], RenderCommand::RestoreState);
    }

    #[test]
    fn test_configuration() {
        let mut renderer = CommandRenderer::new();

        renderer.set_line_width(1.5);
        renderer.set_widths(1.0, 0.8, 0.6, 1.2);
        renderer.set_music_size(28.0);
        renderer.set_page_size(612.0, 792.0);

        assert_eq!(renderer.len(), 4);
        assert_eq!(renderer.commands()[0], RenderCommand::SetLineWidth(1.5));
        assert_eq!(
            renderer.commands()[1],
            RenderCommand::SetWidths {
                staff: 1.0,
                ledger: 0.8,
                stem: 0.6,
                bar: 1.2
            }
        );
        assert_eq!(renderer.commands()[2], RenderCommand::SetMusicSize(28.0));
        assert_eq!(
            renderer.commands()[3],
            RenderCommand::SetPageSize {
                width: 612.0,
                height: 792.0
            }
        );
    }

    #[test]
    fn test_page_management() {
        let mut renderer = CommandRenderer::new();

        renderer.begin_page(1);
        renderer.staff_line(100.0, 0.0, 500.0);
        renderer.end_page();
        renderer.begin_page(2);
        renderer.staff_line(100.0, 0.0, 500.0);
        renderer.end_page();

        assert_eq!(renderer.count_commands("BeginPage"), 2);
        assert_eq!(renderer.count_commands("EndPage"), 2);
        assert_eq!(renderer.count_commands("StaffLine"), 2);
    }

    #[test]
    fn test_line_types() {
        let mut renderer = CommandRenderer::new();

        renderer.line(0.0, 0.0, 100.0, 100.0, 2.0);
        renderer.line_vertical_thick(0.0, 0.0, 100.0, 100.0, 2.0);
        renderer.line_horizontal_thick(0.0, 0.0, 100.0, 100.0, 2.0);
        renderer.hdashed_line(0.0, 50.0, 100.0, 1.0, 5.0);
        renderer.vdashed_line(50.0, 0.0, 100.0, 1.0, 5.0);

        assert_eq!(renderer.count_commands("Line"), 1);
        assert_eq!(renderer.count_commands("LineVerticalThick"), 1);
        assert_eq!(renderer.count_commands("LineHorizontalThick"), 1);
        assert_eq!(renderer.count_commands("HDashedLine"), 1);
        assert_eq!(renderer.count_commands("VDashedLine"), 1);
    }
}
