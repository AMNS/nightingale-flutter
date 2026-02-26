//! Note/rest/grace-note rendering — port of DrawNRGR.cp.
//!
//! Draws Sync objects: noteheads, stems, flags, ledger lines, accidentals,
//! augmentation dots, and slash noteheads.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawNRGR.cp

use crate::context::ContextState;
use crate::defs::*;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore};
use crate::render::types::{ddist_wide_to_render, MusicGlyph};
use crate::render::MusicRenderer;

use super::draw_utils::{
    accidental_glyph, flag_glyph, notehead_glyph, resolve_rest_l_dur, rest_glyph_for_duration,
    REST_Y_OFFSET,
};
use super::helpers::{d2r_sum, d2r_sum3, lnspace_for_staff, TieEndpoint};

/// Compute which ledger lines are needed for a note at a given Y position.
///
/// Returns a Vec of half-line positions where ledger lines should be drawn.
/// Reference: DrawNRGR.cp, DrawLedgerLines() (line 888)
///
/// For a 5-line staff:
/// - Half-line 0 = top staff line
/// - Half-line 8 = bottom staff line
/// - Ledger lines above: -2, -4, -6, etc.
/// - Ledger lines below: 10, 12, 14, etc.
pub fn ledger_lines_for_note(yd: i16, staff_height: i16) -> Vec<i16> {
    let mut lines = Vec::new();

    // Convert yd to half-lines (assuming 5-line staff, staff_height = 8 half-lines)
    // half_ln = yd * 8 / staff_height
    let half_ln = (yd * 8) / staff_height;

    // Above staff: half_ln < 0
    if half_ln < 0 {
        let mut hl = -2;
        while hl >= half_ln {
            lines.push(hl);
            hl -= 2;
        }
    }

    // Below staff: half_ln > 8
    if half_ln > 8 {
        let mut hl = 10;
        while hl <= half_ln {
            lines.push(hl);
            hl += 2;
        }
    }

    lines
}

/// Draw a Sync object (all notes/rests in the synchronization).
///
/// Port of DrawNRGR.cp DrawSYNC() (line 1509) + DrawNote() (line 662).
///
/// For each ANote subobject:
/// - Get context for that note's staff
/// - If visible:
///   - Compute note X from measure_left + obj.header.xd + aNote.xd (all DDIST)
///   - Compute note Y from staff_top + aNote.yd (DDIST, already staff-relative)
///   - Draw notehead/rest glyph (mapped from l_dur)
///   - Draw accidental if present (mapped from accident field)
///   - Draw ledger lines if needed (computed from yd)
///   - If not rest and not whole note (l_dur > 2):
///     - Draw stem from note Y to ystem
///     - If unbeamed eighth/sixteenth: draw flag
///
/// Reference: DrawNRGR.cp, DrawSYNC(), line 1509; DrawNote(), line 662
pub fn draw_sync(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let Some(anote_list) = score.notes.get(&obj.header.first_sub_obj) {
        for anote in anote_list {
            if let Some(note_ctx) = ctx.get(anote.header.staffn) {
                // Check if note is visible
                if note_ctx.visible && anote.header.visible {
                    // "Normal" note X: measure_left + sync xd + note xd
                    // This is used as the accidental anchor point.
                    // Reference: DrawUtils.cp NoteXLoc() line 1456-1479
                    let xd_norm = d2r_sum3(note_ctx.measure_left, obj.header.xd, anote.xd);

                    // Line spacing for ledger lines and notehead sizing
                    let lnspace = lnspace_for_staff(note_ctx.staff_height, note_ctx.staff_lines);
                    let head_width = 1.125 * lnspace; // HeadWidth = 9*lnSp*4/32

                    // Apply otherStemSide offset for seconds in chords.
                    // Reference: DrawUtils.cp NoteXLoc() line 1470-1478
                    let note_x = if anote.other_stem_side && !anote.rest {
                        let stem_down = find_stem_down(anote, anote_list);
                        if stem_down {
                            xd_norm - head_width // downstem: shift LEFT
                        } else {
                            xd_norm + head_width // upstem: shift RIGHT
                        }
                    } else {
                        xd_norm
                    };

                    // Compute note Y: staff_top + note yd (yd is relative to staff top)
                    let note_y = d2r_sum(note_ctx.staff_top, anote.yd);

                    // l_dur is stored in header.sub_type
                    let l_dur = anote.header.sub_type;

                    // Half-space for dot positioning
                    let half_sp = lnspace / 2.0;

                    if !anote.rest {
                        // === NOTES ===
                        draw_note(
                            anote, anote_list, note_ctx, note_x, xd_norm, note_y, l_dur, lnspace,
                            half_sp, renderer,
                        );
                    } else {
                        // === RESTS ===
                        draw_rest(anote, note_x, note_y, l_dur, lnspace, half_sp, renderer);
                    }
                }
            }
        }
    }
}

