//! Object-level drawing — port of DrawObject.cp.
//!
//! Type-specific drawing functions for Staff, Measure, Connect, Clef,
//! KeySig, TimeSig, and Ties.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawObject.cp

use crate::context::ContextState;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore};
use crate::obj_types::PartInfo;
use crate::render::types::{ddist_to_render, BarLineType, MusicGlyph, Point, TextFont};
use crate::render::MusicRenderer;

use super::draw_utils::{clef_glyph, clef_halfline_position, get_ks_y_offset};
use super::helpers::{d2r_sum, d2r_sum3, lnspace_for_staff, TieEndpoint};

/// Header/footer delimiter character (0x01).
///
/// Template strings use this to separate left|center|right sections.
/// Reference: HeaderFooterDialog.cp line 46 — HEADERFOOTER_DELIM_CHAR
const HEADERFOOTER_DELIM_CHAR: char = '\x01';

/// Page number placeholder character.
///
/// In OG Nightingale this is loaded from a string resource (HEADERFOOTER_STRS index 3).
/// In practice it is always '#'.
/// Reference: HeaderFooterDialog.cp line 84-86
const PAGE_NUM_CHAR: char = '#';

/// Parse a header/footer template string into (left, center, right) sections.
///
/// The template format is: `leftText<0x01>centerText<0x01>rightText`.
/// The page number placeholder '#' is replaced with the actual page number.
///
/// Port of HeaderFooterDialog.cp GetHeaderFooterStrings() (lines 64-118).
fn parse_header_footer_template(template: &str, page_num: i32) -> (String, String, String) {
    if template.is_empty() {
        return (String::new(), String::new(), String::new());
    }

    let page_num_str = page_num.to_string();
    let mut sections: Vec<String> = Vec::with_capacity(3);
    let mut current = String::new();

    for ch in template.chars() {
        if ch == HEADERFOOTER_DELIM_CHAR {
            sections.push(current);
            current = String::new();
        } else if ch == PAGE_NUM_CHAR {
            // Substitute page number placeholder
            current.push_str(&page_num_str);
        } else {
            current.push(ch);
        }
    }
    sections.push(current);

    let left = sections.first().cloned().unwrap_or_default();
    let center = sections.get(1).cloned().unwrap_or_default();
    let right = sections.get(2).cloned().unwrap_or_default();
    (left, center, right)
}

/// Draw page header and footer text.
///
/// Port of DrawObject.cp DrawHeaderFooter() (lines 60-177).
///
/// Parses the header/footer template strings, handles alternating left/right
/// positioning for even pages, and renders up to 6 text strings (3 header + 3 footer).
///
/// Reference: DrawObject.cp, DrawHeaderFooter(), lines 60-177
fn draw_header_footer(score: &InterpretedScore, sheet_num: i16, renderer: &mut dyn MusicRenderer) {
    let page_num = sheet_num as i32 + score.first_page_number as i32;

    // Skip pages before startPageNumber
    // Reference: DrawObject.cp:73 — if (pageNum < doc->startPageNumber) return;
    if page_num < score.start_page_number as i32 {
        return;
    }

    let font = TextFont::new(&score.pg_font_name, score.pg_font_size);
    let font_size = score.pg_font_size;

    // Parse header and footer template strings
    // Reference: DrawObject.cp:82-90 — handles alternate page numbering
    let (lh, ch, rh, lf, cf, rf) = if score.alternate_pgn && page_num % 2 == 0 {
        // Even pages: swap left and right
        let (r, c, l) = parse_header_footer_template(&score.header_str, page_num);
        let (rf, cf2, lf) = parse_header_footer_template(&score.footer_str, page_num);
        (l, c, r, lf, cf2, rf)
    } else {
        let (l, c, r) = parse_header_footer_template(&score.header_str, page_num);
        let (lf, cf2, rf) = parse_header_footer_template(&score.footer_str, page_num);
        (l, c, r, lf, cf2, rf)
    };

    // Vertical positions
    // Reference: DrawObject.cp:92-98 — hypt/fypt from margins, adjusted by fontSize/2
    let hy = score.hf_margin_top + font_size / 2.0;
    let fy = score.page_height_pt - score.hf_margin_bottom + font_size / 2.0;

    // Horizontal positions
    let left_x = score.hf_margin_left;
    let page_right = score.page_width_pt - score.hf_margin_right;

    // Render header strings (left, center, right)
    if !lh.is_empty() {
        renderer.text_string(left_x, hy, &lh, &font);
    }
    if !ch.is_empty() {
        // Center: we approximate centering. text_string places at baseline x;
        // exact centering requires string width which we don't have. Use page center.
        let cx = score.page_width_pt / 2.0;
        renderer.text_string(cx, hy, &ch, &font);
    }
    if !rh.is_empty() {
        // Right-aligned: place at right margin (renderer doesn't support right-align,
        // so we approximate by placing near right margin — exact alignment requires
        // string width measurement which the renderer doesn't expose yet)
        renderer.text_string(page_right, hy, &rh, &font);
    }

    // Render footer strings (left, center, right)
    if !lf.is_empty() {
        renderer.text_string(left_x, fy, &lf, &font);
    }
    if !cf.is_empty() {
        let cx = score.page_width_pt / 2.0;
        renderer.text_string(cx, fy, &cf, &font);
    }
    if !rf.is_empty() {
        renderer.text_string(page_right, fy, &rf, &font);
    }
}

/// Draw a simple page number (when useHeaderFooter is false).
///
/// Port of DrawObject.cp DrawPageNum() (lines 183-243).
///
/// Respects topPGN, hPosPGN, alternatePGN, and startPageNumber flags.
/// Font uses the PG font from the score header.
///
/// Reference: DrawObject.cp, DrawPageNum(), lines 183-243
fn draw_page_number_simple(
    score: &InterpretedScore,
    sheet_num: i16,
    renderer: &mut dyn MusicRenderer,
) {
    // Calculate display page number: sheet_num is 0-indexed
    // Reference: DrawObject.cp:195 — pageNum = p->sheetNum + doc->firstPageNumber
    let page_num = sheet_num as i32 + score.first_page_number as i32;

    // Skip pages before startPageNumber
    // Reference: DrawObject.cp:196 — if (pageNum < doc->startPageNumber) return;
    if page_num < score.start_page_number as i32 {
        return;
    }

    let page_str = page_num.to_string();
    let font_size = score.pg_font_size;
    let font = TextFont::new(&score.pg_font_name, font_size);

    // Vertical position: top or bottom
    // Reference: DrawObject.cp:206-210
    let y = if score.top_pgn {
        score.hf_margin_top + font_size / 2.0
    } else {
        score.page_height_pt - score.hf_margin_bottom + font_size / 2.0
    };

    // Horizontal position: left, center, or right
    // Reference: DrawObject.cp:212-226
    let mut h_pos = score.h_pos_pgn;

    // Handle alternating page numbers
    // Reference: DrawObject.cp:201-203
    if score.alternate_pgn && page_num % 2 == 0 {
        // Even pages: swap left<->right
        h_pos = match h_pos {
            1 => 3, // LEFT -> RIGHT
            3 => 1, // RIGHT -> LEFT
            _ => h_pos,
        };
    }

    let x = match h_pos {
        1 => score.hf_margin_left,                        // LEFT
        3 => score.page_width_pt - score.hf_margin_right, // RIGHT
        _ => score.page_width_pt / 2.0,                   // CENTER (default)
    };

    renderer.text_string(x, y, &page_str, &font);
}

/// Draw page number or header/footer for a PAGE object.
///
/// Dispatches to either draw_header_footer() or draw_page_number_simple()
/// based on the score's useHeaderFooter flag.
///
/// Port of DrawObject.cp DrawPAGE() (lines 249-257).
///
/// Reference: DrawObject.cp, DrawPAGE(), lines 249-257
pub fn draw_page_number(
    score: &InterpretedScore,
    sheet_num: i16,
    renderer: &mut dyn MusicRenderer,
) {
    if score.use_header_footer {
        draw_header_footer(score, sheet_num, renderer);
    } else {
        draw_page_number_simple(score, sheet_num, renderer);
    }
}

/// Draw a Staff object (all staves in the system).
///
/// Port of DrawObject.cp Draw1Staff() (line 591).
///
/// For each AStaff subobject:
/// - Get context for that staff
/// - If staff is visible and showLines indicates lines should be drawn:
///   - Draw the N-line staff at the appropriate position
///
/// Reference: DrawObject.cp, Draw1Staff(), line 591
pub fn draw_staff(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
        for astaff in astaff_list {
            if let Some(staff_ctx) = ctx.get(astaff.staffn) {
                // Set music font size from staff height.
                // OG: PS_MusSize(doc, d2pt(pContext->staffHeight) + config.musFontSizeOffset)
                // Reference: DrawUtils.cp:2619
                // staffHeight in DDIST → points = staffHeight / 16.
                // SMuFL font size = staff height in points (4 staff spaces).
                let music_pt_size = ddist_to_render(staff_ctx.staff_height);
                if music_pt_size > 0.0 {
                    renderer.set_music_size(music_pt_size);
                }

                // Check if staff is visible and should show lines
                // C++ code: if (showLines>0) — any non-zero means draw
                // SHOW_ALL_LINES (127) means draw all lines
                // Reference: DrawObject.cp, Draw1Staff(), line 569
                if staff_ctx.staff_visible && astaff.show_lines > 0 {
                    // Context already has absolute coordinates (staff_top, staff_left, staff_right)
                    let top_y = ddist_to_render(staff_ctx.staff_top);
                    let left_x = ddist_to_render(staff_ctx.staff_left);
                    let right_x = ddist_to_render(staff_ctx.staff_right);
                    let n_lines = staff_ctx.staff_lines as u8;

                    // Line spacing = staff_height / (staff_lines - 1) in render coords
                    let line_spacing = if n_lines > 1 {
                        ddist_to_render(staff_ctx.staff_height) / (n_lines as f32 - 1.0)
                    } else {
                        0.0
                    };

                    renderer.staff(top_y, left_x, right_x, n_lines, line_spacing);
                }
            }
        }
    }
}

/// Draw part names to the left of staves.
///
/// Port of DrawObject.cp DrawPartName() (lines 326-410).
///
/// Called during STAFF object rendering. For each part, the name is drawn
/// once per part (triggered on the last staff), centered vertically between
/// the part's first and last staves.
///
/// Which name is shown depends on whether this is the first system:
///   - System 1: uses score.first_names (0=none, 1=abbrev, 2=full)
///   - Other systems: uses score.other_names
///
/// Horizontal centering formula from OG (DrawObject.cp:370-377):
///   xd = systemLeft - connWidth - indent/2 - pt2d(nameWidth/2)
///
/// Vertical centering from OG (DrawObject.cp:399, PostScript path):
///   yd = midpoint(firstStaffTop, lastStaffBot) + pt2d(fontSize/4)
///
/// Reference: DrawObject.cp:326-410, DrawObject.cp:636-651
pub fn draw_part_names(
    score: &InterpretedScore,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
    system_num: i16,
) {
    use crate::render::types::ddist_to_render;

    // Determine which name code and indent apply for this system.
    // OG: DrawObject.cp:636-651 — first system uses firstNames/dIndentFirst,
    //     subsequent systems use otherNames/dIndentOther.
    // Note: d_indent_first/d_indent_other are available in the score header but
    // not used here because NGL system rects already incorporate the indent.
    let names_code = if system_num <= 1 {
        score.first_names
    } else {
        score.other_names
    };

    // 0 = NONAMES: don't draw part names
    // Reference: DrawObject.cp:344
    if names_code == 0 {
        return;
    }

    for part_info in &score.part_infos {
        // Skip DUMMY parts (first_staff < 1 or last_staff < 1)
        if part_info.first_staff < 1 || part_info.last_staff < 1 {
            continue;
        }

        // Get name string based on code
        // Reference: DrawObject.cp:355-356
        let name = if names_code == 1 {
            // ABBREVNAMES
            PartInfo::short_name_str(part_info)
        } else {
            // FULLNAMES
            PartInfo::name_str(part_info)
        };

        if name.is_empty() {
            continue;
        }

        // Get context for first and last staff of this part
        let first_ctx = match ctx.get(part_info.first_staff) {
            Some(c) if c.staff_visible => c,
            _ => continue,
        };
        let last_ctx = match ctx.get(part_info.last_staff) {
            Some(c) if c.staff_visible => c,
            _ => continue,
        };

        // Compute lineSpace for relative font size resolution
        let line_space = if first_ctx.staff_lines > 1 {
            first_ctx.staff_height / (first_ctx.staff_lines as i16 - 1)
        } else {
            first_ctx.staff_height
        };

        // Resolve part name font from text style FONT_PN.
        // FONT_PN = 2, but text_styles is 0-indexed so we use index 1.
        // Reference: DrawObject.cp:358 — GetTextSize(doc->relFSizePN, doc->fontSizePN, LNSPACE)
        let pn_idx = crate::defs::FONT_PN as usize - 1; // 2 - 1 = 1
        let font = if score.text_styles.len() > pn_idx {
            let ts = &score.text_styles[pn_idx];
            let pt_size = if ts.rel_f_size {
                rel_size_to_pt(ts.font_size, line_space)
            } else {
                (ts.font_size as f32).max(4.0)
            };
            let bold = (ts.font_style & 1) != 0;
            let italic = (ts.font_style & 2) != 0;
            let font_name = if ts.font_name.is_empty() {
                "Times New Roman".to_string()
            } else {
                map_mac_font_name(&ts.font_name)
            };
            TextFont::new(font_name, pt_size).bold(bold).italic(italic)
        } else {
            TextFont::new("Times New Roman".to_string(), 10.0)
        };

        // --- Vertical position ---
        // OG: DrawObject.cp:349-352 — midpoint of first staff top to last staff bottom
        let yd_top = first_ctx.staff_top;
        let yd_bot = last_ctx.staff_top + last_ctx.staff_height;
        let yd_mid = ((yd_top as i32 + yd_bot as i32) / 2) as i16;
        // OG PostScript path: yd += pt2d(fontSize/4) for baseline adjustment
        // Reference: DrawObject.cp:399
        let yd_adjusted = yd_mid.saturating_add((font.size * 4.0) as i16); // pt2d = pt * 16
        let text_y = ddist_to_render(yd_adjusted);

        // --- Horizontal position ---
        // OG centered mode (DrawObject.cp:370-377) centers the name in the indent
        // area: xd = systemLeft - connWidth - indent/2 - pt2d(nameWidth/2)
        //
        // connWidth = 7*lineSpace/4 (for curly brace, the widest connector type).
        // OG always uses CONNECTCURLY here regardless of actual connect type.
        // Reference: SpaceTime.cp ConnectDWidth() lines 525-543
        //
        // indent = d_indent_first (system 1) or d_indent_other (subsequent).
        // Many NGL files store d_indent_first=0 even when the first system is
        // indented (the indent is baked into the system rect). For these cases
        // we right-align the name to the left of the brace.
        //
        // Reference: DrawObject.cp:360-377, SpaceTime.cp:525-543
        let indent = if system_num <= 1 {
            score.d_indent_first
        } else {
            score.d_indent_other
        };

        // connWidth = 7 * lineSpace / 4 (DDIST) — OG SpaceTime.cp:536
        let conn_width = (7 * line_space) / 4;

        // Approximate name width in points, then convert to DDIST (pt * 16)
        let approx_name_width_pt = name.len() as f32 * font.size * 0.5;
        let name_width_ddist = (approx_name_width_pt * 16.0) as i16; // full width in DDIST

        let xd = if indent > 0 {
            // OG centering formula: center the name in the indent area,
            // leaving room for the brace.
            let half_name = name_width_ddist / 2;
            first_ctx
                .system_left
                .saturating_sub(conn_width)
                .saturating_sub(indent / 2)
                .saturating_sub(half_name)
        } else {
            // No explicit indent — right-align the name to the left of the
            // brace with a small margin. The 4pt gap keeps names clear of
            // the brace/bracket.
            let margin_ddist = 64_i16; // 4pt * 16
            first_ctx
                .system_left
                .saturating_sub(conn_width)
                .saturating_sub(margin_ddist)
                .saturating_sub(name_width_ddist)
        };
        let text_x = ddist_to_render(xd);

        renderer.text_string(text_x, text_y, &name, &font);
    }
}

