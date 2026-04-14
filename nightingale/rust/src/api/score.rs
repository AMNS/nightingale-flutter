// Bridge API: render NGL/Notelist scores to a flat list of drawing commands.
//
// We use flat DTO structs rather than Rust enums for the FFI boundary.
// Each RenderCommandDto has a `kind` tag (u8) and enough fields to
// represent every variant. Unused fields are zeroed.
//
// This crosses the FFI boundary as a single Vec<RenderCommandDto>,
// which flutter_rust_bridge serialises efficiently.

use nightingale_core::draw::draw_high_level::render_score;
use nightingale_core::ngl::interpret::InterpretedScore;
use nightingale_core::ngl::{interpret_heap, NglFile};
use nightingale_core::render::{BitmapRenderer, CommandRenderer, MusicRenderer, RenderCommand};

/// Safe stderr logging that won't panic on broken pipe (release .app bundles).
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

// ── Tag constants (must match Dart side) ────────────────────────
// 32 command types matching the RenderCommand enum variants.

pub const CMD_LINE: u8 = 1;
pub const CMD_LINE_VERTICAL_THICK: u8 = 2;
pub const CMD_LINE_HORIZONTAL_THICK: u8 = 3;
pub const CMD_HDASHED_LINE: u8 = 4;
pub const CMD_VDASHED_LINE: u8 = 5;
pub const CMD_FRAME_RECT: u8 = 6;
pub const CMD_STAFF_LINE: u8 = 7;
pub const CMD_STAFF: u8 = 8;
pub const CMD_BAR_LINE: u8 = 9;
pub const CMD_CONNECTOR_LINE: u8 = 10;
pub const CMD_LEDGER_LINE: u8 = 11;
pub const CMD_REPEAT_DOTS: u8 = 12;
pub const CMD_BEAM: u8 = 13;
pub const CMD_SLUR: u8 = 14;
pub const CMD_BRACKET: u8 = 15;
pub const CMD_BRACE: u8 = 16;
pub const CMD_NOTE_STEM: u8 = 17;
pub const CMD_MUSIC_CHAR: u8 = 18;
pub const CMD_MUSIC_STRING: u8 = 19;
pub const CMD_TEXT_STRING: u8 = 20;
pub const CMD_MUSIC_COLON: u8 = 21;
pub const CMD_SET_LINE_WIDTH: u8 = 22;
pub const CMD_SET_WIDTHS: u8 = 23;
pub const CMD_SET_MUSIC_SIZE: u8 = 24;
pub const CMD_SET_PAGE_SIZE: u8 = 25;
pub const CMD_BEGIN_PAGE: u8 = 26;
pub const CMD_END_PAGE: u8 = 27;
pub const CMD_SAVE_STATE: u8 = 28;
pub const CMD_RESTORE_STATE: u8 = 29;
pub const CMD_TRANSLATE: u8 = 30;
pub const CMD_SCALE: u8 = 31;
pub const CMD_SET_COLOR: u8 = 32;

// ── DTO types ───────────────────────────────────────────────────

/// Flat DTO for one drawing command.
///
/// `kind` selects the variant. Fields that don't apply are zero/empty.
/// See CMD_* constants for kind values.
#[derive(Debug, Clone, Default)]
pub struct RenderCommandDto {
    pub kind: u8,
    // Coordinates (multi-purpose depending on kind)
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub x3: f64,
    pub y3: f64,
    // Dimensions
    pub width: f64,
    pub height: f64,
    pub thickness: f64,
    pub line_spacing: f64,
    pub size_percent: f64,
    pub font_size: f64,
    pub dash_len: f64,
    // Color (inline to avoid nested struct)
    pub color_r: f64,
    pub color_g: f64,
    pub color_b: f64,
    pub color_a: f64,
    // Glyph / music
    pub glyph_code: u32,
    pub glyph_codes: Vec<u32>,
    pub n_lines: u8,
    pub bar_type: u8,
    pub page_number: u32,
    // Flags
    pub up0: bool,
    pub up1: bool,
    pub dashed: bool,
    pub bold: bool,
    pub italic: bool,
    // Text
    pub text: String,
    pub font_name: String,
}

// ── Conversion ──────────────────────────────────────────────────