/// Find the stem direction for a note's voice/staff group.
///
/// For chords, we need to find the "main note" (the one determining stem
/// direction) and check its ystem vs yd.
/// Reference: DrawUtils.cp NoteXLoc() line 1474 — FindMainNote + check ystem>yd
fn find_stem_down(anote: &crate::obj_types::ANote, anote_list: &[crate::obj_types::ANote]) -> bool {
    // Look for a note in the same voice/staff that has ystem != yd (i.e., has a stem)
    for sibling in anote_list {
        if sibling.header.staffn == anote.header.staffn
            && sibling.voice == anote.voice
            && !sibling.rest
            && sibling.ystem != sibling.yd
        {
            return sibling.ystem > sibling.yd;
        }
    }
    // Fallback: use this note's own stem direction
    anote.ystem > anote.yd
}

/// Draw a single note (notehead, accidental, ledger lines, stem, flag, dots).
///
/// Reference: DrawNRGR.cp, DrawNote() (line 662)
///
/// `note_x` is the notehead X position (after otherStemSide offset).
/// `xd_norm` is the "normal" X position (before offset) used for accidental anchor.
#[allow(clippy::too_many_arguments)]
fn draw_note(
    anote: &crate::obj_types::ANote,
    anote_list: &[crate::obj_types::ANote],
    note_ctx: &crate::obj_types::Context,
    note_x: f32,
    xd_norm: f32,
    note_y: f32,
    l_dur: i8,
    lnspace: f32,
    half_sp: f32,
    renderer: &mut dyn MusicRenderer,
) {
    // Draw notehead (use head_shape to select glyph)
    // Reference: DrawNRGR.cp DrawNote() line 722, GetNoteheadInfo()
    let notehead = notehead_glyph(anote.head_shape, l_dur);

    if notehead != 0 {
        renderer.music_char(note_x, note_y, MusicGlyph::smufl(notehead), 100.0);
    } else if anote.head_shape == crate::obj_types::HeadShape::SlashShape as u8 {
        // Slash notehead: drawn as a steep filled parallelogram
        // via line_horizontal_thick (PS_LineHT).
        //
        // Reference: PS_Stdio.cp PS_NoteStem() line 1678-1684
        //   yoff = y + 2*dhalfSp
        //   thick = SLASH_THICK * dhalfSp / 4 = dhalfSp  (style.h:75)
        //   PS_LineHT(xoff, yoff, xoff+2*dhalfSp, yoff-4*dhalfSp, thick)
        //
        // Reference: DrawNRGR.cp DrawNotehead() line 477-499
        //   PenSize(thick, 1); Move(0, 2*dhalfSp); Line(2*dhalfSp, -4*dhalfSp)
        //
        // Geometry: 1 space wide × 2 spaces tall, slope ~63°,
        //           thickness = 1 half-space
        let stem_down = anote.ystem > anote.yd;
        let thick = half_sp; // SLASH_THICK * dhalfSp / 4 = dhalfSp
        let slash_xtweak = 2.0 / 16.0; // SLASH_XTWEAK = 2 DDIST = 0.125 pt

        let xoff = if stem_down {
            note_x - slash_xtweak
        } else {
            note_x - (3.0 * thick) / 4.0 + slash_xtweak
        };

        let yoff = note_y + 2.0 * half_sp; // bottom-left
        renderer.line_horizontal_thick(
            xoff,
            yoff,
            xoff + 2.0 * half_sp,
            yoff - 4.0 * half_sp,
            thick,
        );
    }
    // else: NO_VIS (0) or NOTHING_VIS (10) — intentionally invisible

    // Draw accidental if present.
    // Accidentals anchor from xdNorm (before otherStemSide offset).
    // Reference: DrawNRGR.cp DrawNote() line 798: DrawAcc(..., xdNorm, ...)
    if anote.accident != 0 {
        if let Some(acc_glyph) = accidental_glyph(anote.accident) {
            // If chord is downstemmed with notes to the left of the stem,
            // shift accidental left by one head width.
            // Reference: DrawNRGR.cp line 341-342
            let head_width = 1.125 * lnspace;
            let chord_note_to_l = chord_note_to_left(anote, anote_list);
            let acc_anchor = if chord_note_to_l {
                xd_norm - head_width
            } else {
                xd_norm
            };
            // Position accidental to the left of notehead
            // Typical offset: ~1.5 * lnspace
            let acc_x = acc_anchor - (1.5 * lnspace);
            renderer.music_char(acc_x, note_y, MusicGlyph::smufl(acc_glyph), 100.0);
        }
    }

    // Draw ledger lines if needed
    // Reference: DrawUtils.cp NoteLedgers() (line 1552)
    // Config defaults: ledgerLLen=48, ledgerLOtherLen=12 (in 32nds of lnSpace)
    let ledgers = ledger_lines_for_note(anote.yd, note_ctx.staff_height);
    if !ledgers.is_empty() {
        let stem_down = anote.ystem > anote.yd;
        // OG Ngale: dLLen = ledgerLLen * lnSpace / 32 = 48/32 = 1.5 * lnSpace
        let d_l_len = 1.5 * lnspace;
        // dLOtherLen = 12/32 = 0.375 * lnSpace
        let d_l_other_len = 0.375 * lnspace;
        // HeadWidth = 9*lnSpace*4/32 = 1.125 * lnSpace
        let head_width = 1.125 * lnspace;
        let d_sticks_out = d_l_len - head_width; // 0.375 * lnSpace

        // Left edge of ledger line relative to note_x
        // Note: note_x is the glyph origin (left edge of notehead)
        let ledger_left = if stem_down {
            note_x - d_l_other_len // extend slightly left
        } else {
            note_x - d_sticks_out // extend by sticks_out amount
        };
        let ledger_len = d_l_len + d_l_other_len; // total length: 1.875 * lnSpace
                                                  // Compute center X for our renderer API (which takes center + half_width)
        let ledger_center_x = ledger_left + ledger_len / 2.0;
        let ledger_half_width = ledger_len / 2.0;

        for halfline in ledgers {
            let ledger_y = ddist_wide_to_render(
                note_ctx.staff_top as i32 + (halfline as i32 * note_ctx.staff_height as i32 / 8),
            );
            renderer.ledger_line(ledger_y, ledger_center_x, ledger_half_width);
        }
    }

    // Draw stem if not whole note or breve (l_dur > 2)
    // Skip if ystem == yd (chord notes with hidden stems)
    if l_dur > WHOLE_L_DUR && anote.ystem != anote.yd {
        // Determine stem direction: stem_down = (ystem > yd)
        let stem_down = anote.ystem > anote.yd;

        // HeadWidth (defs.h:355): 9*lnSp*4/32 = 1.125 * lnSpace
        let head_width = 1.125 * lnspace;

        // Stem X: always relative to the NORMAL column (xd_norm), not the
        // displaced notehead position (note_x). This ensures the stem sits
        // between the two note columns when seconds are present.
        // Reference: PS_Stdio.cp PS_NoteStem() line 1694-1731:
        //   stem-down: stem at x (the main note's position)
        //   stem-up: stem at xNorm + headWidth
        // In both cases the stem aligns with the normal column edge.
        let stem_x = if stem_down {
            xd_norm // Stems down: left edge of normal column
        } else {
            xd_norm + head_width // Stems up: right edge of normal column
        };

        // For chords, the stem should span from the near note's yd
        // (closest to ystem) through to ystem. Find the extreme yd
        // on the opposite side of the chord.
        // Reference: FixChordForYStem (Objects.cp:1684-1744)
        let stem_near_yd = if anote.in_chord {
            // Find the note in this chord closest to ystem
            // (i.e. the opposite extreme from the far note)
            let mut near_yd = anote.yd;
            for sibling in anote_list {
                if sibling.header.staffn == anote.header.staffn
                    && sibling.voice == anote.voice
                    && !sibling.rest
                {
                    if stem_down {
                        // Stem down: near note is highest (min yd)
                        if sibling.yd < near_yd {
                            near_yd = sibling.yd;
                        }
                    } else {
                        // Stem up: near note is lowest (max yd)
                        if sibling.yd > near_yd {
                            near_yd = sibling.yd;
                        }
                    }
                }
            }
            near_yd
        } else {
            anote.yd
        };

        // Stem endpoints: from near notehead to ystem
        let stem_top = d2r_sum(note_ctx.staff_top, anote.ystem.min(stem_near_yd));
        let stem_bottom = d2r_sum(note_ctx.staff_top, anote.ystem.max(stem_near_yd));

        // Stem width (default from set_widths)
        let stem_width = 0.8;

        renderer.note_stem(stem_x, stem_top, stem_bottom, stem_width);

        // Draw flag if unbeamed eighth/sixteenth
        // Check if note is beamed: beamed flag is in anote.beamed
        if !anote.beamed {
            if let Some(flag) = flag_glyph(l_dur, !stem_down) {
                // Flag is positioned at stem endpoint (ystem)
                let flag_x = stem_x;
                let flag_y = d2r_sum(note_ctx.staff_top, anote.ystem);
                renderer.music_char(flag_x, flag_y, MusicGlyph::smufl(flag), 100.0);
            }
        }
    }

    // Draw augmentation dots if any
    // Faithful port of DrawAugDots (DrawNRGR.cp:248-307)
    //                  + AugDotXDOffset (DrawUtils.cp:1532-1582)
    if anote.ndots > 0 && anote.y_move_dots != 0 {
        draw_aug_dots_note(anote, note_x, note_y, l_dur, lnspace, half_sp, renderer);
    }
}