/// Map AMeasure.header.sub_type to BarLineType enum.
///
/// Reference: obj_types.rs BarlineType enum (lines 382-390)
fn map_barline_type(sub_type: i8) -> BarLineType {
    match sub_type {
        1 => BarLineType::Single,
        2 => BarLineType::Double,
        3 => BarLineType::FinalDouble,
        5 => BarLineType::RepeatLeft,
        6 => BarLineType::RepeatRight,
        7 => BarLineType::RepeatBoth,
        _ => BarLineType::Single, // Default to single
    }
}

/// Draw a Measure object (bar lines + measure numbers for all staves).
///
/// Port of DrawObject.cp DrawMEASURE() (line 2772) + DrawBarline() + DrawMeasNum().
///
/// For each AMeasure subobject:
/// - Draw bar line if visible
/// - Draw measure number if conditions are met (ShouldDrawMeasNum logic)
///
/// Measure number conditions (DrawUtils.cp:2132-2161, ShouldDrawMeasNum):
/// - `number_meas != 0` (measure numbers enabled)
/// - Not a fake measure
/// - Staff is top visible (if aboveMN) or bottom visible (if !aboveMN)
/// - measureNum >= threshold (startMNPrint1 ? 1 : 2)
/// - Either every nth measure (number_meas > 0) or first measure in system (number_meas < 0)
///
/// Reference: DrawObject.cp:2772 (DrawMEASURE), DrawObject.cp:2504-2553 (DrawMeasNum),
///            DrawObject.cp:2885-2911 (measure number call site),
///            DrawUtils.cp:2132-2161 (ShouldDrawMeasNum)
pub fn draw_measure(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
    first_meas_in_system: bool,
) {
    if let Some(ameasure_list) = score.measures.get(&obj.header.first_sub_obj) {
        // Find the top and bottom visible staff numbers for measure number placement.
        // Reference: DrawUtils.cp:2149-2150 — NextVisStaffn()
        let mut top_vis_staff: i8 = 0;
        let mut bot_vis_staff: i8 = 0;
        for ameasure in ameasure_list {
            let staffn = ameasure.header.staffn;
            if let Some(c) = ctx.get(staffn) {
                if c.staff_visible {
                    if top_vis_staff == 0 || staffn < top_vis_staff {
                        top_vis_staff = staffn;
                    }
                    if staffn > bot_vis_staff {
                        bot_vis_staff = staffn;
                    }
                }
            }
        }

        for ameasure in ameasure_list {
            if let Some(measure_ctx) = ctx.get(ameasure.header.staffn) {
                // --- Bar line ---
                // Port of ShouldDrawBarline (DrawUtils.cp:2020-2067):
                // - conn_staff>0 && !conn_above: group leader — draw from this staff to conn_staff
                // - conn_staff==0 && !conn_above: standalone staff — draw single-staff barline
                // - conn_above: subordinate staff — skip (covered by group leader above)
                let draw_bar = !ameasure.conn_above || ameasure.conn_staff != 0;
                if measure_ctx.visible && ameasure.header.visible && draw_bar {
                    let x = ddist_to_render(measure_ctx.measure_left);
                    let top_y = ddist_to_render(measure_ctx.staff_top);
                    // If conn_staff > 0, extend barline to bottom of connected staff
                    // Reference: DrawObject.cp:2679-2690 (DrawBarline connStaff logic)
                    let bottom_y = if ameasure.conn_staff > 0 {
                        // Find the bottom-most visible staff in the connected range
                        let target = ameasure.conn_staff;
                        if let Some(target_ctx) = ctx.get(target) {
                            d2r_sum(target_ctx.staff_top, target_ctx.staff_height)
                        } else {
                            // Target staff not visible — fall back to just this staff
                            d2r_sum(measure_ctx.staff_top, measure_ctx.staff_height)
                        }
                    } else {
                        d2r_sum(measure_ctx.staff_top, measure_ctx.staff_height)
                    };
                    let bar_type = map_barline_type(ameasure.header.sub_type);
                    // lineSpace = staffHeight / (staffLines - 1), in render coords (pt)
                    let ls_render = if measure_ctx.staff_lines > 1 {
                        ddist_to_render(
                            measure_ctx.staff_height / (measure_ctx.staff_lines as i16 - 1),
                        )
                    } else {
                        ddist_to_render(measure_ctx.staff_height)
                    };
                    renderer.bar_line(top_y, bottom_y, x, bar_type, ls_render);
                }

                // --- Measure number ---
                // Reference: DrawObject.cp:2885-2911
                if should_draw_meas_num(
                    score,
                    ameasure,
                    top_vis_staff,
                    bot_vis_staff,
                    first_meas_in_system,
                ) {
                    // Suppress measure numbers at the system's closing barline
                    // and at the score's final barline.
                    // NGL files repeat the boundary measure as both the closing
                    // barline of system N and the opening barline of system N+1.
                    // System-end barlines have dist=1; the score's final barline
                    // has dist=72 (N103) or 64 (N105). Mid-system measures have
                    // dist >= 595 across all fixtures. Threshold of 80 DDIST
                    // (5 points) covers both cases with wide safety margin.
                    // Reference: DrawObject.cp:2885-2911
                    let dist_to_right =
                        (measure_ctx.measure_left - measure_ctx.staff_right).unsigned_abs();
                    let at_system_end = dist_to_right <= 80;
                    if !at_system_end {
                        draw_meas_num(
                            score,
                            obj,
                            ameasure,
                            measure_ctx,
                            renderer,
                            first_meas_in_system,
                        );
                    }
                }
            }
        }
    }
}

/// Determine if a measure number should be drawn for this AMeasure subobject.
///
/// Port of DrawUtils.cp ShouldDrawMeasNum() (lines 2132-2161).
///
/// Reference: DrawUtils.cp:2132-2161
fn should_draw_meas_num(
    score: &InterpretedScore,
    ameasure: &crate::obj_types::AMeasure,
    top_vis_staff: i8,
    bot_vis_staff: i8,
    first_meas_in_system: bool,
) -> bool {
    // number_meas == 0 means never show measure numbers
    if score.number_meas == 0 {
        return false;
    }

    // Check if this is a fake measure (high bit of reserved_m was oldFakeMeas)
    // Reference: DrawUtils.cp:2141 — MeasISFAKE
    if ameasure.reserved_m < 0 {
        return false;
    }

    let measure_num = ameasure.measure_num as i32 + score.first_mn_number as i32;

    // Only draw on the correct staff (top if aboveMN, bottom otherwise)
    // Reference: DrawUtils.cp:2149-2154
    let target_staff = if score.above_mn {
        top_vis_staff
    } else {
        bot_vis_staff
    };
    if ameasure.header.staffn != target_staff {
        return false;
    }

    // Check minimum measure number threshold
    // Reference: DrawUtils.cp:2156
    let threshold = if score.start_mn_print1 { 1 } else { 2 };
    if measure_num < threshold {
        return false;
    }

    // Check frequency: every nth measure or first-in-system only
    // Reference: DrawUtils.cp:2158-2159
    if score.number_meas > 0 {
        // Every nth measure
        (measure_num % score.number_meas as i32) == 0
    } else {
        // number_meas < 0: first measure of each system only
        first_meas_in_system
    }
}

/// Draw a measure number at the appropriate position.
///
/// Port of DrawObject.cp DrawMeasNum() (lines 2504-2553) and the position
/// calculation from DrawMEASURE (lines 2885-2911, PostScript path).
///
/// Position calculation:
///   xdMN = dLeft + halfLn2d(xMNOffset, ...) [+ per-measure xMNStdOffset]
///   yOffset = aboveMN ? -yMNOffset : 2*staffLines + yMNOffset
///   ydMN = dTop + halfLn2d(yOffset, ...) [+ per-measure yMNStdOffset]
///
/// Reference: DrawObject.cp:2504-2553, DrawObject.cp:2885-2911
fn draw_meas_num(
    score: &InterpretedScore,
    _obj: &InterpretedObject,
    ameasure: &crate::obj_types::AMeasure,
    measure_ctx: &crate::obj_types::Context,
    renderer: &mut dyn MusicRenderer,
    first_meas_in_system: bool,
) {
    use crate::utility::std2d;

    let measure_num = ameasure.measure_num as i32 + score.first_mn_number as i32;
    let num_str = measure_num.to_string();

    let staff_height = measure_ctx.staff_height;
    let staff_lines = measure_ctx.staff_lines as i16;

    // halfLn2d: convert half-line units to DDIST
    // halfLn2d(h, staffHeight, staffLines) = h * staffHeight / (2 * (staffLines - 1))
    // Reference: defs.h, halfLn2d macro
    let half_ln_2d = |half_lines: i32| -> i16 {
        if staff_lines <= 1 {
            return 0;
        }
        (half_lines * staff_height as i32 / (2 * (staff_lines as i32 - 1))) as i16
    };

    // X position: at the measure's left edge + horizontal offset
    // Reference: DrawObject.cp:2893-2898
    let d_left = measure_ctx.measure_left;
    let x_offset = if first_meas_in_system && score.sys_first_mn {
        // First measure in system uses staffLeft + xSysMNOffset
        // Reference: DrawObject.cp:2893-2894
        let base = measure_ctx.staff_left;
        base + half_ln_2d(score.x_sys_mn_offset as i32)
    } else {
        d_left + half_ln_2d(score.x_mn_offset as i32)
    };
    // Per-measure adjustment
    // Reference: DrawObject.cp:2905-2906
    let xd_mn = x_offset + std2d(ameasure.x_mn_std_offset as i16, staff_height, staff_lines);

    // Y position: above or below the staff
    // Reference: DrawObject.cp:2899-2904
    let y_offset_halflines = if score.above_mn {
        -(score.y_mn_offset as i32)
    } else {
        2 * staff_lines as i32 + score.y_mn_offset as i32
    };
    let d_top = measure_ctx.staff_top;
    let yd_base = d_top + half_ln_2d(y_offset_halflines);
    let yd_mn = yd_base + std2d(ameasure.y_mn_std_offset as i16, staff_height, staff_lines);

    // Compute lineSpace for relative font size
    let line_space = if staff_lines > 1 {
        staff_height / (staff_lines - 1)
    } else {
        staff_height
    };

    // Resolve font from FONT_MN.
    // FONT_MN = 1, but text_styles is 0-indexed so we use index 0.
    // Reference: DrawObject.cp:2540 — GetTextSize(doc->relFSizeMN, doc->fontSizeMN, LNSPACE)
    let mn_idx = crate::defs::FONT_MN as usize - 1; // 1 - 1 = 0
    let font = if score.text_styles.len() > mn_idx {
        let ts = &score.text_styles[mn_idx];
        let pt_size = if ts.rel_f_size {
            rel_size_to_pt(ts.font_size, line_space)
        } else {
            (ts.font_size as f32).max(4.0)
        };
        let bold = (ts.font_style & 1) != 0;
        let italic = (ts.font_style & 2) != 0;
        let font_name = if ts.font_name.is_empty() {
            "Times New Roman".to_string()
        } else {
            map_mac_font_name(&ts.font_name)
        };
        TextFont::new(font_name, pt_size).bold(bold).italic(italic)
    } else {
        TextFont::new("Times New Roman".to_string(), 10.0)
    };

    let text_x = ddist_to_render(xd_mn);
    let text_y = ddist_to_render(yd_mn);
    renderer.text_string(text_x, text_y, &num_str, &font);
}

/// Draw a Connect object (brackets, braces, or connecting lines).
///
/// Port of DrawObject.cp DrawCONNECT() (line 670).
///
/// For each AConnect subobject:
/// - Get contexts for top and bottom staves
/// - Compute connector line from top staff top to bottom staff bottom
/// - Call appropriate renderer method based on connect type
///
/// Reference: DrawObject.cp, DrawCONNECT(), line 670
pub fn draw_connect(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    let num_staves = ctx.num_staves();

    if let Some(aconnect_list) = score.connects.get(&obj.header.first_sub_obj) {
        for aconnect in aconnect_list {
            // Determine staff range based on connLevel.
            // OG: connLevel==SystemLevel(0) means "entire system" — use first/last staff.
            // connLevel!=0 means staffAbove/staffBelow are valid.
            // Reference: DrawObject.cp DrawCONNECT() line 686-710
            let entire = aconnect.conn_level == 0; // SystemLevel

            let (stf_above, stf_below) = if entire {
                (1_i8, num_staves as i8)
            } else {
                (aconnect.staff_above, aconnect.staff_below)
            };

            // Visibility check (port of ShouldDrawConnect, DrawUtils.cp:1918)
            // SystemLevel: draw if more than 1 visible staff
            // GroupLevel/PartLevel: draw if any staves in range are visible
            let should_draw = if entire {
                // Count visible staves in the system
                let vis_count = (1..=num_staves as i8)
                    .filter(|&s| ctx.get(s).is_some_and(|c| c.visible))
                    .count();
                vis_count > 1
            } else {
                // At least one staff in range must be visible
                (stf_above..=stf_below).any(|s| ctx.get(s).is_some_and(|c| c.visible))
            };

            if !should_draw {
                continue;
            }

            // Find actual top and bottom visible staves in range
            let top_staff =
                (stf_above..=stf_below).find(|&s| ctx.get(s).is_some_and(|c| c.visible));
            let bottom_staff = (stf_above..=stf_below)
                .rev()
                .find(|&s| ctx.get(s).is_some_and(|c| c.visible));

            if let (Some(ts), Some(bs)) = (top_staff, bottom_staff) {
                if let (Some(top_ctx), Some(bottom_ctx)) = (ctx.get(ts), ctx.get(bs)) {
                    // X position from aconnect.xd
                    let x = d2r_sum(top_ctx.staff_left, aconnect.xd);

                    // Top Y from top staff top
                    let y_top = ddist_to_render(top_ctx.staff_top);

                    // Bottom Y from bottom staff top + height
                    let y_bottom = d2r_sum(bottom_ctx.staff_top, bottom_ctx.staff_height);

                    // Map connect type (1=line, 2=bracket, 3=brace)
                    match aconnect.connect_type {
                        1 => renderer.connector_line(y_top, y_bottom, x),
                        2 => renderer.bracket(x, y_top, y_bottom),
                        3 => renderer.brace(x, y_top, y_bottom),
                        _ => {} // Unknown connect type
                    }
                }
            }
        }
    }
}

