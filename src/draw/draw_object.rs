//! Object-level drawing — port of DrawObject.cp.
//!
//! Type-specific drawing functions for Staff, Measure, Connect, Clef,
//! KeySig, TimeSig, and Ties.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawObject.cp

use crate::context::ContextState;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore};
use crate::render::types::{ddist_to_render, BarLineType, MusicGlyph, Point};
use crate::render::MusicRenderer;

use super::draw_utils::{clef_glyph, clef_halfline_position, get_ks_y_offset};
use super::helpers::{d2r_sum, d2r_sum3, lnspace_for_staff, TieEndpoint};

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

/// Draw a Measure object (bar lines for all staves).
///
/// Port of DrawObject.cp DrawMEASURE() (line 2772) + DrawBarline().
///
/// For each AMeasure subobject:
/// - Get context for that staff
/// - If visible:
///   - Compute bar line X from measure_left (already absolute)
///   - Compute top/bottom Y from staff_top and staff_height
///   - Map AMeasure.header.sub_type to BarLineType enum
///   - Call renderer.bar_line()
///
/// Reference: DrawObject.cp, DrawMEASURE(), line 2772
pub fn draw_measure(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let Some(ameasure_list) = score.measures.get(&obj.header.first_sub_obj) {
        for ameasure in ameasure_list {
            if let Some(measure_ctx) = ctx.get(ameasure.header.staffn) {
                // Check if measure is visible (both staff context and sub-object)
                if measure_ctx.visible && ameasure.header.visible {
                    // Bar line X is at the measure left (already absolute from context)
                    let x = ddist_to_render(measure_ctx.measure_left);

                    // Top and bottom from staff top + staff height
                    let top_y = ddist_to_render(measure_ctx.staff_top);
                    let bottom_y = d2r_sum(measure_ctx.staff_top, measure_ctx.staff_height);

                    // Map subtype to BarLineType
                    let bar_type = map_barline_type(ameasure.header.sub_type);

                    renderer.bar_line(top_y, bottom_y, x, bar_type);
                }
            }
        }
    }
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
    if let Some(aconnect_list) = score.connects.get(&obj.header.first_sub_obj) {
        for aconnect in aconnect_list {
            // Get contexts for top and bottom staves
            if let (Some(top_ctx), Some(bottom_ctx)) =
                (ctx.get(aconnect.staff_above), ctx.get(aconnect.staff_below))
            {
                // Check visibility: connLevel!=0 means this connector should draw,
                // and both staves must be visible.
                // Reference: DrawObject.cp DrawCONNECT() line 686-692
                if aconnect.conn_level != 0 && top_ctx.visible && bottom_ctx.visible {
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
                    let clef_y =
                        ddist_to_render(clef_ctx.staff_top) + (halfline as f32 * lnspace / 2.0);

                    // Mid-measure clefs drawn at 75% (OG: SMALLSIZE macro, style.h:16)
                    let size_pct = if aclef.small != 0 { 75.0 } else { 100.0 };
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

    if let Some(akeysig_list) = score.keysigs.get(&obj.header.first_sub_obj) {
        for akeysig in akeysig_list {
            if let Some(ks_ctx) = ctx.get(akeysig.header.staffn) {
                if !ks_ctx.visible || !akeysig.header.visible {
                    continue;
                }
                let n_items = akeysig.ks_info.n_ks_items;
                if n_items <= 0 {
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

                for k in 0..n_items.min(7) as usize {
                    let ks_item = &akeysig.ks_info.ks_item[k];
                    let is_sharp = ks_item.sharp != 0;
                    let halfln = get_ks_y_offset(ks_ctx.clef_type, ks_item.letcode, is_sharp);

                    let x = base_x + k as f32 * acc_width;
                    let y = staff_top_y + (halfln as f32 * lnspace / 2.0);

                    let glyph = if is_sharp { SMUFL_SHARP } else { SMUFL_FLAT };
                    renderer.music_char(x, y, MusicGlyph::smufl(glyph), 100.0);
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

                    // Y positions: numerator at half-line 2, denominator at half-line 6
                    let num_y = ddist_to_render(timesig_ctx.staff_top) + (2.0 * lnspace / 2.0);
                    let denom_y = ddist_to_render(timesig_ctx.staff_top) + (6.0 * lnspace / 2.0);

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