/// Draw augmentation dots for a note.
///
/// Port of DrawAugDots (DrawNRGR.cp:248-307) + AugDotXDOffset (DrawUtils.cp:1532-1582)
#[allow(clippy::too_many_arguments)]
fn draw_aug_dots_note(
    anote: &crate::obj_types::ANote,
    note_x: f32,
    note_y: f32,
    l_dur: i8,
    lnspace: f32,
    half_sp: f32,
    renderer: &mut dyn MusicRenderer,
) {
    // --- AugDotXDOffset (from note origin xdNorm) ---
    // xdDots = dhalfSp (base gap)
    let mut xd_offset = half_sp;
    // For non-small notes: += dhalfSp/2
    xd_offset += half_sp / 2.0;
    // WIDEHEAD: whole += dhalfSp/2, breve += dhalfSp
    if l_dur <= WHOLE_L_DUR {
        xd_offset += half_sp / 2.0;
        if l_dur <= 1 {
            // breve: WIDEHEAD=2, gets another dhalfSp/2
            xd_offset += half_sp / 2.0;
        }
    }
    // xMoveDots fine-tune: std2d(STD_LINEHT*(xMoveDots-3)/4, ...)
    // STD_LINEHT=8, so 8*(x-3)/4 = 2*(x-3) STDIST
    // std2d converts STDIST→DDIST: val * staffHt / (4*(staffLines-1))
    // In render coords: 2*(x-3) * lnspace / (4 * 4) = (x-3) * lnspace / 8
    // (since STD_LINEHT=8 maps to 1 lnspace)
    xd_offset += (anote.x_move_dots as f32 - 3.0) * lnspace / 4.0;

    // --- DrawAugDots PS path: xdDots += 2*dhalfSp before each dot ---
    // Y offset: (yMoveDots-2)*dhalfSp from note yd
    let yd_dots = note_y + (anote.y_move_dots as f32 - 2.0) * half_sp;

    let dot_glyph = 0xE1E7_u32; // SMuFL augmentationDot

    let mut dot_x = note_x + xd_offset;
    for _ in 0..anote.ndots {
        dot_x += lnspace; // OG: xdDots += 2*dhalfSp before draw
        renderer.music_char(dot_x, yd_dots, MusicGlyph::smufl(dot_glyph), 100.0);
    }
}