/// Draw a Clef object.
///
/// Port of DrawObject.cp DrawCLEF() (line 1075).
///
/// For each AClef subobject:
/// - Get context for that staff
/// - If visible:
///   - Map clef type (header.sub_type) to SMuFL glyph
///   - Compute X from measure_left + obj.xd + aclef.xd
///   - Compute Y from staff_top + clef half-line position
///   - Draw clef glyph
///
/// Reference: DrawObject.cp, DrawCLEF(), line 1075
pub fn draw_clef(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let Some(aclef_list) = score.clefs.get(&obj.header.first_sub_obj) {
        for aclef in aclef_list {
            if let Some(clef_ctx) = ctx.get(aclef.header.staffn) {
                // Check visibility
                if clef_ctx.visible && aclef.header.visible {
                    // Clef type from header.sub_type
                    let clef_type = aclef.header.sub_type;

                    // Map to SMuFL glyph
                    let glyph = clef_glyph(clef_type);

                    // X position: for initial clefs (before first Measure), use staff_left;
                    // for mid-score clefs, use measure_left.
                    let base_x = if clef_ctx.measure_left > 0 {
                        clef_ctx.measure_left
                    } else {
                        clef_ctx.staff_left
                    };
                    let clef_x = d2r_sum3(base_x, obj.header.xd, aclef.xd);

                    // Y position: staff_top + half-line offset
                    let halfline = clef_halfline_position(clef_type);
                    let lnspace = lnspace_for_staff(clef_ctx.staff_height, clef_ctx.staff_lines);

                    // Mid-measure clefs drawn at 75% (OG: SMALLSIZE macro, style.h:16)
                    let size_pct = if aclef.small != 0 { 75.0 } else { 100.0 };

                    // Sonata correction: Sonata clef glyphs have their origin at the
                    // bottom staff line (4*lnspace from top for 5-line staff), but our
                    // halfline positions assume SMuFL convention (origin at reference
                    // pitch line: treble=G, alto=C, bass=F).
                    // Reference: DrawUtils.cp GetClefDrawInfo() lines 320-384.
                    //
                    // At reduced size (size_pct < 100), the Sonata origin-to-reference
                    // distance scales, so we use SizePercentSCALE for the correction.
                    // Formula from OG: ydR = <reference_hl>*lnspace/2 + SCALE(<correction>)
                    let clef_y = if renderer.uses_sonata_font() {
                        let scale_f = size_pct / 100.0;
                        let sonata_correction = match clef_type {
                            // Treble/Treble8/TrTenor: ref at G line (hl 6 = 3*ln),
                            // Sonata origin at bottom line (4*ln). Correction = +1*ln.
                            1 | 3 | 7 => 1.0 * lnspace * scale_f,
                            // Soprano: ref at C bottom line (hl 8 = 4*ln), but
                            // Sonata C clef origin at bottom = 4*ln + 0 correction
                            // OG: ydR = 4*dLnHt + SCALE(2*dLnHt) - SCALE(2*dLnHt)
                            4 => 0.0,
                            // Mezzosoprano: ref at C 2nd line (hl 6 = 3*ln)
                            // OG: ydR = 3*dLnHt + SCALE(dLnHt) + SCALE(dLnHt)
                            5 => 2.0 * lnspace * scale_f,
                            // Alto: ref at C middle (hl 4 = 2*ln), Sonata at 4*ln.
                            // Correction = +2*ln.
                            6 => 2.0 * lnspace * scale_f,
                            // Tenor: ref at C 4th line (hl 2 = 1*ln)
                            // OG: ydR = 1*dLnHt + SCALE(3*dLnHt) - SCALE(dLnHt)
                            8 => 2.0 * lnspace * scale_f,
                            // Baritone: ref at C top line (hl 0 = 0*ln)
                            // OG: ydR = 0 + SCALE(4*dLnHt) - SCALE(2*dLnHt)
                            9 => 2.0 * lnspace * scale_f,
                            // Bass/Bass8b: ref at F line (hl 2 = 1*ln), Sonata at 4*ln.
                            // Correction = +3*ln.
                            10 | 11 => 3.0 * lnspace * scale_f,
                            // Percussion: different handling
                            // OG: ydR = 2*dLnHt + SCALE(dLnHt)
                            12 => 1.0 * lnspace * scale_f,
                            _ => 2.0 * lnspace * scale_f,
                        };
                        ddist_to_render(clef_ctx.staff_top)
                            + (halfline as f32 * lnspace / 2.0)
                            + sonata_correction
                    } else {
                        // SMuFL: glyph origin IS at reference pitch line. No correction.
                        ddist_to_render(clef_ctx.staff_top) + (halfline as f32 * lnspace / 2.0)
                    };

                    renderer.music_char(clef_x, clef_y, MusicGlyph::smufl(glyph), size_pct);
                }
            }
        }
    }
}

/// Draw a KeySig object (key signature accidentals).
///
/// Port of DrawObject.cp DrawKEYSIG() (line 963) + DrawUtils.cp DrawKSItems() (line 956).
///
/// For each AKeySig subobject:
/// - Get context for that staff (clef type determines accidental positions)
/// - Draw each sharp/flat glyph at the correct half-line position
/// - Horizontal spacing: STD_KS_ACCSPACE = 9*STD_LINEHT/8 per accidental
///
/// Reference: DrawObject.cp:963, DrawUtils.cp:737-1010
pub fn draw_keysig(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    // SMuFL accidental codes
    const SMUFL_SHARP: u32 = 0xE262; // accidentalSharp
    const SMUFL_FLAT: u32 = 0xE260; // accidentalFlat
    const SMUFL_NATURAL: u32 = 0xE261; // accidentalNatural

    if let Some(akeysig_list) = score.keysigs.get(&obj.header.first_sub_obj) {
        for akeysig in akeysig_list {
            if let Some(ks_ctx) = ctx.get(akeysig.header.staffn) {
                if !ks_ctx.visible || !akeysig.header.visible {
                    continue;
                }

                // X origin: staff_left + object xd + subobj xd
                let origin_x = if ks_ctx.measure_left > 0 {
                    ks_ctx.measure_left
                } else {
                    ks_ctx.staff_left
                };
                let base_x = d2r_sum3(origin_x, obj.header.xd, akeysig.xd);

                // Line spacing
                let lnspace = lnspace_for_staff(ks_ctx.staff_height, ks_ctx.staff_lines);

                // Horizontal spacing per accidental:
                // STD_KS_ACCSPACE = 9*STD_LINEHT/8 STDIST = 9 STDIST
                // In render coords: 9 * lnspace / 8
                let acc_width = lnspace * 9.0 / 8.0;

                let staff_top_y = ddist_to_render(ks_ctx.staff_top);

                let n_items = akeysig.ks_info.n_ks_items;

                if n_items > 0 {
                    // Normal key signature: draw sharps/flats
                    // Reference: DrawUtils.cp:977-987
                    for k in 0..n_items.min(7) as usize {
                        let ks_item = &akeysig.ks_info.ks_item[k];
                        let is_sharp = ks_item.sharp != 0;
                        let halfln = get_ks_y_offset(ks_ctx.clef_type, ks_item.letcode, is_sharp);

                        let x = base_x + k as f32 * acc_width;
                        let y = staff_top_y + (halfln as f32 * lnspace / 2.0);

                        let glyph = if is_sharp { SMUFL_SHARP } else { SMUFL_FLAT };
                        renderer.music_char(x, y, MusicGlyph::smufl(glyph), 100.0);
                    }
                } else {
                    // Cancellation key signature: n_ks_items == 0.
                    // Draw naturals from the previous key signature on this staff.
                    // Reference: DrawUtils.cp:988-1010 — LSSearch for previous keysig
                    let prev = &ks_ctx.prev_ks_info;
                    let prev_n = prev.n_ks_items;
                    if prev_n > 0 {
                        for k in 0..prev_n.min(7) as usize {
                            let ks_item = &prev.ks_item[k];
                            let is_sharp = ks_item.sharp != 0;
                            let halfln =
                                get_ks_y_offset(ks_ctx.clef_type, ks_item.letcode, is_sharp);

                            let x = base_x + k as f32 * acc_width;
                            let y = staff_top_y + (halfln as f32 * lnspace / 2.0);

                            renderer.music_char(x, y, MusicGlyph::smufl(SMUFL_NATURAL), 100.0);
                        }
                    }
                }
            }
        }
    }
}