fn convert(cmd: &RenderCommand) -> RenderCommandDto {
    use nightingale_core::render::types::{BarLineType, MusicGlyph};

    match cmd {
        RenderCommand::Line {
            x0,
            y0,
            x1,
            y1,
            width,
        } => RenderCommandDto {
            kind: CMD_LINE,
            x0: *x0 as f64,
            y0: *y0 as f64,
            x1: *x1 as f64,
            y1: *y1 as f64,
            width: *width as f64,
            ..Default::default()
        },
        RenderCommand::LineVerticalThick {
            x0,
            y0,
            x1,
            y1,
            width,
        } => RenderCommandDto {
            kind: CMD_LINE_VERTICAL_THICK,
            x0: *x0 as f64,
            y0: *y0 as f64,
            x1: *x1 as f64,
            y1: *y1 as f64,
            width: *width as f64,
            ..Default::default()
        },
        RenderCommand::LineHorizontalThick {
            x0,
            y0,
            x1,
            y1,
            width,
        } => RenderCommandDto {
            kind: CMD_LINE_HORIZONTAL_THICK,
            x0: *x0 as f64,
            y0: *y0 as f64,
            x1: *x1 as f64,
            y1: *y1 as f64,
            width: *width as f64,
            ..Default::default()
        },
        RenderCommand::HDashedLine {
            x0,
            y,
            x1,
            width,
            dash_len,
        } => RenderCommandDto {
            kind: CMD_HDASHED_LINE,
            x0: *x0 as f64,
            y0: *y as f64,
            x1: *x1 as f64,
            width: *width as f64,
            dash_len: *dash_len as f64,
            ..Default::default()
        },
        RenderCommand::VDashedLine {
            x,
            y0,
            y1,
            width,
            dash_len,
        } => RenderCommandDto {
            kind: CMD_VDASHED_LINE,
            x0: *x as f64,
            y0: *y0 as f64,
            y1: *y1 as f64,
            width: *width as f64,
            dash_len: *dash_len as f64,
            ..Default::default()
        },
        RenderCommand::FrameRect { rect, width } => RenderCommandDto {
            kind: CMD_FRAME_RECT,
            x0: rect.x as f64,
            y0: rect.y as f64,
            width: rect.width as f64,
            height: rect.height as f64,
            thickness: *width as f64,
            ..Default::default()
        },
        RenderCommand::StaffLine { y, x0, x1 } => RenderCommandDto {
            kind: CMD_STAFF_LINE,
            y0: *y as f64,
            x0: *x0 as f64,
            x1: *x1 as f64,
            ..Default::default()
        },
        RenderCommand::Staff {
            y,
            x0,
            x1,
            n_lines,
            line_spacing,
        } => RenderCommandDto {
            kind: CMD_STAFF,
            y0: *y as f64,
            x0: *x0 as f64,
            x1: *x1 as f64,
            n_lines: *n_lines,
            line_spacing: *line_spacing as f64,
            ..Default::default()
        },
        RenderCommand::BarLine {
            top,
            bottom,
            x,
            bar_type,
        } => RenderCommandDto {
            kind: CMD_BAR_LINE,
            y0: *top as f64,
            y1: *bottom as f64,
            x0: *x as f64,
            bar_type: match bar_type {
                BarLineType::Single => 0,
                BarLineType::Double => 1,
                BarLineType::FinalDouble => 2,
                BarLineType::RepeatLeft => 3,
                BarLineType::RepeatRight => 4,
                BarLineType::RepeatBoth => 5,
                BarLineType::Dotted => 6,
            },
            ..Default::default()
        },
        RenderCommand::ConnectorLine { top, bottom, x } => RenderCommandDto {
            kind: CMD_CONNECTOR_LINE,
            y0: *top as f64,
            y1: *bottom as f64,
            x0: *x as f64,
            ..Default::default()
        },
        RenderCommand::LedgerLine {
            y,
            x_center,
            half_width,
        } => RenderCommandDto {
            kind: CMD_LEDGER_LINE,
            y0: *y as f64,
            x0: *x_center as f64,
            width: *half_width as f64,
            ..Default::default()
        },
        RenderCommand::RepeatDots { top, bottom, x } => RenderCommandDto {
            kind: CMD_REPEAT_DOTS,
            y0: *top as f64,
            y1: *bottom as f64,
            x0: *x as f64,
            ..Default::default()
        },
        RenderCommand::Beam {
            x0,
            y0,
            x1,
            y1,
            thickness,
            up0,
            up1,
        } => RenderCommandDto {
            kind: CMD_BEAM,
            x0: *x0 as f64,
            y0: *y0 as f64,
            x1: *x1 as f64,
            y1: *y1 as f64,
            thickness: *thickness as f64,
            up0: *up0,
            up1: *up1,
            ..Default::default()
        },
        RenderCommand::Slur {
            p0,
            c1,
            c2,
            p3,
            dashed,
        } => RenderCommandDto {
            kind: CMD_SLUR,
            x0: p0.x as f64,
            y0: p0.y as f64,
            x1: c1.x as f64,
            y1: c1.y as f64,
            x2: c2.x as f64,
            y2: c2.y as f64,
            x3: p3.x as f64,
            y3: p3.y as f64,
            dashed: *dashed,
            ..Default::default()
        },
        RenderCommand::Bracket { x, y_top, y_bottom } => RenderCommandDto {
            kind: CMD_BRACKET,
            x0: *x as f64,
            y0: *y_top as f64,
            y1: *y_bottom as f64,
            ..Default::default()
        },
        RenderCommand::Brace { x, y_top, y_bottom } => RenderCommandDto {
            kind: CMD_BRACE,
            x0: *x as f64,
            y0: *y_top as f64,
            y1: *y_bottom as f64,
            ..Default::default()
        },
        RenderCommand::NoteStem {
            x,
            y_top,
            y_bottom,
            width,
        } => RenderCommandDto {
            kind: CMD_NOTE_STEM,
            x0: *x as f64,
            y0: *y_top as f64,
            y1: *y_bottom as f64,
            width: *width as f64,
            ..Default::default()
        },
        RenderCommand::MusicChar {
            x,
            y,
            glyph,
            size_percent,
        } => {
            let code = match glyph {
                MusicGlyph::Smufl(cp) => *cp,
                MusicGlyph::Sonata(ch) => *ch as u32,
            };
            RenderCommandDto {
                kind: CMD_MUSIC_CHAR,
                x0: *x as f64,
                y0: *y as f64,
                glyph_code: code,
                size_percent: *size_percent as f64,
                ..Default::default()
            }
        }
        RenderCommand::MusicString {
            x,
            y,
            glyphs,
            size_percent,
        } => {
            let codes: Vec<u32> = glyphs
                .iter()
                .map(|g| match g {
                    MusicGlyph::Smufl(cp) => *cp,
                    MusicGlyph::Sonata(ch) => *ch as u32,
                })
                .collect();
            RenderCommandDto {
                kind: CMD_MUSIC_STRING,
                x0: *x as f64,
                y0: *y as f64,
                glyph_codes: codes,
                size_percent: *size_percent as f64,
                ..Default::default()
            }
        }
        RenderCommand::TextString { x, y, text, font } => RenderCommandDto {
            kind: CMD_TEXT_STRING,
            x0: *x as f64,
            y0: *y as f64,
            text: text.clone(),
            font_name: font.name.clone(),
            font_size: font.size as f64,
            bold: font.bold,
            italic: font.italic,
            ..Default::default()
        },
        RenderCommand::MusicColon {
            x,
            y,
            size_percent,
            line_space,
        } => RenderCommandDto {
            kind: CMD_MUSIC_COLON,
            x0: *x as f64,
            y0: *y as f64,
            size_percent: *size_percent as f64,
            line_spacing: *line_space as f64,
            ..Default::default()
        },
        RenderCommand::SetLineWidth(w) => RenderCommandDto {
            kind: CMD_SET_LINE_WIDTH,
            width: *w as f64,
            ..Default::default()
        },
        RenderCommand::SetWidths {
            staff,
            ledger,
            stem,
            bar,
        } => RenderCommandDto {
            kind: CMD_SET_WIDTHS,
            x0: *staff as f64,
            x1: *ledger as f64,
            x2: *stem as f64,
            x3: *bar as f64,
            ..Default::default()
        },
        RenderCommand::SetMusicSize(s) => RenderCommandDto {
            kind: CMD_SET_MUSIC_SIZE,
            size_percent: *s as f64,
            ..Default::default()
        },
        RenderCommand::SetPageSize { width, height } => RenderCommandDto {
            kind: CMD_SET_PAGE_SIZE,
            width: *width as f64,
            height: *height as f64,
            ..Default::default()
        },
        RenderCommand::BeginPage(n) => RenderCommandDto {
            kind: CMD_BEGIN_PAGE,
            page_number: *n,
            ..Default::default()
        },
        RenderCommand::EndPage => RenderCommandDto {
            kind: CMD_END_PAGE,
            ..Default::default()
        },
        RenderCommand::SaveState => RenderCommandDto {
            kind: CMD_SAVE_STATE,
            ..Default::default()
        },
        RenderCommand::RestoreState => RenderCommandDto {
            kind: CMD_RESTORE_STATE,
            ..Default::default()
        },
        RenderCommand::Translate { dx, dy } => RenderCommandDto {
            kind: CMD_TRANSLATE,
            x0: *dx as f64,
            y0: *dy as f64,
            ..Default::default()
        },
        RenderCommand::Scale { sx, sy } => RenderCommandDto {
            kind: CMD_SCALE,
            x0: *sx as f64,
            y0: *sy as f64,
            ..Default::default()
        },
        RenderCommand::SetColor(c) => RenderCommandDto {
            kind: CMD_SET_COLOR,
            color_r: c.r as f64,
            color_g: c.g as f64,
            color_b: c.b as f64,
            color_a: c.a as f64,
            ..Default::default()
        },
    }
}

