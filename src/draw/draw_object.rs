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

/// Draw a Slur object (slurs from NGL file with pre-computed spline data).
///
/// Port of DrawObject.cp DrawSLUR() (line 3054) + Slurs.cp GetSlurPoints() (line 76).
///
/// For NGL files, the ASlur subobjects contain pre-computed spline data:
/// - startPt/endPt: absolute paper-relative base positions (Points = 1/72 inch)
/// - seg.knot: offset from startPt to start knot (DDIST = 1/16 point)
/// - endKnot: offset from endPt to end knot (DDIST)
/// - seg.c0: offset from start knot to first control point (DDIST)
/// - seg.c1: offset from end knot to second control point (DDIST)
///
/// Coordinate conversion (GetSlurPoints, Slurs.cp:76-92):
///   knot     = p2d(startPt) + seg.knot         (absolute DDIST)
///   endKnot  = p2d(endPt)   + endKnot          (absolute DDIST)
///   c0       = knot + seg.c0                    (absolute DDIST)
///   c1       = endKnot + seg.c1                 (absolute DDIST)
///   render_x = ddist / 16.0 = pt.h + seg.knot.h/16.0
///
/// Ties (slur.tie == true) are skipped here — they're handled by draw_ties()
/// using note-level tied_l/tied_r flags with recomputed Bezier curves.
///
/// Reference: DrawObject.cp:3054, Slurs.cp:76-92, PS_Stdio.cp:1933
pub fn draw_slur(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    _ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let crate::ngl::interpret::ObjData::Slur(slur) = &obj.data {
        // Skip ties — those are handled by draw_ties() via note-level flags
        if slur.tie {
            return;
        }

        let n_entries = obj.header.n_entries;
        let aslur_list = score.get_slur_subs(obj.header.first_sub_obj, n_entries);

        for aslur in &aslur_list {
            if !aslur.visible {
                continue;
            }

            // Skip degenerate slurs where both endpoints are at origin
            if aslur.start_pt.h == 0
                && aslur.start_pt.v == 0
                && aslur.end_pt.h == 0
                && aslur.end_pt.v == 0
            {
                continue;
            }

            // GetSlurPoints algorithm (Slurs.cp:76-92):
            // startPt/endPt are in Points (screen coords, 72dpi).
            // seg.knot, endKnot, seg.c0, seg.c1 are in DDIST (1/16 point).
            // p2d(pt) = pt * 16 converts Points→DDIST.
            // In render coords (points): render = Points + DDIST/16.0

            let start_x = aslur.start_pt.h as f32 + aslur.seg.knot.h as f32 / 16.0;
            let start_y = aslur.start_pt.v as f32 + aslur.seg.knot.v as f32 / 16.0;

            let end_x = aslur.end_pt.h as f32 + aslur.end_knot.h as f32 / 16.0;
            let end_y = aslur.end_pt.v as f32 + aslur.end_knot.v as f32 / 16.0;

            let c0_x = start_x + aslur.seg.c0.h as f32 / 16.0;
            let c0_y = start_y + aslur.seg.c0.v as f32 / 16.0;

            let c1_x = end_x + aslur.seg.c1.h as f32 / 16.0;
            let c1_y = end_y + aslur.seg.c1.v as f32 / 16.0;

            let p0 = Point {
                x: start_x,
                y: start_y,
            };
            let c1_pt = Point { x: c0_x, y: c0_y };
            let c2_pt = Point { x: c1_x, y: c1_y };
            let p3 = Point { x: end_x, y: end_y };

            renderer.slur(p0, c1_pt, c2_pt, p3, aslur.dashed);
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
        }
        // else: unmatched slur start — cross-system slur (TODO)
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

        // Compute X position from firstSyncL
        // OG: xd = SysRelxd(DynamFIRSTSYNC(pL)) + LinkXD(pL) + aDynamic->xd + systemLeft
        // In our pipeline: sync positions are relative to system via measure_left + sync.xd
        let first_sync_xd = match score.get(dyn_obj.first_sync_l) {
            Some(sync_obj) => sync_obj.header.xd,
            None => continue,
        };

        // xd = measure_left + firstSync.xd + obj.xd + aDynamic.xd
        let xd = staff_ctx.measure_left as i32
            + first_sync_xd as i32
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
            let last_sync_xd = match score.get(dyn_obj.last_sync_l) {
                Some(sync_obj) => {
                    // Check if lastSyncL is a System object (cross-system hairpin)
                    if matches!(&sync_obj.data, ObjData::System(_)) {
                        // Cross-system: endxd relative to system left
                        0i16
                    } else {
                        sync_obj.header.xd
                    }
                }
                None => continue,
            };

            let endxd = staff_ctx.measure_left as i32 + last_sync_xd as i32 + adynamic.endxd as i32;
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