/// Draw a TimeSig object (time signature numerator/denominator).
///
/// Port of DrawObject.cp DrawTIMESIG() (line 1248).
///
/// For each ATimeSig subobject:
/// - Get context for that staff
/// - If visible:
///   - Extract numerator and denominator
///   - Draw each digit as a music glyph (SMuFL timeSig0-9)
///   - Numerator above middle line, denominator below
///
/// Reference: DrawObject.cp, DrawTIMESIG(), line 1248
pub fn draw_timesig(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let Some(atimesig_list) = score.timesigs.get(&obj.header.first_sub_obj) {
        for atimesig in atimesig_list {
            if let Some(timesig_ctx) = ctx.get(atimesig.header.staffn) {
                // Check visibility
                if timesig_ctx.visible && atimesig.header.visible {
                    // X position: for initial timesigs (before first Measure), use staff_left;
                    // for mid-score timesigs, use measure_left.
                    let origin_x = if timesig_ctx.measure_left > 0 {
                        timesig_ctx.measure_left
                    } else {
                        timesig_ctx.staff_left
                    };
                    let base_x = d2r_sum3(origin_x, obj.header.xd, atimesig.xd);

                    // Line spacing
                    let lnspace =
                        lnspace_for_staff(timesig_ctx.staff_height, timesig_ctx.staff_lines);

                    // Check for common time (C) or cut time (₵) — single glyph centered
                    // on staff. Port of DrawObject.cp:1074 + DrawUtils.cp FillTimeSig().
                    // subType is C_TIME (2) or CUT_TIME (3) from NObjTypes.h:353-355.
                    let sub_type = atimesig.header.sub_type;
                    if sub_type == crate::defs::C_TIME || sub_type == crate::defs::CUT_TIME {
                        // Single glyph at half-line 4 (vertical center of 5-line staff).
                        // SMuFL: U+E08A timeSigCommon, U+E08B timeSigCutCommon
                        let glyph = if sub_type == crate::defs::C_TIME {
                            0xE08A_u32
                        } else {
                            0xE08B_u32
                        };
                        let center_y =
                            ddist_to_render(timesig_ctx.staff_top) + (4.0 * lnspace / 2.0);
                        renderer.music_char(base_x, center_y, MusicGlyph::smufl(glyph), 100.0);
                    } else {
                        // N_OVER_D (or other numeric types): draw numerator/denominator digits.
                        // Y positions: numerator at half-line 2, denominator at half-line 6
                        let num_y = ddist_to_render(timesig_ctx.staff_top) + (2.0 * lnspace / 2.0);
                        let denom_y =
                            ddist_to_render(timesig_ctx.staff_top) + (6.0 * lnspace / 2.0);

                        // Draw numerator digits
                        let num_str = atimesig.numerator.to_string();
                        let mut x_offset = 0.0;
                        for digit_char in num_str.chars() {
                            if let Some(digit) = digit_char.to_digit(10) {
                                let glyph = 0xE080 + digit; // SMuFL timeSig0-9
                                renderer.music_char(
                                    base_x + x_offset,
                                    num_y,
                                    MusicGlyph::smufl(glyph),
                                    100.0,
                                );
                                x_offset += lnspace * 0.8; // Space between digits
                            }
                        }

                        // Draw denominator digits
                        x_offset = 0.0;
                        let denom_str = atimesig.denominator.to_string();
                        for digit_char in denom_str.chars() {
                            if let Some(digit) = digit_char.to_digit(10) {
                                let glyph = 0xE080 + digit;
                                renderer.music_char(
                                    base_x + x_offset,
                                    denom_y,
                                    MusicGlyph::smufl(glyph),
                                    100.0,
                                );
                                x_offset += lnspace * 0.8;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Draw a Slur object (slurs from NGL file with ASlur spline data).
///
/// Ties (slur.tie == true) are skipped here — they're handled by draw_ties()
/// using note-level tied_l/tied_r flags with recomputed Bezier curves.
///
/// Port of DrawSLUR (DrawObject.cp:3054-3160) + GetSlurContext (Slurs.cp:869-980).
///
/// The key fix: endpoints are recomputed at render time from firstSyncL/lastSyncL
/// rather than using stale startPt/endPt stored in the file. The stored Points were
/// absolute screen coordinates from the original rendering context and don't match
/// our freshly computed staff positions.
///
/// Algorithm (from GetSlurContext):
/// 1. Look up firstSyncL/lastSyncL from the Slur object
/// 2. Find the attached notes via slurredR/slurredL flags (GetSlurNoteLinks)
/// 3. Compute absolute DDIST positions: staff_left + sys_rel_xd(sync) + note.xd
/// 4. Apply knot/endKnot/c0/c1 offsets from ASlur spline data
/// 5. Convert DDIST to render coords (÷16) and draw Bezier
pub fn draw_slur(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::defs::MEASURE_TYPE;
    use crate::ngl::interpret::ObjData;
    use crate::render::types::ddist_wide_to_render;

    if let ObjData::Slur(slur) = &obj.data {
        // Skip ties — those are handled by draw_ties() via note-level flags
        if slur.tie {
            return;
        }

        let staffn = slur.ext_header.staffn;
        let staff_ctx = match ctx.get(staffn) {
            Some(c) => c,
            None => return,
        };
        let voice = slur.voice;

        // Check cross-system cases (Slurs.cp:881-889):
        // firstSyncL may be a MEASURE (2nd piece of cross-system slur)
        // lastSyncL may be a SYSTEM (1st piece of cross-system slur)
        let first_is_measure = score
            .get(slur.first_sync_l)
            .is_some_and(|o| o.header.obj_type == MEASURE_TYPE as i8);
        let last_is_system = score
            .get(slur.last_sync_l)
            .is_some_and(|o| matches!(&o.data, ObjData::System(_)));

        // Find the notes at each endpoint (GetSlurNoteLinks, Slurs.cp:809-863)
        let first_note = if !first_is_measure {
            find_slur_note(score, slur.first_sync_l, voice, true)
        } else {
            None
        };
        let last_note = if !last_is_system {
            find_slur_note(score, slur.last_sync_l, voice, false)
        } else {
            None
        };

        // For cross-system slurs, validate that the required note endpoint exists.
        // 1st piece (last_is_system): needs first_note (the real note at slur start)
        // 2nd piece (first_is_measure): needs last_note (the real note at slur end)
        // Reference: GetSlurContext (Slurs.cp:940-967)
        if last_is_system && first_note.is_none() {
            return; // 1st piece but no start note — skip
        }
        if first_is_measure && last_note.is_none() {
            return; // 2nd piece but no end note — skip
        }
        // Normal (non-cross-system) slurs need both endpoints
        if !first_is_measure && !last_is_system && (first_note.is_none() || last_note.is_none()) {
            return;
        }

        // Compute start position in DDIST (GetSlurContext, Slurs.cp:940-949)
        let (xd_first, yd_first) = if first_is_measure {
            // Cross-system 2nd piece: X at measure left, Y from last note
            // For cross-system slurs, both endpoints are required (checked above)
            let meas_xd = score.sys_rel_xd(slur.first_sync_l);
            let yd = last_note.as_ref().map(|n| n.yd as i32).unwrap_or(0);
            (
                staff_ctx.staff_left as i32 + meas_xd,
                staff_ctx.measure_top as i32 + yd,
            )
        } else if let Some(ref note) = first_note {
            let sys_xd = score.sys_rel_xd(slur.first_sync_l);
            (
                staff_ctx.staff_left as i32 + sys_xd + note.xd as i32,
                staff_ctx.measure_top as i32 + note.yd as i32,
            )
        } else {
            return; // Can't find start note, skip
        };

        // Compute end position in DDIST (GetSlurContext, Slurs.cp:951-967)
        // For cross-system slurs: Y interpolation at system boundary
        let (xd_last, yd_last) = if last_is_system {
            // Cross-system 1st piece: extend to staff right edge
            // Y position should be first_note.yd (on the first system)
            let yd = first_note.as_ref().map(|n| n.yd as i32).unwrap_or(0);
            (
                staff_ctx.staff_right as i32,
                staff_ctx.measure_top as i32 + yd,
            )
        } else if let Some(ref note) = last_note {
            let sys_xd = score.sys_rel_xd(slur.last_sync_l);
            (
                staff_ctx.staff_left as i32 + sys_xd + note.xd as i32,
                staff_ctx.measure_top as i32 + note.yd as i32,
            )
        } else {
            return; // Can't find end note, skip
        };

        // Iterate slur subobjects and render Bezier curves
        // (DrawSLUR PostScript path, DrawObject.cp:3086-3146)
        let aslur_list = score.get_slur_subs(obj.header.first_sub_obj, obj.header.n_entries);
        for aslur in &aslur_list {
            if !aslur.visible {
                continue;
            }

            // Apply knot and endKnot offsets (all DDIST, cast i16→i32)
            // Reference: DrawObject.cp:3091-3097
            let start_xd = xd_first + aslur.seg.knot.h as i32;
            let start_yd = yd_first + aslur.seg.knot.v as i32;
            let end_xd = xd_last + aslur.end_knot.h as i32;
            let end_yd = yd_last + aslur.end_knot.v as i32;

            // Control points relative to knot/endKnot (DDIST)
            // Reference: DrawObject.cp:3140-3143
            let c0_xd = start_xd + aslur.seg.c0.h as i32;
            let c0_yd = start_yd + aslur.seg.c0.v as i32;
            let c1_xd = end_xd + aslur.seg.c1.h as i32;
            let c1_yd = end_yd + aslur.seg.c1.v as i32;

            // Convert DDIST to render coords (Points = DDIST / 16)
            let p0 = Point {
                x: ddist_wide_to_render(start_xd),
                y: ddist_wide_to_render(start_yd),
            };
            let c0_pt = Point {
                x: ddist_wide_to_render(c0_xd),
                y: ddist_wide_to_render(c0_yd),
            };
            let c1_pt = Point {
                x: ddist_wide_to_render(c1_xd),
                y: ddist_wide_to_render(c1_yd),
            };
            let p3 = Point {
                x: ddist_wide_to_render(end_xd),
                y: ddist_wide_to_render(end_yd),
            };

            renderer.slur(p0, c0_pt, c1_pt, p3, aslur.dashed);
        }
    }
}

/// Find the note in a sync that a slur attaches to.
///
/// Port of GetSlurNoteLinks (Slurs.cp:809-863). For slurs (not ties),
/// searches for the first note in the given voice with slurredR (start)
/// or slurredL (end) flag set. Falls back to the first note in the voice
/// if no slurred flag is found (handles edge cases in older files).
fn find_slur_note(
    score: &InterpretedScore,
    sync_link: crate::basic_types::Link,
    voice: i8,
    is_start: bool,
) -> Option<crate::obj_types::ANote> {
    use crate::defs::SYNC_TYPE;

    let sync_obj = score.get(sync_link)?;
    if sync_obj.header.obj_type != SYNC_TYPE as i8 {
        return None;
    }
    let notes = score.get_notes(sync_obj.header.first_sub_obj);

    // Primary: find note with matching voice and slurred flag
    let slurred = notes
        .iter()
        .find(|n| n.voice == voice && if is_start { n.slurred_r } else { n.slurred_l });
    if let Some(note) = slurred {
        return Some(note.clone());
    }

    // Fallback: first note in voice (older files may lack slurred flags)
    notes.iter().find(|n| n.voice == voice).cloned()
}

/// Draw ties by matching tied_r notes (starts) to tied_l notes (ends).
///
/// Port of DrawSLUR (DrawObject.cp:3054) + SetSlurCtlPoints (Slurs.cp:1021).
///
/// Matching: For each tie_start, find the first tie_end with the same
/// (staff, voice, note_num) that appears to the right (end.x > start.x).
/// This handles same-system ties. Cross-system ties where the end is on
/// a different system line are handled by checking Y proximity.
///
/// Tie curve shape (from Slurs.cp:1074-1121):
/// - Direction: curves UP when stem is DOWN (and vice versa)
/// - Endpoint horizontal offsets: start at 2/3 of notehead width, end at 1/6
/// - Endpoint vertical offset: 1 staff line space above/below notehead
/// - Control points: symmetric, with height based on span length
///   Short spans (<3 lnSp): config.tieCurvature * lnSp / 100
///   Long spans (>6 lnSp): span/16
///   Medium: linear blend
pub fn draw_ties(
    tie_starts: &[TieEndpoint],
    tie_ends: &[TieEndpoint],
    renderer: &mut dyn MusicRenderer,
) {
    // Default tie curvature: 85 percent of lnSpace (config.tieCurvature from OG)
    const TIE_CURVATURE: f32 = 85.0;

    for start in tie_starts {
        // Find matching end: same staff, voice, pitch, appearing to the right
        let matching_end = tie_ends.iter().find(|end| {
            end.staff == start.staff
                && end.voice == start.voice
                && end.note_num == start.note_num
                && end.x > start.x
        });

        if let Some(end) = matching_end {
            let lnsp = start.lnspace;

            // Tie direction: curve UP when stem is DOWN (OG: NewSlur.cp:939-947)
            let curve_up = start.stem_down;

            // Endpoint horizontal offsets (Slurs.cp:1045-1058)
            // Start: 2/3 of notehead width from left edge
            let start_x = start.x + (2.0 * start.head_width) / 3.0;
            // End: 1/6 of notehead width from left edge
            let end_x = end.x + end.head_width / 6.0;

            // Endpoint vertical offset: 1 lnSpace above/below note center (Slurs.cp:1063)
            let vert_offset = if curve_up { -lnsp } else { lnsp };
            let start_y = start.y + vert_offset;
            let end_y = end.y + vert_offset;

            // Control point computation (Slurs.cp:1074-1121)
            let span = end_x - start_x;

            // Short-slur control point distance
            let x0_short = 2.0 * lnsp; // SCALECURVE(dLineSp) = 2*lnSp
            let y0_short = TIE_CURVATURE * lnsp / 100.0;

            // Long-slur control point distance
            let x0_long = span / 4.0;
            let y0_long = span / 16.0;

            // Blend based on span (Slurs.cp:1088-1107)
            let short_threshold = 3.0 * lnsp;
            let long_threshold = 6.0 * lnsp;

            let (cx_offset, cy_offset) = if span <= short_threshold {
                (x0_short, y0_short)
            } else if span >= long_threshold {
                (x0_long, y0_long)
            } else {
                // Linear interpolation
                let t = (span - short_threshold) / (long_threshold - short_threshold);
                (
                    x0_short + t * (x0_long - x0_short),
                    y0_short + t * (y0_long - y0_short),
                )
            };

            // Control points: offset from endpoints
            // c1 is relative to start, c2 is relative to end
            let cy = if curve_up { -cy_offset } else { cy_offset };

            let c1 = Point {
                x: start_x + cx_offset,
                y: start_y + cy,
            };
            let c2 = Point {
                x: end_x - cx_offset,
                y: end_y + cy,
            };

            let p0 = Point {
                x: start_x,
                y: start_y,
            };
            let p3 = Point { x: end_x, y: end_y };

            renderer.slur(p0, c1, c2, p3, false);
        } else {
            // Cross-system tie: draw partial arc from start note to right edge of staff
            let lnsp = start.lnspace;
            let curve_up = start.stem_down;
            let start_x = start.x + (2.0 * start.head_width) / 3.0;
            let end_x = start.staff_right; // Extend to right edge of system
            let vert_offset = if curve_up { -lnsp } else { lnsp };
            let start_y = start.y + vert_offset;
            let end_y = start_y; // Same Y (horizontal partial arc)

            let span = end_x - start_x;
            if span > 0.0 {
                let cx = span / 3.0;
                let cy_offset = TIE_CURVATURE * lnsp / 100.0;
                let cy = if curve_up { -cy_offset } else { cy_offset };

                renderer.slur(
                    Point {
                        x: start_x,
                        y: start_y,
                    },
                    Point {
                        x: start_x + cx,
                        y: start_y + cy,
                    },
                    Point {
                        x: end_x - cx,
                        y: end_y + cy,
                    },
                    Point { x: end_x, y: end_y },
                    false,
                );
            }
        }
    }

    // Draw partial ties for unmatched tie_ends (incoming cross-system ties).
    // These start at the left edge of the staff and arc to the note.
    let mut matched_ends: Vec<bool> = vec![false; tie_ends.len()];
    for start in tie_starts {
        for (i, end) in tie_ends.iter().enumerate() {
            if !matched_ends[i]
                && end.staff == start.staff
                && end.voice == start.voice
                && end.note_num == start.note_num
                && end.x > start.x
            {
                matched_ends[i] = true;
                break;
            }
        }
    }

    for (i, end) in tie_ends.iter().enumerate() {
        if matched_ends[i] {
            continue; // Already drawn as part of a matched tie
        }
        // Cross-system incoming tie: draw partial arc from left edge to note
        let lnsp = end.lnspace;
        let curve_up = end.stem_down;
        let start_x = end.staff_left; // Start at left edge of system
        let end_x = end.x + end.head_width / 6.0;
        let vert_offset = if curve_up { -lnsp } else { lnsp };
        let start_y = end.y + vert_offset;
        let end_y = start_y;

        let span = end_x - start_x;
        if span > 0.0 {
            let cx = span / 3.0;
            let cy_offset = TIE_CURVATURE * lnsp / 100.0;
            let cy = if curve_up { -cy_offset } else { cy_offset };

            renderer.slur(
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point {
                    x: start_x + cx,
                    y: start_y + cy,
                },
                Point {
                    x: end_x - cx,
                    y: end_y + cy,
                },
                Point { x: end_x, y: end_y },
                false,
            );
        }
    }
}

/// Draw slurs from collected endpoint data (Notelist pipeline).
///
/// Port of IICreateAllSlurs (InternalInput.cp:881-918) matching + SetSlurCtlPoints
/// (Slurs.cp:1021-1122) control point computation.
///
/// Differs from ties in two ways:
/// 1. Matching: by voice only (first slurred_r → next slurred_l in same voice),
///    not by pitch. This matches the OG IICreateAllSlurs algorithm.
/// 2. Curvature: uses config.slurCurvature (default 50) instead of tieCurvature (85).
///
/// NGL slurs use pre-stored ASlur spline data and go through draw_slur() instead.
pub fn draw_slurs_from_endpoints(
    slur_starts: &[TieEndpoint],
    slur_ends: &[TieEndpoint],
    renderer: &mut dyn MusicRenderer,
) {
    // OG: config.slurCurvature default = 50 (Initialize.cp:974)
    const SLUR_CURVATURE: f32 = 50.0;

    // Track which ends have been matched (each end can only be used once)
    let mut matched_ends: Vec<bool> = vec![false; slur_ends.len()];

    for start in slur_starts {
        // OG IICreateAllSlurs: search forward in same voice for first slurred_l note
        // Match by voice only, taking the closest (leftmost) unmatched end to the right
        let mut best_idx: Option<usize> = None;
        let mut best_x = f32::MAX;

        for (i, end) in slur_ends.iter().enumerate() {
            if matched_ends[i] {
                continue;
            }
            if end.voice == start.voice && end.x > start.x && end.x < best_x {
                best_idx = Some(i);
                best_x = end.x;
            }
        }

        if let Some(idx) = best_idx {
            matched_ends[idx] = true;
            let end = &slur_ends[idx];
            let lnsp = start.lnspace;

            // Slur direction: curve UP when stem is DOWN (OG: SetAllSlursShape)
            let curve_up = start.stem_down;

            // Endpoint horizontal offsets (Slurs.cp:1045-1058)
            let start_x = start.x + (2.0 * start.head_width) / 3.0;
            let end_x = end.x + end.head_width / 6.0;

            // Endpoint vertical offset: 1 lnSpace above/below note center (Slurs.cp:1063)
            let vert_offset = if curve_up { -lnsp } else { lnsp };
            let start_y = start.y + vert_offset;
            let end_y = end.y + vert_offset;

            // Control point computation (Slurs.cp:1074-1121)
            let span = end_x - start_x;
            if span <= 0.0 {
                continue;
            }

            // Short-slur control point distance
            // OG: SCALECURVE(dLineSp) = 4*(z)/2 = 2*z, capped at span/3
            let mut x0_short = 2.0 * lnsp;
            if x0_short > span / 3.0 {
                x0_short = span / 3.0;
            }
            let y0_short = SLUR_CURVATURE * lnsp / 100.0;

            // Long-slur control point distance
            let x0_long = span / 4.0;
            let y0_long = span / 16.0;

            // Blend based on span (Slurs.cp:1088-1107)
            // OG: tmp = 6*dLineSp; long if span > 2*tmp, blend if span > tmp
            let short_threshold = 6.0 * lnsp;
            let long_threshold = 12.0 * lnsp;

            let (cx_offset, cy_offset) = if span > long_threshold {
                (x0_long, y0_long)
            } else if span > short_threshold {
                let t = (span - short_threshold) / (long_threshold - short_threshold);
                (
                    x0_short + t * (x0_long - x0_short),
                    y0_short + t * (y0_long - y0_short),
                )
            } else {
                (x0_short, y0_short)
            };

            // Control points: offset from endpoints (symmetric, then rotated)
            let cy = if curve_up { -cy_offset } else { cy_offset };

            // Before rotation: c0 = (cx_offset, cy), c1 = (-cx_offset, cy)
            // Rotation for slanted slurs (Slurs.cp:1114-1122 + RotateSlurCtrlPts)
            let dx = end_x - start_x;
            let dy = end_y - start_y;
            let r = (dx * dx + dy * dy).sqrt();

            let (c0_h, c0_v, c1_h, c1_v) = if r > 0.0 {
                let cs = dx / r;
                let sn = dy / r;
                // Rotate c0 = (cx_offset, cy)
                let c0h = cx_offset * cs - cy * sn;
                let c0v = cx_offset * sn + cy * cs;
                // Rotate c1 = (-cx_offset, cy)
                let c1h = -cx_offset * cs - cy * sn;
                let c1v = -cx_offset * sn + cy * cs;
                (c0h, c0v, c1h, c1v)
            } else {
                (cx_offset, cy, -cx_offset, cy)
            };

            let c1 = Point {
                x: start_x + c0_h,
                y: start_y + c0_v,
            };
            let c2 = Point {
                x: end_x + c1_h,
                y: end_y + c1_v,
            };

            let p0 = Point {
                x: start_x,
                y: start_y,
            };
            let p3 = Point { x: end_x, y: end_y };

            renderer.slur(p0, c1, c2, p3, false);
        } else {
            // Cross-system slur (outgoing): draw partial arc from note to right edge of staff.
            // Port of Slurs.cp GetSlurContext lines 951-967: when lastSyncL=System, the slur's
            // right endpoint is at the staff right edge, Y held constant from start.
            let lnsp = start.lnspace;
            let curve_up = start.stem_down;
            let start_x = start.x + (2.0 * start.head_width) / 3.0;
            let end_x = start.staff_right; // Extend to right edge of current system
            let vert_offset = if curve_up { -lnsp } else { lnsp };
            let start_y = start.y + vert_offset;
            let end_y = start_y; // Horizontal partial arc (Y unchanged at system edge)

            let span = end_x - start_x;
            if span > 0.0 {
                // Short-arc control points (capped at span/3)
                let mut cx = 2.0 * lnsp;
                if cx > span / 3.0 {
                    cx = span / 3.0;
                }
                let cy_offset = SLUR_CURVATURE * lnsp / 100.0;
                let cy = if curve_up { -cy_offset } else { cy_offset };

                renderer.slur(
                    Point {
                        x: start_x,
                        y: start_y,
                    },
                    Point {
                        x: start_x + cx,
                        y: start_y + cy,
                    },
                    Point {
                        x: end_x - cx,
                        y: end_y + cy,
                    },
                    Point { x: end_x, y: end_y },
                    false,
                );
            }
        }
    }

    // Draw partial slur arcs for unmatched slur_ends (incoming cross-system continuation).
    // Port of Slurs.cp GetSlurContext lines 908-944: when firstSyncL=Measure, the slur's
    // left endpoint is at the left edge of the first measure on the new system.
    let mut matched_ends: Vec<bool> = vec![false; slur_ends.len()];
    for start in slur_starts {
        for (i, end) in slur_ends.iter().enumerate() {
            if !matched_ends[i] && end.voice == start.voice && end.x > start.x {
                matched_ends[i] = true;
                break;
            }
        }
    }

    for (i, end) in slur_ends.iter().enumerate() {
        if matched_ends[i] {
            continue; // Already drawn as part of a matched slur
        }
        // Cross-system incoming slur: draw partial arc from left edge of staff to note
        let lnsp = end.lnspace;
        let curve_up = end.stem_down;
        let start_x = end.staff_left; // Start at left edge of current system
        let end_x = end.x + end.head_width / 6.0;
        let vert_offset = if curve_up { -lnsp } else { lnsp };
        let start_y = end.y + vert_offset;
        let end_y = start_y; // Horizontal partial arc

        let span = end_x - start_x;
        if span > 0.0 {
            let mut cx = 2.0 * lnsp;
            if cx > span / 3.0 {
                cx = span / 3.0;
            }
            let cy_offset = SLUR_CURVATURE * lnsp / 100.0;
            let cy = if curve_up { -cy_offset } else { cy_offset };

            renderer.slur(
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point {
                    x: start_x + cx,
                    y: start_y + cy,
                },
                Point {
                    x: end_x - cx,
                    y: end_y + cy,
                },
                Point { x: end_x, y: end_y },
                false,
            );
        }
    }
}

// =============================================================================
// DYNAMIC rendering — port of DrawDYNAMIC (DrawObject.cp:1226-1324)
// =============================================================================

/// SMuFL codepoints for dynamic text markings.
/// Range U+E520..U+E54F (dynamics).
/// Reference: SMuFL spec §4.52 "Dynamics"
#[allow(dead_code)]
mod dyn_glyphs {
    pub const DYNAMIC_PIANO: u32 = 0xE520;
    pub const DYNAMIC_MEZZO: u32 = 0xE521;
    pub const DYNAMIC_FORTE: u32 = 0xE522;
    pub const DYNAMIC_RINFORZANDO: u32 = 0xE523; // 'r' used in rf, rfz
    pub const DYNAMIC_SFORZANDO: u32 = 0xE524; // 's' used in sf, sfz
    pub const DYNAMIC_Z: u32 = 0xE525; // 'z' used in fz, sfz, rfz

    // Combined dynamics (single glyphs for common markings)
    pub const DYNAMIC_PPPPPP: u32 = 0xE527;
    pub const DYNAMIC_PPPPP: u32 = 0xE528;
    pub const DYNAMIC_PPPP: u32 = 0xE529;
    pub const DYNAMIC_PPP: u32 = 0xE52A;
    pub const DYNAMIC_PP: u32 = 0xE52B;
    pub const DYNAMIC_MP: u32 = 0xE52C;
    pub const DYNAMIC_MF: u32 = 0xE52D;
    pub const DYNAMIC_FF: u32 = 0xE52F;
    pub const DYNAMIC_FFF: u32 = 0xE530;
    pub const DYNAMIC_FFFF: u32 = 0xE531;
    pub const DYNAMIC_FFFFF: u32 = 0xE532;
    pub const DYNAMIC_FFFFFF: u32 = 0xE533;
    pub const DYNAMIC_FP: u32 = 0xE534;
    pub const DYNAMIC_FZ: u32 = 0xE535;
    pub const DYNAMIC_SF: u32 = 0xE536;
    pub const DYNAMIC_SFP: u32 = 0xE537;
    pub const DYNAMIC_SFZ: u32 = 0xE53B;
    pub const DYNAMIC_RF: u32 = 0xE53C;
    pub const DYNAMIC_RFZ: u32 = 0xE53D;
}

/// Map a Nightingale dynamic type (1-21) to a SMuFL glyph codepoint.
///
/// OG mapping: DrawUtils.cp GetDynamicDrawInfo() lines 495-520, defs.h MCH_* constants.
/// Nightingale uses Sonata font chars; we map to SMuFL combined dynamic glyphs.
fn dynamic_type_to_smufl(dynamic_type: i8) -> Option<u32> {
    match dynamic_type {
        1 => Some(dyn_glyphs::DYNAMIC_PPPP),  // PPPP_DYNAM
        2 => Some(dyn_glyphs::DYNAMIC_PPP),   // PPP_DYNAM
        3 => Some(dyn_glyphs::DYNAMIC_PP),    // PP_DYNAM
        4 => Some(dyn_glyphs::DYNAMIC_PIANO), // P_DYNAM (single p)
        5 => Some(dyn_glyphs::DYNAMIC_MP),    // MP_DYNAM
        6 => Some(dyn_glyphs::DYNAMIC_MF),    // MF_DYNAM
        7 => Some(dyn_glyphs::DYNAMIC_FORTE), // F_DYNAM (single f)
        8 => Some(dyn_glyphs::DYNAMIC_FF),    // FF_DYNAM
        9 => Some(dyn_glyphs::DYNAMIC_FFF),   // FFF_DYNAM
        10 => Some(dyn_glyphs::DYNAMIC_FFFF), // FFFF_DYNAM
        // Relative dynamics (11-14): più piano, meno piano, meno forte, più forte
        // No standard SMuFL combined glyph — build from individual chars
        11 => Some(dyn_glyphs::DYNAMIC_PIANO), // più p (TODO: composite)
        12 => Some(dyn_glyphs::DYNAMIC_PIANO), // meno p (TODO: composite)
        13 => Some(dyn_glyphs::DYNAMIC_FORTE), // meno f (TODO: composite)
        14 => Some(dyn_glyphs::DYNAMIC_FORTE), // più f (TODO: composite)
        // Sforzando variants (15-21)
        15 => Some(dyn_glyphs::DYNAMIC_SF),  // SF_DYNAM
        16 => Some(dyn_glyphs::DYNAMIC_FZ),  // FZ_DYNAM
        17 => Some(dyn_glyphs::DYNAMIC_SFZ), // SFZ_DYNAM
        18 => Some(dyn_glyphs::DYNAMIC_RF),  // RF_DYNAM
        19 => Some(dyn_glyphs::DYNAMIC_RFZ), // RFZ_DYNAM
        20 => Some(dyn_glyphs::DYNAMIC_FP),  // FP_DYNAM
        21 => Some(dyn_glyphs::DYNAMIC_SFP), // SFP_DYNAM
        _ => None,                           // Hairpins (22-23) or unknown
    }
}

/// Draw a Dynamic object — hairpins and text dynamic markings.
///
/// Port of DrawDYNAMIC (DrawObject.cp:1226-1324) and DrawHairpin (DrawObject.cp:1129-1219).
///
/// For hairpins (dim/cresc):
///   - Two lines forming a wedge, with mouth on one end and point on the other
///   - DIM_DYNAM (22): mouth at start, point at end (diminuendo ">")
///   - CRESC_DYNAM (23): point at start, mouth at end (crescendo "<")
///   - Line width: config.hairpinLW (12%) * lnSpace / 100
///   - Rise = qd2d(mouthWidth) / 2, offset = qd2d(otherWidth) / 2
///
/// For text dynamics (pp, f, mf, sf, etc.):
///   - Position at firstSyncL.xd + aDynamic.xd, measureTop + aDynamic.yd
///   - SMuFL glyph from dynamic_type_to_smufl()
///
/// Reference: DrawObject.cp:1226-1324, DrawUtils.cp:466-520, PS_Stdio.cp:1351
pub fn draw_dynamic(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::ngl::interpret::ObjData;
    use crate::obj_types::FIRSTHAIRPIN_DYNAM;
    use crate::render::types::ddist_wide_to_render;

    let dyn_obj = match &obj.data {
        ObjData::Dynamic(d) => d,
        _ => return,
    };

    // Get ADYNAMIC subobjects
    let adynamic_list = match score.dynamics.get(&obj.header.first_sub_obj) {
        Some(subs) => subs,
        None => return,
    };

    // config.hairpinLW = 12 (percent of a space)
    // Reference: Initialize.cp:964, HAIRPINLW_DFLT = 12
    const HAIRPIN_LW_PCT: i32 = 12;

    for adynamic in adynamic_list {
        // Get staff context
        let staff_ctx = match ctx.get(adynamic.header.staffn) {
            Some(c) => c,
            None => continue,
        };

        if !staff_ctx.visible || !adynamic.header.visible {
            continue;
        }

        let staff_height = staff_ctx.staff_height as i32;
        let staff_lines = staff_ctx.staff_lines as i32;
        let lnspace_ddist = if staff_lines > 1 {
            staff_height / (staff_lines - 1)
        } else {
            8 // default
        };

        // Compute X position from firstSyncL using page-relative coordinates.
        // OG: xd = SysRelxd(DynamFIRSTSYNC(pL)) + LinkXD(pL) + aDynamic->xd + systemLeft
        // Our model: staff_left + sys_rel_xd(firstSync) + obj.xd + aDynamic.xd
        // Reference: DSUtils.cp:309-311 (PageRelxd for DYNAMtype)
        let first_sync_sys_xd = score.sys_rel_xd(dyn_obj.first_sync_l);

        let xd = staff_ctx.staff_left as i32
            + first_sync_sys_xd
            + obj.header.xd as i32
            + adynamic.xd as i32;

        // yd = measureTop + aDynamic.yd
        let yd = staff_ctx.measure_top as i32 + adynamic.yd as i32;

        let is_hairpin = (dyn_obj.dynamic_type as u8) >= FIRSTHAIRPIN_DYNAM;

        if is_hairpin {
            // ---- Hairpin rendering ----
            // Port of DrawHairpin (DrawObject.cp:1129-1219), PostScript path

            // Compute endxd from lastSyncL
            // OG: if SystemTYPE(lastSyncL) → endxd = aDynamic->endxd + sysLeft
            //     else → endxd = SysRelxd(lastSyncL) + aDynamic->endxd + sysLeft
            let last_sync_sys_xd = match score.get(dyn_obj.last_sync_l) {
                Some(sync_obj) => {
                    // Check if lastSyncL is a System object (cross-system hairpin)
                    if matches!(&sync_obj.data, ObjData::System(_)) {
                        // Cross-system: endxd relative to system left only
                        0i32
                    } else {
                        score.sys_rel_xd(dyn_obj.last_sync_l)
                    }
                }
                None => continue,
            };

            let endxd = staff_ctx.staff_left as i32 + last_sync_sys_xd + adynamic.endxd as i32;
            let endyd = staff_ctx.measure_top as i32 + adynamic.endyd as i32;

            // Convert mouthWidth/otherWidth from quarter-DDIST to DDIST
            // qd2d(qd, stfHt, lines) = (qd * stfHt) / (4 * (lines - 1))
            // then divide by 2 for half-rise/offset
            let divisor = if staff_lines > 1 {
                4 * (staff_lines - 1)
            } else {
                4
            };
            let rise = (adynamic.mouth_width as i32 * staff_height) / divisor / 2;
            let offset = (adynamic.other_width as i32 * staff_height) / divisor / 2;

            // Line thickness: hairpinLW * lnSpace / 100
            let hair_thick = (HAIRPIN_LW_PCT * lnspace_ddist) / 100;
            let thick_r = ddist_wide_to_render(hair_thick).max(0.25);

            // Convert positions to render coords
            let x0 = ddist_wide_to_render(xd);
            let y0 = ddist_wide_to_render(yd);
            let x1 = ddist_wide_to_render(endxd);
            let y1 = ddist_wide_to_render(endyd);
            let rise_r = ddist_wide_to_render(rise);
            let offset_r = ddist_wide_to_render(offset);

            match dyn_obj.dynamic_type as u8 {
                22 => {
                    // DIM_DYNAM: mouth at start (rise), point at end (offset)
                    // Top line: (x0, y0+rise) → (x1, y1+offset)
                    // Bottom line: (x0, y0-rise) → (x1, y1-offset)
                    renderer.line(x0, y0 + rise_r, x1, y1 + offset_r, thick_r);
                    renderer.line(x0, y0 - rise_r, x1, y1 - offset_r, thick_r);
                }
                23 => {
                    // CRESC_DYNAM: point at start (offset), mouth at end (rise)
                    // Top line: (x0, y0+offset) → (x1, y1+rise)
                    // Bottom line: (x0, y0-offset) → (x1, y1-rise)
                    renderer.line(x0, y0 + offset_r, x1, y1 + rise_r, thick_r);
                    renderer.line(x0, y0 - offset_r, x1, y1 - rise_r, thick_r);
                }
                _ => {}
            }
        } else {
            // ---- Text dynamic rendering ----
            // Port of DrawDYNAMIC text path (DrawObject.cp:1290-1319), PostScript path

            let glyph_cp = match dynamic_type_to_smufl(dyn_obj.dynamic_type) {
                Some(cp) => cp,
                None => continue,
            };

            let x = ddist_wide_to_render(xd);
            let y = ddist_wide_to_render(yd);

            let size_pct = if adynamic.small != 0 { 75.0 } else { 100.0 };
            renderer.music_char(x, y, MusicGlyph::smufl(glyph_cp), size_pct);
        }
    }
}

/// Draw a GRAPHIC object (text strings, lyrics, rehearsal marks, etc.).
///
/// Handles GRString, GRLyric, GRRehearsal, GRChordSym subtypes by rendering
/// the resolved text from the string pool at the GRAPHIC's position.
///
/// Position computation follows the OG DrawGRAPHIC (DrawObject.cp:1983-2224):
///   x = measure_left + firstObj.xd + graphic.xd
///   y = measure_top + graphic.yd
///
/// Font is determined by:
///   1. The text style index in graphic.info (FONT_MN, FONT_PN, FONT_R1, etc.)
///   2. Or the graphic's own fontInd/fontSize/fontStyle fields (FONT_THISITEMONLY=0)
///
/// Reference: DrawObject.cp:1983-2224, DrawObject.cp:1808-1969 (DrawTextBlock)
pub fn draw_graphic(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::ngl::interpret::ObjData;
    use crate::obj_types::GraphicType;
    use crate::render::types::ddist_wide_to_render;

    let gfx = match &obj.data {
        ObjData::Graphic(g) => g,
        _ => return,
    };

    // Only handle text-based graphic types for now
    let gtype = match gfx.graphic_type as u8 {
        x if x == GraphicType::GrString as u8 => GraphicType::GrString,
        x if x == GraphicType::GrLyric as u8 => GraphicType::GrLyric,
        x if x == GraphicType::GrRehearsal as u8 => GraphicType::GrRehearsal,
        x if x == GraphicType::GrChordSym as u8 => GraphicType::GrChordSym,
        _ => return, // Skip GRDraw, GRArpeggio, etc. for now
    };

    // Get the resolved text string, stripping trailing control chars (DEL, NUL)
    // that appear in OG Nightingale chord symbol string pools.
    let raw_text = match score.graphic_strings.get(&obj.header.first_sub_obj) {
        Some(s) if !s.is_empty() => s.as_str(),
        _ => return, // No text to render
    };

    // For chord symbols, normalize the 0x7F-delimited multi-field format and
    // replace Sonata accidental codes with Unicode equivalents before rendering.
    // Reference: ChordSym.cp ParseChordSym() + DrawChordSym()
    let normalized_cs;
    let text = if gtype == GraphicType::GrChordSym {
        normalized_cs = normalize_chord_symbol(raw_text);
        normalized_cs.as_str()
    } else {
        raw_text.trim_end_matches(['\x7f', '\0'])
    };
    if text.is_empty() {
        return;
    }

    // Determine which staff context to use
    // NB: staffn can be 0 for page-relative graphics; use staff 1 as fallback
    let staffn = if gfx.ext_header.staffn > 0 {
        gfx.ext_header.staffn
    } else {
        1
    };
    let staff_ctx = match ctx.get(staffn) {
        Some(c) => c,
        None => return,
    };

    if !staff_ctx.visible {
        return;
    }

    // Compute X/Y position.
    // Two cases based on whether firstObj is a PAGE (page-relative) or not.
    //
    // Page-relative GRAPHICs (firstObj is PAGE):
    //   xd = graphic.xd  (absolute page coordinates, no staff/system offsets)
    //   yd = graphic.yd
    //   Reference: DrawUtils.cp:2431-2436 (GetGraphicOrTempoDrawInfo, PageTYPE branch)
    //
    // Staff-relative GRAPHICs (firstObj is Sync/Measure/etc.):
    //   xd = staff_left + SysRelxd(firstObj) + graphic.xd
    //   yd = measureTop + graphic.yd
    //   Reference: DSUtils.cp:312-320 (PageRelxd for GRAPHICtype)
    let (xd, yd) = if score.is_page_type(gfx.first_obj) {
        // Page-relative: only the GRAPHIC's own offsets, no staff/system positioning.
        (obj.header.xd as i32, obj.header.yd as i32)
    } else {
        // Staff-relative: compute staff_left from the anchor's enclosing system/staff,
        // NOT from the current ContextState (which may reflect a later system).
        // Reference: DrawUtils.cp:2438-2447 (GetGraphicOrTempoDrawInfo, staff-relative path)
        let anchor_staff_left = score
            .staff_left_at(gfx.first_obj, staffn)
            .unwrap_or(staff_ctx.staff_left);
        let first_obj_sys_xd = score.sys_rel_xd(gfx.first_obj);
        let xd = anchor_staff_left as i32 + first_obj_sys_xd + obj.header.xd as i32;
        // Use staff_top when measure_top is 0: GRAPHIC objects can appear before
        // any MEASURE in the object list (e.g. title text attached to preamble clef),
        // so measure_top may not yet be populated. Same pattern as draw_tempo.
        // Reference: DrawUtils.cp:2438 GetGraphicOrTempoDrawInfo
        let y_base = if staff_ctx.measure_top != 0 {
            staff_ctx.measure_top
        } else {
            staff_ctx.staff_top
        };
        let yd = y_base as i32 + obj.header.yd as i32;
        (xd, yd)
    };

    let x = ddist_wide_to_render(xd);
    let y = ddist_wide_to_render(yd);

    // Compute lineSpace for relative font size resolution
    // lineSpace = staff_height / (staff_lines - 1), in DDIST
    // Reference: defs.h LNSPACE macro
    // For page-relative GRAPHICs at the start of a system, staff_height might
    // be zero (context not yet populated). Fall back to the standard 5-line
    // staff spacing: 4 * STD_LINEHT * 2 = 72 DDIST (used by rastral 5).
    let line_space = if staff_ctx.staff_lines > 1 && staff_ctx.staff_height > 0 {
        staff_ctx.staff_height / (staff_ctx.staff_lines as i16 - 1)
    } else if staff_ctx.staff_height > 0 {
        staff_ctx.staff_height
    } else {
        72 // standard LNSPACE fallback (4 interline spaces at rastral 5 = 72 DDIST)
    };

    // Determine font from text style or GRAPHIC's own fields
    // Mac fontStyle bits: 0=normal, bit0=bold, bit1=italic, bit2=underline, etc.
    let is_sonata = is_sonata_font(gfx, score);
    let font = resolve_graphic_font(gfx, score, &gtype, line_space);

    // If the original font is a Sonata-compatible music font, render each character
    // as a SMuFL music glyph rather than as text. Sonata/Briard/etc. music fonts use
    // the same character encoding where '%' = segno, 'U' = fermata, etc.
    //
    // Text has been converted from Mac Roman → UTF-8 during parsing, so we must
    // convert each UTF-8 char back to its original Mac Roman byte value before
    // looking up the SMuFL mapping.
    //
    // Reference: MusicFont.cp MapMusChar(), defs.h MCH_* constants
    if is_sonata {
        use super::draw_utils::utf8_music_char_to_smufl;
        use crate::render::types::MusicGlyph;

        // Compute size_percent from font size relative to the music font size
        // (line_space * 4 is roughly the staff height; standard music font = ~24pt)
        let size_pct = (font.size / 24.0 * 100.0).max(50.0);
        let mut char_x = x;
        for ch in text.chars() {
            if ch == ' ' {
                // Space: advance by ~0.5 em width
                char_x += font.size * 0.5;
                continue;
            }
            if ch == '~' {
                // Tilde in music font context often means "hairpin" or space-like separator
                char_x += font.size * 0.3;
                continue;
            }
            if let Some(smufl_cp) = utf8_music_char_to_smufl(ch) {
                renderer.music_char(char_x, y, MusicGlyph::Smufl(smufl_cp), size_pct);
                // Advance by approximate glyph width (~1.0 em for most music chars)
                char_x += font.size * 1.0;
            } else if ch.is_ascii_graphic() {
                // Unmapped ASCII chars (e.g. 'd' in "D.S." text) — render as text
                // using the resolved font. This handles music fonts like Briard that
                // contain both Sonata glyphs AND regular letterforms.
                let s = ch.to_string();
                renderer.text_string(char_x, y, &s, &font);
                char_x += font.size * 0.5; // narrower than music glyphs
            }
        }
        return;
    }

    // Handle multiline text (lines delimited by CR = '\r')
    // Reference: DrawTextBlock, DrawObject.cp:1808-1969
    let text_draw_x;
    if gfx.multi_line != 0 && text.contains('\r') {
        let line_spacing = font.size * 1.2; // ~120% line spacing
        for (i, line) in text.split('\r').enumerate() {
            if !line.is_empty() {
                // Apply justification per-line so each line is independently aligned
                let line_x = apply_text_justification(x, line, &font, gfx.justify, renderer);
                let line_y = y + (i as f32) * line_spacing;
                renderer.text_string(line_x, line_y, line, &font);
            }
        }
        text_draw_x = x; // approximate for enclosure
    } else {
        // Apply text justification offset.
        // Reference: NObjTypes.h:612-617 (GRJustLeft, GRJustRight, GRJustCenter)
        // For right-justified text, xd marks the right edge; for centered, the center.
        text_draw_x = apply_text_justification(x, text, &font, gfx.justify, renderer);
        renderer.text_string(text_draw_x, y, text, &font);
    }

    // Draw enclosure (box or circle) around the text if requested.
    // Reference: DrawObject.cp:2211-2213 (DrawEnclosure call)
    // Reference: DrawObject.cp:1490-1535 (DrawEnclosure function)
    //
    // OG defaults:
    //   enclLW = 4 quarter-points = 1.0 pt (NDocAndCnfgTypes.h:504, Initialize.cp:956)
    //   enclMargin = 2 points (NDocAndCnfgTypes.h:585, Initialize.cp:1129)
    //   enclWidthOffset = 0 points (NDocAndCnfgTypes.h:598)
    use crate::obj_types::EnclosureType;
    if gfx.enclosure != EnclosureType::EnclNone as u8 {
        let encl_margin = 2.0_f32; // points (ENCLMARGIN_DFLT)
        let encl_lw = 1.0_f32; // points (ENCLLW_DFLT=4 qtr-pts)

        // Compute text bounding box
        let text_width = renderer
            .measure_text_width(text, &font)
            .unwrap_or(text.len() as f32 * font.size * 0.55);
        // Text height: approximate as font ascent (~0.8 * font size for typical fonts).
        // The text baseline is at y; text extends upward by ~ascent.
        let text_height = font.size * 0.8;

        // Build the enclosure rect: text bbox expanded by margin on all sides.
        // x = text left edge - margin, y = baseline - ascent - margin
        let rect_x = text_draw_x - encl_margin;
        let rect_y = y - text_height - encl_margin;
        let rect_w = text_width + 2.0 * encl_margin;
        let rect_h = text_height + 2.0 * encl_margin;

        if gfx.enclosure == EnclosureType::EnclBox as u8 {
            // PS_FrameRect(dBox, dEnclThick)
            let rect = crate::render::types::RenderRect::new(rect_x, rect_y, rect_w, rect_h);
            renderer.frame_rect(&rect, encl_lw);
        }
        // ENCL_CIRCLE is #ifdef NOTYET in OG — not implemented
    }
}

/// Adjust x-position based on GRAPHIC text justification.
///
/// For left-justified text (default), x is the left edge.
/// For right-justified text, x is the right edge — subtract text width.
/// For centered text, x is the center — subtract half the text width.
///
/// If the renderer doesn't support text measurement, falls back to an
/// approximate width estimate (0.5 × font_size × character count).
fn apply_text_justification(
    x: f32,
    text: &str,
    font: &crate::render::types::TextFont,
    justify: u8,
    renderer: &dyn crate::render::MusicRenderer,
) -> f32 {
    use crate::defs::{GR_JUST_CENTER, GR_JUST_RIGHT};

    match justify {
        GR_JUST_RIGHT => {
            let w = renderer
                .measure_text_width(text, font)
                .unwrap_or(text.len() as f32 * font.size * 0.5);
            x - w
        }
        GR_JUST_CENTER => {
            let w = renderer
                .measure_text_width(text, font)
                .unwrap_or(text.len() as f32 * font.size * 0.5);
            x - w * 0.5
        }
        _ => x, // GR_JUST_LEFT or 0 (default)
    }
}

/// Convert a GrRelSize code to point size using staff line spacing.
///
/// OG Nightingale formula (Utility.cp:1132-1141, vars.h:350):
///   relFSizeTab = [1.0, 1.5, 1.7, 2.0, 2.2, 2.5, 3.0, 3.6, 0, 4.0]
///   point_size = d2pt(relFSizeTab[code] * lineSpace)
///   d2pt(d) = (d + 8) >> 4    (DDIST → points with rounding)
///
/// lineSpace is in DDIST units (staff_height / (staff_lines - 1)).
/// MIN_TEXT_SIZE = 4pt enforced.
fn rel_size_to_pt(code: u8, line_space: i16) -> f32 {
    // relFSizeTab from vars.h:350
    // Index:  0    1     2     3     4     5     6     7    8   9
    //        ---  Tiny  VSm   Sm    Med   Lg    VLg   Jmbo ---  StHt
    const REL_F_SIZE_TAB: [f32; 10] = [1.0, 1.5, 1.7, 2.0, 2.2, 2.5, 3.0, 3.6, 0.0, 4.0];

    let idx = code as usize;
    if idx >= REL_F_SIZE_TAB.len() || line_space <= 0 {
        return 12.0; // fallback
    }
    let multiplier = REL_F_SIZE_TAB[idx];
    if multiplier == 0.0 {
        return 12.0; // unused slot (code 8)
    }
    // d2pt: (ddist + 8) >> 4 = (ddist + 8) / 16
    let ddist = multiplier * line_space as f32;
    let pt = (ddist + 8.0) / 16.0;
    pt.max(4.0) // MIN_TEXT_SIZE
}

/// Resolve font parameters for a GRAPHIC object.
///
/// If graphic.info == FONT_THISITEMONLY (0), use the graphic's own font fields.
/// Otherwise, look up the text style from the score header.
///
/// line_space: staff line spacing in DDIST = staff_height / (staff_lines - 1).
///
/// Reference: GetGraphicFontInfo (DrawUtils.cp:2481-2506), GetTextSize (Utility.cp:1162)
fn resolve_graphic_font(
    gfx: &crate::obj_types::Graphic,
    score: &InterpretedScore,
    gtype: &crate::obj_types::GraphicType,
    line_space: i16,
) -> TextFont {
    use crate::defs::FONT_THISITEMONLY;

    let style_idx = gfx.info as usize;

    // Try to use text style from header.
    // FONT_* constants are 1-based (FONT_MN=1, FONT_PN=2, etc.) but
    // text_styles[] is 0-indexed, so we use text_styles[style_idx - 1].
    // Reference: defs.h enum (line 6-18), DrawObject.cp SetFontFromTEXTSTYLE()
    if style_idx > FONT_THISITEMONLY as usize && (style_idx - 1) < score.text_styles.len() {
        let ts = &score.text_styles[style_idx - 1];
        let pt_size = if ts.rel_f_size {
            rel_size_to_pt(ts.font_size, line_space)
        } else {
            (ts.font_size as f32).max(4.0)
        };
        let bold = (ts.font_style & 1) != 0;
        let italic = (ts.font_style & 2) != 0;
        let name = if ts.font_name.is_empty() {
            default_font_for_type(gtype)
        } else {
            map_mac_font_name(&ts.font_name)
        };
        TextFont::new(name, pt_size).bold(bold).italic(italic)
    } else {
        // FONT_THISITEMONLY: use the graphic's own font fields.
        // Look up font name from font_names[fontInd] (score header font table).
        // Reference: GetGraphicFontInfo() — doc->fontTable[p->fontInd].fontID
        let pt_size = if gfx.rel_f_size != 0 {
            rel_size_to_pt(gfx.font_size, line_space)
        } else if gfx.font_size > 0 {
            gfx.font_size as f32
        } else {
            12.0
        };
        let bold = (gfx.font_style & 1) != 0;
        let italic = (gfx.font_style & 2) != 0;
        let font_idx = gfx.font_ind as usize;
        let name = if font_idx < score.font_names.len() {
            let table_name = &score.font_names[font_idx];
            if table_name == "Sonata" {
                // Sonata text will be rendered as music glyphs via is_sonata_font(),
                // but we still need a font for metrics. Use Sonata name so the
                // renderer can identify it if needed.
                "Sonata".to_string()
            } else {
                map_mac_font_name(table_name)
            }
        } else {
            default_font_for_type(gtype)
        };
        TextFont::new(name, pt_size).bold(bold).italic(italic)
    }
}

/// Check whether a GRAPHIC object's font is a Sonata-compatible music font.
///
/// When a music font is detected, the text characters are music symbol codes
/// (e.g. '%' = segno, 'U' = fermata, 'q' = quarter note) and must be
/// mapped to SMuFL glyphs rather than rendered as normal text.
///
/// Recognizes Sonata and compatible music fonts (Briard, Petrucci, Opus, etc.)
/// via `is_music_font_name()`.
///
/// For info > FONT_THISITEMONLY: checks text_styles[info-1].font_name.
/// For info == FONT_THISITEMONLY (0): checks font_names[fontInd] from
/// the score header's font table.
///
/// Reference: DrawUtils.cp GetGraphicFontInfo() (line 2481-2506)
fn is_sonata_font(gfx: &crate::obj_types::Graphic, score: &InterpretedScore) -> bool {
    use super::draw_utils::is_music_font_name;
    use crate::defs::FONT_THISITEMONLY;

    let style_idx = gfx.info as usize;
    if style_idx > FONT_THISITEMONLY as usize && (style_idx - 1) < score.text_styles.len() {
        let ts = &score.text_styles[style_idx - 1];
        if is_music_font_name(&ts.font_name) {
            return true;
        }
    } else if gfx.info == FONT_THISITEMONLY as i16 {
        // FONT_THISITEMONLY: use the graphic's own fontInd to look up the font table.
        // Reference: GetGraphicFontInfo() — doc->fontTable[p->fontInd].fontID
        let font_idx = gfx.font_ind as usize;
        if font_idx < score.font_names.len() && is_music_font_name(&score.font_names[font_idx]) {
            return true;
        }
    }
    false
}

/// Map OG Nightingale Mac font names to modern equivalents.
///
/// The PdfRenderer classifies serif vs sans-serif from the name and maps to
/// Times-Roman or Helvetica standard PDF fonts, so the primary goal here is
/// to preserve the serif/sans-serif classification for unknown fonts.
fn map_mac_font_name(name: &str) -> String {
    match name {
        "Times" | "Times New Roman" | "New York" => "Times New Roman".to_string(),
        "Helvetica" | "Arial" | "Geneva" | "Chicago" => "Helvetica".to_string(),
        "Courier" | "Courier New" | "Monaco" => "Courier".to_string(),
        "Palatino" | "Palatino Linotype" => "Palatino".to_string(),
        // Decorative/calligraphic serif fonts — map to Times family so the
        // PDF renderer selects Times-Roman (closer in width than Helvetica).
        "Briard" | "Zapf Chancery" | "Apple Chancery" | "Zapfino" => "Times New Roman".to_string(),
        // Sonata was the OG Nightingale music font. When it appears in text styles
        // for text GRAPHICs (annotations, directions), map to Helvetica (sans-serif)
        // since these are performance directions, not music symbols.
        "Sonata" => "Helvetica".to_string(),
        _ => name.to_string(), // Pass through — PdfRenderer will classify
    }
}

/// Default font for a given graphic type.
fn default_font_for_type(gtype: &crate::obj_types::GraphicType) -> String {
    use crate::obj_types::GraphicType;
    match gtype {
        GraphicType::GrLyric => "Times New Roman".to_string(),
        GraphicType::GrRehearsal => "Helvetica".to_string(),
        GraphicType::GrChordSym => "Helvetica".to_string(),
        _ => "Times New Roman".to_string(),
    }
}

// =============================================================================
// CHORD SYMBOL NORMALIZATION
// =============================================================================

/// Normalize a chord symbol string from OG Nightingale's internal format to
/// display-ready Unicode text.
///
/// OG Nightingale stores chord symbols as 7 fields delimited by 0x7F (ASCII DEL):
///   rootStr | qualStr | extStr | extStk1 | extStk2 | extStk3 | bassStr
///
/// Within root/ext/bass fields, Sonata music font accidental bytes are embedded
/// inline. After Mac Roman conversion, these become:
///   - 'b' after A-G  → flat (keep as 'b' — standard jazz chord notation)
///   - '#' after A-G  → sharp (keep as '#')
///   - 'n' after A-G  → natural → '♮' (U+266E)
///   - 'º' (U+00BA)   → double-flat → 'bb'
///   - 'Ü' (U+00DC)   → double-sharp → '##'
///
/// For extension fields (extStr, extStk), only 'b' and '#' are recognized as
/// accidentals (OG IsCSAcc else-branch). Quality (qualStr) has NO accidental
/// processing.
///
/// Reference: ChordSym.cp ParseChordSym() lines 46-116, DrawChordSym() lines 171-658
fn normalize_chord_symbol(raw: &str) -> String {
    // Split on CS_DELIMITER (0x7F). If there are no delimiters, treat the
    // whole string as a simple chord symbol (old-style, pre-field format).
    let fields: Vec<&str> = raw.split('\x7f').collect();

    if fields.len() < 7 {
        // Old-style chord symbol without delimiters, or malformed.
        // Just strip control chars and return.
        return raw
            .replace(['\x7f', '\0'], "")
            .trim_end_matches(|c: char| c.is_control())
            .to_string();
    }

    let root = fields[0];
    let qual = fields[1];
    let ext = fields[2];
    let ext_stk1 = fields[3];
    let ext_stk2 = fields[4];
    let ext_stk3 = fields[5];
    let bass = fields[6].trim_end_matches(|c: char| c.is_control() || c == '\0');

    let mut out = String::with_capacity(raw.len());

    // Root: replace Sonata accidentals (all 5 types after A-G)
    replace_root_accidentals(root, &mut out);

    // Quality: no accidental processing, just append
    out.push_str(qual);

    // Extension: only 'b' and '#' are accidentals (not context-dependent)
    replace_ext_accidentals(ext, &mut out);

    // Extension stack: concatenate non-empty stacked extensions.
    // OG Nightingale draws these vertically stacked; we render inline in parens.
    let stk_parts: Vec<&str> = [ext_stk1, ext_stk2, ext_stk3]
        .iter()
        .copied()
        .filter(|s| !s.is_empty())
        .collect();
    if !stk_parts.is_empty() {
        out.push('(');
        for (i, part) in stk_parts.iter().enumerate() {
            if i > 0 {
                out.push('/');
            }
            replace_ext_accidentals(part, &mut out);
        }
        out.push(')');
    }

    // Bass: preceded by "/" with same accidental handling as root
    if !bass.is_empty() {
        out.push('/');
        replace_root_accidentals(bass, &mut out);
    }

    out
}

/// Replace Sonata accidental codes in a root or bass field.
///
/// After A-G, recognize all accidentals:
///   'b' → 'b' (flat — standard notation), '#' → '#', 'n' → '♮',
///   'º' (U+00BA) → 'bb' (double-flat), 'Ü' (U+00DC) → '##' (double-sharp)
///
/// Outside that context, only 'b' and '#' are accidentals (kept as-is).
///
/// Reference: ChordSym.cp IsCSAcc() lines 119-132
fn replace_root_accidentals(field: &str, out: &mut String) {
    let chars: Vec<char> = field.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        let prev_is_note = i > 0 && chars[i - 1] >= 'A' && chars[i - 1] <= 'G';
        if prev_is_note {
            // After A-G: all 5 accidental types recognized
            match ch {
                // 'b' and '#' stay as standard text (jazz convention)
                'n' => out.push('♮'),
                '\u{00BA}' => out.push_str("bb"), // Sonata double-flat (was 0xBA)
                '\u{00DC}' => out.push_str("##"), // Sonata double-sharp (was 0xDC)
                other => out.push(other),
            }
        } else {
            // Not after a note letter: only fix the exotic Sonata bytes
            match ch {
                '\u{00BA}' => out.push_str("bb"),
                '\u{00DC}' => out.push_str("##"),
                other => out.push(other),
            }
        }
    }
}

/// Replace Sonata accidental codes in extension fields.
///
/// In extension context, only 'b' and '#' are recognized as accidentals
/// (OG IsCSAcc else-branch). These are already standard text chars, so we
/// only need to fix the exotic Sonata double-flat/sharp bytes.
///
/// Reference: ChordSym.cp IsCSAcc() lines 119-132 (else branch)
fn replace_ext_accidentals(field: &str, out: &mut String) {
    for ch in field.chars() {
        match ch {
            '\u{00BA}' => out.push_str("bb"), // Sonata double-flat
            '\u{00DC}' => out.push_str("##"), // Sonata double-sharp
            other => out.push(other),
        }
    }
}

// =============================================================================
// TEMPO
// =============================================================================

/// Draw a TEMPO object: verbal tempo string and/or metronome mark.
///
/// The metronome mark consists of a duration-unit note glyph (possibly dotted)
/// followed by " = N" where N is the BPM number string.
///
/// Reference: DrawObject.cp, DrawTEMPO(), lines 2275-2461
/// PostScript path: lines 2440-2454
pub fn draw_tempo(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::defs::*;
    use crate::ngl::interpret::ObjData;
    use crate::render::types::ddist_wide_to_render;

    let tempo = match &obj.data {
        ObjData::Tempo(t) => t,
        _ => return,
    };

    // Staff context for positioning
    let staffn = if tempo.ext_header.staffn > 0 {
        tempo.ext_header.staffn
    } else {
        1
    };
    let staff_ctx = match ctx.get(staffn) {
        Some(c) => c,
        None => return,
    };
    if !staff_ctx.visible {
        return;
    }

    // Compute position — same two-case logic as draw_graphic.
    // Page-relative TEMPOs (firstObj is PAGE): absolute page coordinates.
    // Staff-relative TEMPOs: staff_left + SysRelxd(firstObj) + tempo.xd.
    // Reference: DrawUtils.cp:2421-2449 (GetGraphicOrTempoDrawInfo)
    let (xd, yd) = if score.is_page_type(tempo.first_obj_l) {
        (obj.header.xd as i32, obj.header.yd as i32)
    } else {
        let anchor_staff_left = score
            .staff_left_at(tempo.first_obj_l, staffn)
            .unwrap_or(staff_ctx.staff_left);
        let first_obj_sys_xd = score.sys_rel_xd(tempo.first_obj_l);
        let xd = anchor_staff_left as i32 + first_obj_sys_xd + obj.header.xd as i32;
        // Use staff_top rather than measure_top: TEMPO objects appear BEFORE
        // MEASURE objects in the NGL object list, so measure_top may not yet
        // be set (it gets assigned = staff_top when the MEASURE is processed).
        // In OG Nightingale, GetContext at the anchor fills measureTop = staffTop.
        // Reference: DrawUtils.cp:2438-2447, DSUtils.cp:351-435 (PageRelyd)
        let yd = staff_ctx.staff_top as i32 + obj.header.yd as i32;
        (xd, yd)
    };

    let x = ddist_wide_to_render(xd);
    let y = ddist_wide_to_render(yd);

    // Compute lineSpace for relative font size resolution (in DDIST)
    // lineSpace = staff_height / (staff_lines - 1)
    // Reference: defs.h LNSPACE macro
    let line_space = if staff_ctx.staff_lines > 1 {
        staff_ctx.staff_height / (staff_ctx.staff_lines as i16 - 1)
    } else {
        staff_ctx.staff_height
    };

    // Resolve text style from FONT_TM (index 8)
    let font = resolve_tempo_font(score, line_space);

    // Get resolved strings
    let (verbal, metro_num) = match score.tempo_strings.get(&obj.index) {
        Some((v, m)) => (v.as_str(), m.as_str()),
        None => return,
    };

    // Draw the verbal tempo string (e.g., "Allegro")
    // Reference: DrawObject.cp:2443 (PostScript path)
    if !verbal.is_empty() {
        renderer.text_string(x, y, verbal, &font);
    }

    // Decide whether to draw the metronome mark
    // OG: if (p->noMM) doDrawMM = False; else doDrawMM = !p->hideMM;
    // Reference: DrawObject.cp:2386-2387
    let do_draw_mm = !tempo.no_mm && !tempo.hide_mm;

    if do_draw_mm && tempo.sub_type != NO_L_DUR {
        // Compute x position for metronome mark.
        // If there's a verbal string, put the MM to its right with a gap.
        // OG: xdMM = dEnclBox.right + lnSpace + extraHorizGap (when verbal string present)
        // Reference: DrawObject.cp:2354-2362
        let xd_mm = if !verbal.is_empty() {
            // Estimate width of verbal string: ~0.55 * font.size * char_count
            // This is approximate; OG used GetNPtStringBBox for exact width.
            let approx_width = verbal.len() as f32 * font.size * 0.55;
            let gap = line_space as f32 / 16.0; // DDIST to points
            x + approx_width + gap
        } else {
            x
        };
        let yd_mm = y;

        // Draw the duration glyph at 80% size (METROSIZE = 8*size/10)
        // Reference: DrawObject.cp:2445, style.h:17
        let note_glyph = tempo_glyph(tempo.sub_type);
        if let Some(glyph) = note_glyph {
            renderer.music_char(xd_mm, yd_mm, glyph, 80.0);
        }

        // Estimate note glyph width for positioning the dot and "= N" string.
        // OG used CharWidth which is pixel-based; we approximate.
        // Reference: DrawObject.cp:2370-2383
        let note_width_pts = font.size * 0.8 * 0.7; // ~70% of metro font size

        // Draw dot if dotted
        let mut xd_after_note = xd_mm + note_width_pts;
        if tempo.dotted {
            // Augmentation dot: small gap + dot glyph
            // Reference: DrawObject.cp:2446-2449
            // SMuFL: U+E1E7 augmentationDot
            let dot_x = xd_after_note + 1.5; // small gap
            renderer.music_char(dot_x, yd_mm, MusicGlyph::smufl(0xE1E7), 80.0);
            xd_after_note = dot_x + font.size * 0.3;
        } else if tempo.sub_type > QTR_L_DUR {
            // Flagged notes need extra spacing for the flag
            xd_after_note += (line_space as f32 / 16.0) * 0.5;
        }

        // Draw " = N" string
        // OG: sprintf(metroStr," = %s", PToCString(PCopy(p->metroStrOffset)));
        // Reference: DrawObject.cp:2346, 2451-2452
        let equals_str = format!(" = {}", metro_num);
        let mm_font = TextFont::new(font.name.clone(), font.size * 0.8)
            .bold(font.bold)
            .italic(font.italic);
        renderer.text_string(xd_after_note, yd_mm, &equals_str, &mm_font);
    }
}

/// Map a Tempo subType (l_dur code) to a SMuFL "individual note" glyph.
///
/// OG Nightingale used Sonata font characters ('q', 'h', 'e', etc.) which are
/// complete notes with stems. SMuFL equivalents are in the U+E1D0 range.
///
/// Reference: DrawUtils.cp, TempoGlyph(), lines 2383-2411
/// SMuFL: https://w3c.github.io/smufl/latest/tables/individual-notes.html
fn tempo_glyph(sub_type: i8) -> Option<MusicGlyph> {
    use crate::defs::*;
    match sub_type {
        BREVE_L_DUR => Some(MusicGlyph::smufl(0xE0A0)), // noteheadDoubleWhole
        WHOLE_L_DUR => Some(MusicGlyph::smufl(0xE1D2)), // noteWhole
        HALF_L_DUR => Some(MusicGlyph::smufl(0xE1D3)),  // noteHalfUp
        QTR_L_DUR => Some(MusicGlyph::smufl(0xE1D5)),   // noteQuarterUp
        EIGHTH_L_DUR => Some(MusicGlyph::smufl(0xE1D7)), // note8thUp
        SIXTEENTH_L_DUR => Some(MusicGlyph::smufl(0xE1D9)), // note16thUp
        THIRTY2ND_L_DUR => Some(MusicGlyph::smufl(0xE1DB)), // note32ndUp
        SIXTY4TH_L_DUR => Some(MusicGlyph::smufl(0xE1DD)), // note64thUp
        ONE28TH_L_DUR => Some(MusicGlyph::smufl(0xE1DF)), // note128thUp
        _ => None,
    }
}

// =============================================================================
// ENDING (volta brackets)
// =============================================================================

/// Draw an ENDING object: volta bracket with optional label ("1.", "2.", etc.).
///
/// An ending bracket consists of:
/// - Optional left cutoff (vertical line descending from bracket)
/// - Horizontal bracket line
/// - Optional right cutoff (vertical line descending from bracket)
/// - Optional ending number label (e.g., "1.", "2.")
///
/// Reference: DrawObject.cp, DrawENDING(), lines 1384-1487
/// PostScript path: lines 1474-1484
/// Draw repeat end barlines (RPTEND objects).
///
/// Repeat end barlines display dots and barlines at repeat points in the score.
/// This is a stub implementation that dispatches rendering to individual staves.
///
/// Full implementation pending: draw repeat dots at appropriate positions and handle
/// multi-staff barline grouping via ARPTEND.connStaff field.
///
/// Reference: DrawObject.cp, DrawRPTEND(), lines 1330-1381
/// Draw a RPTEND object (repeat end markers: D.C., D.S., Segno, repeat barlines).
///
/// Port of DrawObject.cp DrawRPTEND() (lines 1330-1381).
///
/// RPTEND objects mark repeat instructions (D.C. = Da Capo, D.S. = Dal Segno, etc.)
/// and display repeat barlines (with dots on left, right, or both sides).
///
/// The RptEndType enum distinguishes:
/// - RptDc (1) = D.C. (Da Capo) — jump back to start
/// - RptDs (2) = D.S. (Dal Segno) — jump to segno mark
/// - RptSegno1 (3) = Segno symbol (¶)
/// - RptSegno2 (4) = Alternate segno (often coda symbol)
/// - RptL (5) = Repeat left (dots on left only)
/// - RptR (6) = Repeat right (dots on right only)
/// - RptLr (7) = Repeat both sides (dots on both)
///
/// Note: Text rendering for D.C., D.S., and symbols is not yet implemented.
/// Repeat barlines (RptL/R/Lr) are rendered as repeats in measure-based barlines.
///
/// Reference: DrawObject.cp, DrawRPTEND(), lines 1330-1381
pub fn draw_rptend(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::ngl::interpret::ObjData;

    let rptend = match &obj.data {
        ObjData::RptEnd(r) => r,
        _ => return,
    };

    // Get the ARPTEND subobjects for this RPTEND
    let rptend_list = match score.rptend_subs.get(&obj.header.first_sub_obj) {
        Some(list) => list,
        None => return,
    };

    if rptend_list.is_empty() {
        return;
    }

    let x = ddist_to_render(obj.header.xd);

    // Iterate through each ARPTEND subobject
    for arptend in rptend_list {
        if !arptend.header.visible {
            continue;
        }

        let staffn = arptend.header.staffn;
        let staff_ctx = match ctx.get(staffn) {
            Some(c) => c,
            None => continue,
        };

        if !staff_ctx.visible {
            continue;
        }

        // Determine barline type from RptEnd subType
        // Only RptL, RptR, RptLr actually render a barline
        // RptDc, RptDs, RptSegno1, RptSegno2 are text/symbol only (not yet implemented)
        let bar_type = match rptend.sub_type {
            5 => BarLineType::RepeatLeft,  // RptL
            6 => BarLineType::RepeatRight, // RptR
            7 => BarLineType::RepeatBoth,  // RptLr
            // For RptDc, RptDs, RptSegno variants, skip barline rendering
            // (these would need text/glyph rendering, not yet implemented)
            _ => continue,
        };

        // Draw the barline
        let top_y = ddist_to_render(staff_ctx.staff_top);
        let bottom_y = if arptend.conn_staff > 0 {
            // Extend barline to connected staff below
            if let Some(target_ctx) = ctx.get(arptend.conn_staff) {
                d2r_sum(target_ctx.staff_top, target_ctx.staff_height)
            } else {
                // Target staff not visible — fall back to just this staff
                d2r_sum(staff_ctx.staff_top, staff_ctx.staff_height)
            }
        } else {
            d2r_sum(staff_ctx.staff_top, staff_ctx.staff_height)
        };

        let ls_render = if staff_ctx.staff_lines > 1 {
            ddist_to_render(staff_ctx.staff_height / (staff_ctx.staff_lines as i16 - 1))
        } else {
            ddist_to_render(staff_ctx.staff_height)
        };

        renderer.bar_line(top_y, bottom_y, x, bar_type, ls_render);
    }
}

pub fn draw_ending(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::ngl::interpret::ObjData;
    use crate::render::types::ddist_wide_to_render;

    let ending = match &obj.data {
        ObjData::Ending(e) => e,
        _ => return,
    };

    let staffn = if ending.ext_header.staffn > 0 {
        ending.ext_header.staffn
    } else {
        1
    };
    let staff_ctx = match ctx.get(staffn) {
        Some(c) => c,
        None => return,
    };
    if !staff_ctx.visible {
        return;
    }

    // Compute lineSpace (DDIST)
    let ln_space = if staff_ctx.staff_lines > 1 {
        staff_ctx.staff_height / (staff_ctx.staff_lines as i16 - 1)
    } else {
        staff_ctx.staff_height
    };

    // ENDING_CUTOFFLEN = 2 * lnSpace
    // ENDING_THICK = 6 * lnSpace / 50
    // Reference: style.h lines 121-122
    let rise = 2 * ln_space as i32;
    let end_thick = (6 * ln_space as i32) / 50;
    let end_thick_r = ddist_wide_to_render(end_thick).max(0.5);

    // Compute left end position
    // OG: xd = SysRelxd(firstObjL) + p->xd + sysLeft
    //     yd = measureTop + p->yd
    // Reference: DrawObject.cp:1415-1418, DSUtils.cp:288 (PageRelxd for ENDINGtype)
    let anchor_staff_left = score
        .staff_left_at(ending.first_obj_l, staffn)
        .unwrap_or(staff_ctx.staff_left);
    let first_obj_sys_xd = score.sys_rel_xd(ending.first_obj_l);
    let xd = anchor_staff_left as i32 + first_obj_sys_xd + obj.header.xd as i32;
    let yd = staff_ctx.measure_top as i32 + obj.header.yd as i32;

    // Compute right end position
    // OG: endxd = SysRelxd(lastObjL) + p->endxd + sysLeft
    let last_anchor_staff_left = score
        .staff_left_at(ending.last_obj_l, staffn)
        .unwrap_or(staff_ctx.staff_left);
    let last_obj_sys_xd = score.sys_rel_xd(ending.last_obj_l);
    let endxd = last_anchor_staff_left as i32 + last_obj_sys_xd + ending.endxd as i32;

    let x1 = ddist_wide_to_render(xd);
    let y1 = ddist_wide_to_render(yd);
    let x2 = ddist_wide_to_render(endxd);
    let rise_r = ddist_wide_to_render(rise);

    // Draw left cutoff (vertical line down from bracket)
    // Reference: DrawObject.cp:1476
    if ending.no_l_cutoff == 0 {
        renderer.line(x1, y1 + rise_r, x1, y1, end_thick_r);
    }

    // Draw horizontal bracket line
    // Reference: DrawObject.cp:1477
    renderer.line(x1, y1, x2, y1, end_thick_r);

    // Draw right cutoff (vertical line down from bracket)
    // Reference: DrawObject.cp:1478
    if ending.no_r_cutoff == 0 {
        renderer.line(x2, y1, x2, y1 + rise_r, end_thick_r);
    }

    // Draw ending number label
    // Reference: DrawObject.cp:1479-1483
    if ending.end_num != 0 {
        // Standard ending labels: 1="1.", 2="2.", etc.
        // OG loaded these from a resource; we use a simple mapping.
        let label = ending_label(ending.end_num);

        // Position: xd + lnSpace, yd + 2*lnSpace
        // Reference: DrawObject.cp:1419-1421
        let xd_num = xd + ln_space as i32;
        let yd_num = yd + 2 * ln_space as i32;
        let x_num = ddist_wide_to_render(xd_num);
        let y_num = ddist_wide_to_render(yd_num);

        // Font size = d2pt(2 * lnSpace - 1)
        // Reference: DrawObject.cp:1421, PostScript uses Times
        let font_size = ((2 * ln_space as i32 - 1 + 8) / 16) as f32;
        let font_size = font_size.max(6.0);
        let font = TextFont::new("Times New Roman", font_size);
        renderer.text_string(x_num, y_num, &label, &font);
    }
}

// =============================================================================
// OTTAVA (8va/8vb/15ma lines)
// =============================================================================

/// Draw an OTTAVA object: octave sign number + dashed bracket + optional cutoff.
///
/// An ottava consists of:
/// - The number string ("8" or "15") in Sonata italic digits
/// - A horizontal dashed bracket line
/// - An optional vertical cutoff line at the right end
///
/// For "alta" types (8va, 15ma, 22ma), the bracket is raised 2 half-spaces
/// above the number position. For "bassa" types, the cutoff goes upward.
///
/// Reference: Ottava.cp, DrawOTTAVA(), lines 538-637
/// PostScript path: DrawOctBracket(), lines 1186-1234
pub fn draw_ottava(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::ngl::interpret::ObjData;
    use crate::render::types::ddist_wide_to_render;

    let ottava = match &obj.data {
        ObjData::Ottava(o) => o,
        _ => return,
    };

    let staffn = if ottava.ext_header.staffn > 0 {
        ottava.ext_header.staffn
    } else {
        1
    };
    let staff_ctx = match ctx.get(staffn) {
        Some(c) => c,
        None => return,
    };
    if !staff_ctx.visible {
        return;
    }

    // Compute lineSpace (DDIST)
    let ln_space = if staff_ctx.staff_lines > 1 {
        staff_ctx.staff_height / (staff_ctx.staff_lines as i16 - 1)
    } else {
        staff_ctx.staff_height
    };
    let d_half_sp = ln_space / 2;

    // Get first/last sync positions from ANOTEOTTAVA subobjects
    // Reference: Ottava.cp:568-573
    let first_sync_l = match score.first_in_ottava(obj.header.first_sub_obj, obj.header.n_entries) {
        Some(l) => l,
        None => return,
    };
    let last_sync_l = match score.last_in_ottava(obj.header.first_sub_obj, obj.header.n_entries) {
        Some(l) => l,
        None => return,
    };

    let d_top = staff_ctx.measure_top as i32;

    // Compute first/last x positions using sys_rel_xd for correct cross-measure handling.
    // OG: firstxd = dLeft + LinkXD(firstSyncL), with cross-measure adjustment at line 574
    // Our model: staff_left + sys_rel_xd(sync) gives page-relative x regardless of measure.
    // Reference: Ottava.cp:572-574, DSUtils.cp:568-578
    let firstxd = staff_ctx.staff_left as i32 + score.sys_rel_xd(first_sync_l);
    let lastxd = staff_ctx.staff_left as i32 + score.sys_rel_xd(last_sync_l);

    // Cutoff length
    // OTTAVA_CUTOFFLEN(lnSpace) = lnSpace
    // Reference: style.h line 112
    let y_cutoff_len = if ottava.no_cutoff != 0 {
        0i32
    } else {
        ln_space as i32
    };

    // Get octave type number and bassa flag
    // Reference: Ottava.cp:503-535
    let (number, is_bassa) = get_oct_type_info(ottava.oct_sign_type);
    if number == 0 {
        return; // Unknown type
    }

    // Compute bracket positions
    // OG: octxdFirst = firstxd + p->xdFirst
    //     octydFirst = dTop + p->ydFirst
    //     octxdLast = lastxd + lastNoteWidth + p->xdLast
    // We skip lastNoteWidth for now (minor adjustment).
    // Reference: Ottava.cp:583-588
    let octxd_first = firstxd + ottava.xd_first as i32;
    let octyd_first = d_top + ottava.yd_first as i32;
    let octxd_last = lastxd + ottava.xd_last as i32;
    // octydLast not used in bracket (bracket is horizontal)

    if !ottava.number_vis {
        return;
    }

    // Compute bracket start/end points
    // For alta: raise bracket by 2*dhalfSp
    // Reference: Ottava.cp:601-606
    let first_pt_x = octxd_first;
    let mut first_pt_y = octyd_first;
    let last_pt_x = octxd_last;

    if !is_bassa {
        first_pt_y -= 2 * d_half_sp as i32;
    }

    // OTTAVA_THICK(lnSpace) = 6 * lnSpace / 50
    // Reference: style.h line 111
    let ott_thick = (6 * ln_space as i32) / 50;
    let ott_thick_r = ddist_wide_to_render(ott_thick).max(0.5);

    // Draw the number string using music_char (Sonata italic digits)
    // OG: NumToSonataStr converts number to Sonata italic digits
    //     PS_MusString draws them at octaveNumSize (default 110%)
    // Reference: Ottava.cp:617-619, MusicFont.cp:352-366
    let num_x = ddist_wide_to_render(octxd_first);
    let num_y = ddist_wide_to_render(octyd_first);

    // octaveNumSize: config.octaveNumSize, default 110%
    let oct_num_size: f32 = 110.0;

    // Draw each digit of the number as a Sonata italic glyph
    let num_str = format!("{}", number);
    let mut char_x = num_x;
    let glyph_advance = ddist_wide_to_render(ln_space as i32) * oct_num_size / 100.0 * 0.55;
    for ch in num_str.chars() {
        if let Some(digit) = ch.to_digit(10) {
            let glyph_code = sonata_italic_digit(digit as u8);
            renderer.music_char(char_x, num_y, MusicGlyph::Sonata(glyph_code), oct_num_size);
            char_x += glyph_advance;
        }
    }

    // Estimate the width of the number string in DDIST
    let num_width_ddist = (num_str.len() as i32) * (ln_space as i32 * 55 / 100);
    // XFUDGE = 4 points = 64 DDIST
    let x_fudge = 64i32;

    // Draw bracket if long enough
    // OG: dBrackMinLen = 4 * dhalfSp; if (lastPt.h - firstPt.h > dBrackMinLen) ...
    // Reference: Ottava.cp:607-611
    let d_brack_min_len = 4 * d_half_sp as i32;
    if last_pt_x - first_pt_x > d_brack_min_len {
        // Draw horizontal dashed line from after number to end
        // OG PostScript path: PS_HDashedLine(firstPt.h+p2d(octWidth+XFUDGE), firstPt.v,
        //                                    lastPt.h, OTTAVA_THICK(lnSpace), pt2d(4))
        // Reference: Ottava.cp:1220-1221
        let dash_start_x = ddist_wide_to_render(first_pt_x + num_width_ddist + x_fudge);
        let bracket_y = ddist_wide_to_render(first_pt_y);
        let end_x = ddist_wide_to_render(last_pt_x);
        // dash_len = pt2d(4) = 4 points = 64 DDIST → 4.0 points
        let dash_len: f32 = 4.0;

        if end_x > dash_start_x {
            renderer.hdashed_line(dash_start_x, bracket_y, end_x, ott_thick_r, dash_len);
        }

        // Draw vertical cutoff at right end
        // OG: PS_Line(lastPt.h, firstPt.v, lastPt.h,
        //             firstPt.v + (isBassa ? -yCutoffLen : yCutoffLen),
        //             OTTAVA_THICK(lnSpace))
        // Reference: Ottava.cp:1222-1224
        if y_cutoff_len != 0 {
            let cutoff_y_end = if is_bassa {
                first_pt_y - y_cutoff_len
            } else {
                first_pt_y + y_cutoff_len
            };
            renderer.line(
                end_x,
                bracket_y,
                end_x,
                ddist_wide_to_render(cutoff_y_end),
                ott_thick_r,
            );
        }
    }
}

/// Get the octave type number and bassa flag from an octSignType code.
///
/// Returns (number, is_bassa). Number is 8, 15, or 22.
/// Returns (0, false) for unknown types.
///
/// Reference: Ottava.cp, GetOctTypeNum(), lines 503-535
fn get_oct_type_info(oct_sign_type: u8) -> (i32, bool) {
    match oct_sign_type {
        1 => (8, false),  // OTTAVA8va
        2 => (15, false), // OTTAVA15ma
        3 => (22, false), // OTTAVA22ma
        4 => (8, true),   // OTTAVA8vaBassa
        5 => (15, true),  // OTTAVA15maBassa
        6 => (22, true),  // OTTAVA22maBassa
        _ => (0, false),
    }
}

/// Map a decimal digit (0-9) to the Sonata italic digit glyph code.
///
/// These are the MCH_idigits[] values from vars.h:330.
/// Used by NumToSonataStr() (MusicFont.cp:352-366) for tuplet numbers,
/// ottava numbers, etc.
///
/// Reference: vars.h line 330
fn sonata_italic_digit(digit: u8) -> u8 {
    const MCH_IDIGITS: [u8; 10] = [0xBC, 0xC1, 0xAA, 0xA3, 0xA2, 0xB0, 0xA4, 0xA6, 0xA5, 0xBB];
    if (digit as usize) < MCH_IDIGITS.len() {
        MCH_IDIGITS[digit as usize]
    } else {
        MCH_IDIGITS[0]
    }
}

/// Get the ending label string for an ending number code.
///
/// OG Nightingale loaded these from resources. Standard convention:
/// 1 = "1.", 2 = "2.", 3 = "1. 2.", etc.
///
/// Reference: InitNightingale.cp:133-161
fn ending_label(end_num: u8) -> String {
    match end_num {
        1 => "1.".to_string(),
        2 => "2.".to_string(),
        3 => "1. 2.".to_string(),
        4 => "1. 2. 3.".to_string(),
        n => format!("{}.", n),
    }
}

/// Resolve the tempo mark font from the FONT_TM text style (index 8).
///
/// Reference: DrawObject.cp:2314-2317 (fontSize, fontID, fontStyle from FONT_TM)
fn resolve_tempo_font(score: &InterpretedScore, line_space: i16) -> TextFont {
    use crate::defs::FONT_TM;

    // FONT_TM = 8, but text_styles is 0-indexed so we use index 7.
    let idx = FONT_TM as usize - 1; // 8 - 1 = 7
    if idx < score.text_styles.len() {
        let ts = &score.text_styles[idx];
        let pt_size = if ts.rel_f_size {
            rel_size_to_pt(ts.font_size, line_space)
        } else {
            (ts.font_size as f32).max(4.0)
        };
        let bold = (ts.font_style & 1) != 0;
        let italic = (ts.font_style & 2) != 0;
        let name = if ts.font_name.is_empty() {
            "Times New Roman".to_string()
        } else {
            map_mac_font_name(&ts.font_name)
        };
        TextFont::new(name, pt_size).bold(bold).italic(italic)
    } else {
        // Fallback: bold 12pt Times
        TextFont::new("Times New Roman", 12.0).bold(true)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_chord_symbol_simple() {
        // Simple chord: "C\x7fmaj\x7f7\x7f\x7f\x7f\x7f"
        let raw = "C\x7fmaj\x7f7\x7f\x7f\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "Cmaj7");
    }

    #[test]
    fn test_normalize_chord_symbol_flat_root() {
        // Bb7: "Bb\x7f\x7f7\x7f\x7f\x7f\x7f"
        let raw = "Bb\x7f\x7f7\x7f\x7f\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "Bb7");
    }

    #[test]
    fn test_normalize_chord_symbol_sharp_root() {
        // F#m: "F#\x7fm\x7f\x7f\x7f\x7f\x7f"
        let raw = "F#\x7fm\x7f\x7f\x7f\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "F#m");
    }

    #[test]
    fn test_normalize_chord_symbol_natural() {
        // Cn (C natural): "Cn\x7f\x7f\x7f\x7f\x7f\x7f"
        let raw = "Cn\x7f\x7f\x7f\x7f\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "C\u{266E}");
    }

    #[test]
    fn test_normalize_chord_symbol_double_flat() {
        // Bbb (B double-flat): "B\u{00BA}\x7f\x7f\x7f\x7f\x7f\x7f"
        // 0xBA in Mac Roman → U+00BA (masculine ordinal indicator)
        let raw = "B\u{00BA}\x7f\x7f\x7f\x7f\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "Bbb");
    }

    #[test]
    fn test_normalize_chord_symbol_double_sharp() {
        // F## (F double-sharp): "F\u{00DC}\x7f\x7f\x7f\x7f\x7f\x7f"
        // 0xDC in Mac Roman → U+00DC (U with diaeresis)
        let raw = "F\u{00DC}\x7f\x7f\x7f\x7f\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "F##");
    }

    #[test]
    fn test_normalize_chord_symbol_with_bass() {
        // C7/Bb: "C\x7f\x7f7\x7f\x7f\x7f\x7fBb"
        let raw = "C\x7f\x7f7\x7f\x7f\x7f\x7fBb";
        assert_eq!(normalize_chord_symbol(raw), "C7/Bb");
    }

    #[test]
    fn test_normalize_chord_symbol_extension_stack() {
        // Chord with stacked extensions: root=C, qual=, ext=, stk1=7, stk2=b9, stk3=, bass=
        // 7 fields need 6 delimiters
        let raw = "C\x7f\x7f\x7f7\x7fb9\x7f\x7f";
        assert_eq!(normalize_chord_symbol(raw), "C(7/b9)");
    }

    #[test]
    fn test_normalize_chord_symbol_full() {
        // Full example: Bb7(b9/#11)/Eb
        let raw = "Bb\x7f\x7f7\x7fb9\x7f#11\x7f\x7fEb";
        assert_eq!(normalize_chord_symbol(raw), "Bb7(b9/#11)/Eb");
    }

    #[test]
    fn test_normalize_chord_symbol_old_style() {
        // Old-style without delimiters
        let raw = "Cmaj7";
        assert_eq!(normalize_chord_symbol(raw), "Cmaj7");
    }

    #[test]
    fn test_normalize_chord_symbol_trailing_nul() {
        // Trailing NUL bytes stripped from bass field
        let raw = "C\x7f\x7f7\x7f\x7f\x7f\x7f\x00\x00";
        assert_eq!(normalize_chord_symbol(raw), "C7");
    }

    #[test]
    fn test_draw_rptend_no_crash() {
        // Verify draw_rptend() doesn't panic on empty score.
        // This is a smoke test; actual rendering requires RPTEND objects in fixture.
        use crate::ngl::interpret::ObjData;
        use crate::render::CommandRenderer;

        let score = crate::ngl::interpret::InterpretedScore::default();
        let ctx = crate::context::ContextState::new(1); // 1 staff
        let mut renderer = CommandRenderer::new();

        // Create dummy RPTEND object (no actual data in score)
        let dummy_obj = crate::ngl::interpret::InterpretedObject {
            index: 0,
            header: crate::obj_types::ObjectHeader::default(),
            data: ObjData::RptEnd(crate::obj_types::RptEnd {
                header: crate::obj_types::ObjectHeader::default(),
                first_obj: 0,
                start_rpt: 0,
                end_rpt: 0,
                sub_type: 5, // RptL
                count: 1,
            }),
        };

        // Should not panic (function is no-op with empty score)
        draw_rptend(&score, &dummy_obj, &ctx, &mut renderer);
    }
}