/// Draw a rest glyph with augmentation dots.
///
/// Reference: DrawNRGR.cp, DrawRest() (line 1402)
#[allow(clippy::too_many_arguments)]
fn draw_rest(
    anote: &crate::obj_types::ANote,
    note_x: f32,
    note_y: f32,
    l_dur: i8,
    lnspace: f32,
    half_sp: f32,
    renderer: &mut dyn MusicRenderer,
) {
    // Resolve effective drawing l_dur for rests.
    // Whole-measure rests (l_dur <= -1) are drawn as whole rests.
    // Reference: DrawUtils.cp, GetRestDrawInfo(), line 1303
    let draw_l_dur = resolve_rest_l_dur(l_dur);

    // Apply restYOffset: vertical correction in half-spaces.
    // Reference: DrawUtils.cp, GetRestDrawInfo(), line 1319
    let rest_y_off = if (draw_l_dur as usize) < REST_Y_OFFSET.len() {
        REST_Y_OFFSET[draw_l_dur as usize] as f32 * half_sp
    } else {
        0.0
    };
    // SMuFL glyph origin correction:
    // Sonata: whole rest baseline at bottom of rect (sits on line)
    // SMuFL: whole rest origin at top of rect (hangs from line)
    // NGL yd positions for Sonata baseline; shift up 1 lnSpace for SMuFL.
    // Half rest is the opposite (Sonata baseline at top, SMuFL at bottom).
    let smufl_rest_correction = match draw_l_dur {
        x if x == WHOLE_L_DUR => -lnspace, // shift up to hang from correct line
        _ => 0.0,
    };
    let rest_y = note_y + rest_y_off + smufl_rest_correction;

    // Draw rest glyph
    let rest_glyph = rest_glyph_for_duration(draw_l_dur);
    renderer.music_char(note_x, rest_y, MusicGlyph::smufl(rest_glyph), 100.0);

    // Draw augmentation dots on rests
    // Port of DrawAugDots (DrawNRGR.cp:1388/1458) for rests
    // AugDotXDOffset for rests: xdDots = dhalfSp, + dhalfSp if IS_WIDEREST
    if anote.ndots > 0 && anote.y_move_dots != 0 {
        let mut xd_offset = half_sp;
        // IS_WIDEREST: whole/half rests are wider
        if draw_l_dur <= 3 {
            xd_offset += half_sp;
        }
        // xMoveDots fine-tune (same formula as notes)
        xd_offset += (anote.x_move_dots as f32 - 3.0) * lnspace / 4.0;

        let yd_dots = note_y + (anote.y_move_dots as f32 - 2.0) * half_sp;
        let dot_glyph = 0xE1E7_u32;

        let mut dot_x = note_x + xd_offset;
        for _ in 0..anote.ndots {
            dot_x += lnspace; // OG: += 2*dhalfSp before each draw
            renderer.music_char(dot_x, yd_dots, MusicGlyph::smufl(dot_glyph), 100.0);
        }
    }
}