// Helper: render an InterpretedScore to command DTOs.
//
// Prepends a SetPageSize command using the score's page dimensions so the
// Flutter ScorePainter knows the correct page size (render_score() itself
// does not emit SetPageSize — test harnesses call set_page_size() on the
// renderer directly before render_score()).
fn render_to_dtos(
    score: &nightingale_core::ngl::interpret::InterpretedScore,
) -> Vec<RenderCommandDto> {
    let mut renderer = CommandRenderer::new();
    renderer.set_page_size(score.page_width_pt, score.page_height_pt);
    render_score(score, &mut renderer);
    renderer.commands().iter().map(convert).collect()
}

// ── Bitmap DTO ──────────────────────────────────────────────────

/// One page rendered as an RGBA bitmap.
#[derive(Debug, Clone)]
pub struct PageBitmapDto {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA, width * height * 4 bytes.
    pub rgba: Vec<u8>,
}

/// Load music + text fonts into a BitmapRenderer from a font directory.
fn load_fonts_for_bitmap(renderer: &mut BitmapRenderer, font_dir: &str) {
    let font_dir_path = std::path::Path::new(font_dir);
    let bravura_path = font_dir_path.join("Bravura.otf");
    if bravura_path.exists() {
        if let Ok(data) = std::fs::read(&bravura_path) {
            renderer.load_music_font(data);
        }
    }
    renderer.load_text_fonts_from_dir(font_dir_path);
}

