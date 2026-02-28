//! Tuplet drawing — port of Tuplet.cp.
//!
//! Draws Tuplet objects: bracket + ratio number.
//!
//! Reference: Nightingale/src/CFilesBoth/Tuplet.cp

use crate::context::ContextState;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::render::types::{ddist_to_render, MusicGlyph};
use crate::render::MusicRenderer;

use super::helpers::{d2r_sum3, lnspace_for_staff};

/// Draw a Tuplet object — bracket + number.
///
/// Port of DrawTUPLET (Tuplet.cp:1587-1723) and DrawPSTupletBracket (Tuplet.cp:1368-1411).
///
/// Rendering algorithm:
/// 1. Find first and last syncs via ANoteTuple subobjects
/// 2. Compute bracket endpoints from sync X positions + ydFirst/ydLast
/// 3. Draw number (centered on bracket midpoint) using SMuFL timeSig digits
/// 4. Draw bracket: left cutoff, left segment, gap for number, right segment, right cutoff
pub fn draw_tuplet(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    let tup = match &obj.data {
        ObjData::Tuplet(t) => t,
        _ => return,
    };

    // Get the ANoteTuple subobjects to find first/last sync
    let anottuples = match score.tuplets.get(&obj.header.first_sub_obj) {
        Some(subs) if subs.len() >= 2 => subs,
        _ => return,
    };

    let first_sync_link = anottuples[0].tp_sync;
    let last_sync_link = anottuples[anottuples.len() - 1].tp_sync;

    // Find the first and last sync objects
    let first_sync = match score.objects.iter().find(|o| o.index == first_sync_link) {
        Some(obj) => obj,
        None => return,
    };
    let last_sync = match score.objects.iter().find(|o| o.index == last_sync_link) {
        Some(obj) => obj,
        None => return,
    };

    // Get context for this tuplet's staff
    let staff_ctx = match ctx.get(tup.ext_header.staffn) {
        Some(c) => c,
        None => return,
    };

    let lnspace = lnspace_for_staff(staff_ctx.staff_height, staff_ctx.staff_lines);

    // Compute bracket X positions
    // firstxd = sync.xd + tup.xd_first (relative to containing measure)
    // We need absolute X, which is measure_left + sync.xd
    // The context tracks measure_left based on the most recently traversed measure.
    // Since tuplets are inserted after their last sync, the context should be correct.
    //
    // Port of GetTupletInfo (Tuplet.cp:1508-1515):
    // firstxd = LinkXD(firstSyncL) + tup->xdFirst
    // lastxd = LinkXD(lastSyncL) + lastNoteWidth + tup->xdLast

    // Find the measure_left for the first sync by looking at context
    // For simplicity, use the current staff_left + measure positions
    let first_x = d2r_sum3(staff_ctx.measure_left, first_sync.header.xd, tup.xd_first);
    let last_note_width = lnspace * 1.125; // HeadWidth = 9*lnSp/8
    let last_x =
        d2r_sum3(staff_ctx.measure_left, last_sync.header.xd, tup.xd_last) + last_note_width;

    // Y positions (relative to staff top, in DDIST → render coords)
    let staff_top_y = ddist_to_render(staff_ctx.staff_top);
    let first_y = staff_top_y + ddist_to_render(tup.yd_first);
    let last_y = staff_top_y + ddist_to_render(tup.yd_last);

    // Bracket thickness — port of TUPLE_BRACKTHICK (style.h:116):
    // (6*lnSpace)/50
    let brack_thick = (6.0 * lnspace) / 50.0;
    let brack_thick = brack_thick.max(0.3); // Minimum visible thickness

    // Cutoff length — port of TUPLE_CUTOFFLEN (style.h:117):
    // STD_LINEHT/2 = 4 STDIST → convert to render coords
    // std2d(4, staffHeight, 5) = 4 * staffHeight / 32
    let cutoff_len = lnspace * 0.5;

    // Bracket delta — distance from note extreme to bracket line
    // BRACKETUP = STD_LINEHT/2 = 4 STDIST
    let brack_delta = lnspace * 0.5;

    // Determine bracket orientation (above or below notes).
    // Port of SetTupletYPos (Tuplet.cp): bracketBelow = (firstyd > staffHeight/2)
    // yd_first is a DDIST offset from staff top; if it exceeds half the staff
    // height, the bracket sits below (cutoffs point down).
    let bracket_below = tup.yd_first > staff_ctx.staff_height / 2;
    let first_bracket_y = if bracket_below {
        first_y + brack_delta
    } else {
        first_y - brack_delta
    };
    let last_bracket_y = if bracket_below {
        last_y + brack_delta
    } else {
        last_y - brack_delta
    };

    // Midpoint for number placement
    let mid_x = (first_x + last_x) / 2.0;
    let mid_y = (first_bracket_y + last_bracket_y) / 2.0;

    // Build the number string
    let num_str = tup.acc_num.to_string();

    // Estimate number width (in render coords): each digit ≈ 0.6 * lnspace
    let num_char_width = lnspace * 0.6;
    let num_width = num_str.len() as f32 * num_char_width;

    // NUMMARGIN = 3 pixels ≈ 3/72 * 16 points ≈ 2/3 render unit
    // More practically: about 0.3 * lnspace
    let num_margin = lnspace * 0.3;

    // Gap edges for the bracket
    let gap_left = mid_x - num_width / 2.0 - num_margin;
    let gap_right = mid_x + num_width / 2.0 + num_margin;

    // Draw the number (if visible)
    if tup.num_vis != 0 {
        // SMuFL defines dedicated tuplet digit glyphs at U+E880–U+E889
        // (tuplet0 through tuplet9), specifically designed for rhythmic grouping
        // notation. These are smaller and more appropriately styled than the
        // timeSig digits at U+E080.
        //
        // Use 100% size since these glyphs are already sized for tuplet context.
        let tuplet_font_pct = 100.0;

        let mut x_offset = -num_width / 2.0;
        for digit_char in num_str.chars() {
            if let Some(digit) = digit_char.to_digit(10) {
                let glyph = 0xE880 + digit; // SMuFL tuplet0-9
                renderer.music_char(
                    mid_x + x_offset,
                    mid_y,
                    MusicGlyph::smufl(glyph),
                    tuplet_font_pct,
                );
                x_offset += num_char_width;
            }
        }
    }

    // Draw the bracket (if visible)
    if tup.brack_vis != 0 {
        // Cutoff direction: points away from the staff center
        let first_cutoff_up = !bracket_below;
        let last_cutoff_up = !bracket_below;

        // Helper: interpolate Y along the bracket line at a given X
        let interp_y = |x: f32| -> f32 {
            if (last_x - first_x).abs() < 0.001 {
                first_bracket_y
            } else {
                let t = (x - first_x) / (last_x - first_x);
                first_bracket_y + t * (last_bracket_y - first_bracket_y)
            }
        };

        // 1. Left cutoff (vertical line at start)
        let cutoff_sign = if first_cutoff_up { -1.0 } else { 1.0 };
        renderer.line(
            first_x,
            first_bracket_y + cutoff_sign * cutoff_len,
            first_x,
            first_bracket_y,
            brack_thick,
        );

        if tup.num_vis != 0 && gap_left > first_x + 1.0 {
            // 2. Left bracket segment (from start to gap)
            let gap_left_y = interp_y(gap_left);
            renderer.line(first_x, first_bracket_y, gap_left, gap_left_y, brack_thick);

            // 3. Right bracket segment (from gap end to endpoint)
            let gap_right_y = interp_y(gap_right);
            renderer.line(gap_right, gap_right_y, last_x, last_bracket_y, brack_thick);
        } else {
            // No number or bracket too short: draw continuous bracket
            renderer.line(
                first_x,
                first_bracket_y,
                last_x,
                last_bracket_y,
                brack_thick,
            );
        }

        // 4. Right cutoff (vertical line at end)
        let cutoff_sign = if last_cutoff_up { -1.0 } else { 1.0 };
        renderer.line(
            last_x,
            last_bracket_y,
            last_x,
            last_bracket_y + cutoff_sign * cutoff_len,
            brack_thick,
        );
    }
}