/// Collect tie endpoint information from a Sync object.
///
/// For each note with tied_r, record a TieEndpoint in `tie_starts`.
/// For each note with tied_l, record a TieEndpoint in `tie_ends`.
/// These are matched and drawn later by `draw_ties()`.
pub fn collect_tie_endpoints(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    tie_starts: &mut Vec<TieEndpoint>,
    tie_ends: &mut Vec<TieEndpoint>,
) {
    if let Some(anote_list) = score.notes.get(&obj.header.first_sub_obj) {
        for anote in anote_list {
            if anote.rest || (!anote.tied_l && !anote.tied_r) {
                continue;
            }
            if let Some(note_ctx) = ctx.get(anote.header.staffn) {
                if !note_ctx.visible || !anote.header.visible {
                    continue;
                }
                let note_x = d2r_sum3(note_ctx.measure_left, obj.header.xd, anote.xd);
                let note_y = d2r_sum(note_ctx.staff_top, anote.yd);

                let lnspace = lnspace_for_staff(note_ctx.staff_height, note_ctx.staff_lines);
                let head_width = 1.125 * lnspace; // HeadWidth (defs.h:355)

                // Determine stem direction for tie curvature direction
                let stem_down = anote.ystem > anote.yd;

                let ep = TieEndpoint {
                    x: note_x,
                    y: note_y,
                    head_width,
                    stem_down,
                    staff: anote.header.staffn,
                    voice: anote.voice,
                    note_num: anote.note_num,
                    lnspace,
                    staff_right: crate::render::types::ddist_to_render(note_ctx.staff_right),
                    staff_left: crate::render::types::ddist_to_render(note_ctx.staff_left),
                };

                if anote.tied_r {
                    tie_starts.push(ep.clone());
                }
                if anote.tied_l {
                    tie_ends.push(ep);
                }
            }
        }
    }
}

/// Check if a chord is stem-down and has at least one note to the left of the stem.
///
/// Port of Objects.cp ChordNoteToLeft() (line 1432-1450).
/// Used to adjust accidental X positions for chords with seconds.
///
/// "Left of stem" for downstem means otherStemSide==true (the displaced side).
fn chord_note_to_left(
    anote: &crate::obj_types::ANote,
    anote_list: &[crate::obj_types::ANote],
) -> bool {
    // Only relevant for downstem chords
    let stem_down = find_stem_down(anote, anote_list);
    if !stem_down {
        return false;
    }

    // Check if any note in same voice/staff has otherStemSide set
    // Reference: DSUtils.cp IsNoteLeftOfStem() line 2193-2202:
    //   (stemDown == aNote->otherStemSide) => note is on left
    for sibling in anote_list {
        if sibling.header.staffn == anote.header.staffn
            && sibling.voice == anote.voice
            && !sibling.rest
            && sibling.other_stem_side
        {
            return true; // downstem + otherStemSide => note on left
        }
    }
    false
}