/// Render an InterpretedScore to a list of page bitmaps.
///
/// `font_dir` is the absolute path to a directory containing Bravura.otf and text fonts.
fn render_to_bitmaps(
    score: &InterpretedScore,
    font_dir: &str,
    dpi: f64,
) -> Vec<PageBitmapDto> {
    let mut renderer = BitmapRenderer::new(dpi as f32);
    renderer.set_page_size(score.page_width_pt, score.page_height_pt);
    if !font_dir.is_empty() {
        load_fonts_for_bitmap(&mut renderer, font_dir);
    }
    render_score(score, &mut renderer);

    (0..renderer.page_count())
        .filter_map(|i| {
            let (w, h) = renderer.page_dimensions(i)?;
            let data = renderer.page_data(i)?;
            Some(PageBitmapDto {
                width: w,
                height: h,
                rgba: data.to_vec(),
            })
        })
        .collect()
}

// ── Public API (exposed to Dart via flutter_rust_bridge) ────────

/// Load an NGL file from raw bytes and render it to a list of drawing commands.
///
/// Returns an empty vec on parse/interpret failure.
pub fn render_ngl_from_bytes(data: Vec<u8>) -> Vec<RenderCommandDto> {
    let ngl = match NglFile::read_from_bytes(&data) {
        Ok(n) => n,
        Err(e) => {
            log_debug!("[nightingale-bridge] NGL parse error: {e}");
            return vec![];
        }
    };
    let score = match interpret_heap(&ngl) {
        Ok(s) => s,
        Err(e) => {
            log_debug!("[nightingale-bridge] NGL interpret error: {e}");
            return vec![];
        }
    };
    render_to_dtos(&score)
}

/// Load a Notelist (.nl) file from UTF-8 text and render it to drawing commands.
///
/// Returns an error string on parse/convert failure (empty string = success).
/// The first element of the returned tuple is the error message (empty on success),
/// the second is the command list.
pub fn render_notelist_from_text(text: String) -> Vec<RenderCommandDto> {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};
    use std::io::Cursor;

    let notelist = match parse_notelist(Cursor::new(text.as_bytes())) {
        Ok(nl) => nl,
        Err(e) => {
            log_debug!("[nightingale-bridge] Notelist parse error: {e}");
            return vec![];
        }
    };
    let score = notelist_to_score(&notelist);
    render_to_dtos(&score)
}

/// Load a Notelist (.nl) file from raw bytes (Mac Roman encoded) and render it to drawing commands.
///
/// Notelist files are encoded in Mac Roman (single-byte encoding), not UTF-8.
/// This function accepts raw bytes and decodes them as Mac Roman before parsing.
/// Returns an empty vec on parse/convert failure.
pub fn render_notelist_from_bytes(data: Vec<u8>) -> Vec<RenderCommandDto> {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};
    use std::io::Cursor;

    // Decode Mac Roman bytes to UTF-8 string
    let text = mac_roman_to_string(&data);

    let notelist = match parse_notelist(Cursor::new(text.as_bytes())) {
        Ok(nl) => nl,
        Err(e) => {
            log_debug!("[nightingale-bridge] Notelist parse error: {e}");
            return vec![];
        }
    };
    let score = notelist_to_score(&notelist);
    render_to_dtos(&score)
}

/// Load an NGL file from raw bytes and render to page bitmaps.
///
/// `font_dir` is the absolute path to a directory containing Bravura.otf and text fonts.
/// `dpi` controls resolution (72.0 = screen, 144.0 = retina).
/// Returns an empty vec on parse/interpret failure.
pub fn render_ngl_to_bitmaps(data: Vec<u8>, font_dir: String, dpi: f64) -> Vec<PageBitmapDto> {
    let ngl = match NglFile::read_from_bytes(&data) {
        Ok(n) => n,
        Err(e) => {
            log_debug!("[nightingale-bridge] NGL parse error: {e}");
            return vec![];
        }
    };
    let score = match interpret_heap(&ngl) {
        Ok(s) => s,
        Err(e) => {
            log_debug!("[nightingale-bridge] NGL interpret error: {e}");
            return vec![];
        }
    };
    render_to_bitmaps(&score, &font_dir, dpi)
}

/// Load a Notelist from raw bytes (Mac Roman) and render to page bitmaps.
///
/// `font_dir` is the absolute path to a directory containing Bravura.otf and text fonts.
/// `dpi` controls resolution (72.0 = screen, 144.0 = retina).
/// Returns an empty vec on parse/convert failure.
pub fn render_notelist_to_bitmaps(data: Vec<u8>, font_dir: String, dpi: f64) -> Vec<PageBitmapDto> {
    use nightingale_core::notelist::{notelist_to_score, parse_notelist};
    use std::io::Cursor;

    let text = mac_roman_to_string(&data);
    let notelist = match parse_notelist(Cursor::new(text.as_bytes())) {
        Ok(nl) => nl,
        Err(e) => {
            log_debug!("[nightingale-bridge] Notelist parse error: {e}");
            return vec![];
        }
    };
    let score = notelist_to_score(&notelist);
    render_to_bitmaps(&score, &font_dir, dpi)
}

/// Decode Mac Roman bytes to a UTF-8 String using the `encoding_next` crate.
///
/// Mac Roman is a single-byte encoding used in legacy Mac OS files (including .nl files).
/// The `encoding_next` crate provides a battle-tested mapping table, avoiding the need
/// for a hand-rolled 128-entry lookup.
fn mac_roman_to_string(bytes: &[u8]) -> String {
    use encoding::all::MAC_ROMAN;
    use encoding::{DecoderTrap, Encoding};

    MAC_ROMAN
        .decode(bytes, DecoderTrap::Replace)
        .unwrap_or_else(|e| {
            log_debug!("[nightingale-bridge] Mac Roman decode error: {e}");
            String::from_utf8_lossy(bytes).into_owned()
        })
}

/// Render a score file from a filesystem path (auto-detects .ngl vs .nl).
///
/// Returns an empty vec on failure.
pub fn render_score_from_path(path: String) -> Vec<RenderCommandDto> {
    if path.ends_with(".nl") {
        match std::fs::read_to_string(&path) {
            Ok(text) => render_notelist_from_text(text),
            Err(_) => vec![],
        }
    } else {
        match std::fs::read(&path) {
            Ok(data) => render_ngl_from_bytes(data),
            Err(_) => vec![],
        }
    }
}

/// A score file entry for the file browser.
#[derive(Debug, Clone)]
pub struct ScoreFileEntry {
    /// Display name (filename without path).
    pub name: String,
    /// Full absolute path.
    pub path: String,
    /// "ngl" or "nl"
    pub format: String,
}

/// List score files (.ngl and .nl) in a directory.
///
/// Returns entries sorted by name. Non-recursive.
#[flutter_rust_bridge::frb(sync)]
pub fn list_score_files(directory: String) -> Vec<ScoreFileEntry> {
    let dir = std::path::Path::new(&directory);
    let mut entries = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();

            if ext == "ngl" || ext == "nl" {
                entries.push(ScoreFileEntry {
                    name,
                    path: path.to_string_lossy().to_string(),
                    format: ext,
                });
            }
        }
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

/// Render a score file with landscape orientation (for Notelist files).
///
/// NGL files have their own page dimensions embedded, so landscape is ignored.
/// For Notelist files, landscape swaps page width and height (792x612 instead of 612x792).
pub fn render_score_from_path_landscape(path: String, landscape: bool) -> Vec<RenderCommandDto> {
    if path.ends_with(".nl") {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                use nightingale_core::notelist::{
                    notelist_to_score_with_config, parse_notelist, NotelistLayoutConfig,
                };
                use std::io::Cursor;

                let notelist = match parse_notelist(Cursor::new(text.as_bytes())) {
                    Ok(nl) => nl,
                    Err(_) => return vec![],
                };
                let mut config = NotelistLayoutConfig::default();
                if landscape {
                    // Swap width/height for landscape orientation
                    let w = config.layout.page_width;
                    let h = config.layout.page_height;
                    config.layout.page_width = h;
                    config.layout.page_height = w;
                    // Recalculate system_right for wider page
                    let margin_right_pt: i16 = 54;
                    config.layout.system_right = (config.layout.page_width - margin_right_pt) * 16;
                    // Allow more measures per system in landscape
                    config.layout.max_measures = 6;
                }
                let score = notelist_to_score_with_config(&notelist, &config);
                render_to_dtos(&score)
            }
            Err(_) => vec![],
        }
    } else {
        // NGL files define their own page size
        match std::fs::read(&path) {
            Ok(data) => render_ngl_from_bytes(data),
            Err(_) => vec![],
        }
    }
}

/// Find the project root directory by searching upward from a starting path
/// for a directory containing both `Cargo.toml` and a `tests/` subdirectory.
///
/// Returns the path string if found, empty string otherwise.
#[flutter_rust_bridge::frb(sync)]
pub fn find_project_root(start_path: String) -> String {
    let mut dir = std::path::PathBuf::from(&start_path);

    // If start_path is a file, use its parent directory
    if dir.is_file() {
        dir = dir
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
    }

    // Search upward for project root (Cargo.toml + tests dir)
    for _ in 0..10 {
        // 10 levels up max
        if dir.join("Cargo.toml").exists() && dir.join("tests").is_dir() {
            return dir.to_string_lossy().to_string();
        }
        // Also accept if we find test-output/qa-compare (generated by qa-compare-smart.sh)
        if dir.join("test-output").is_dir() && dir.join("test-output/qa-compare").is_dir() {
            return dir.to_string_lossy().to_string();
        }
        if !dir.pop() {
            break;
        }
    }

    // Fallback: return the input path if we couldn't find a proper root
    start_path
}

/// Convenience: return the number of render commands for a given NGL file.
#[flutter_rust_bridge::frb(sync)]
pub fn render_command_count(data: Vec<u8>) -> i32 {
    render_ngl_from_bytes(data).len() as i32
}

/// Bridge health check.
#[flutter_rust_bridge::frb(sync)]
pub fn bridge_hello() -> String {
    "Nightingale Rust core ready".to_string()
}
