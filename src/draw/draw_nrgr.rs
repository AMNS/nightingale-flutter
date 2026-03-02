//! Note/rest/grace-note rendering — port of DrawNRGR.cp.
//!
//! Draws Sync objects: noteheads, stems, flags, ledger lines, accidentals,
//! augmentation dots, and slash noteheads.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawNRGR.cp

use crate::context::ContextState;
use crate::defs::*;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore};
use crate::obj_types::{ANote, Context, XSTD_OFFSET};
use crate::render::types::{ddist_to_render, ddist_wide_to_render, MusicGlyph};
use crate::render::MusicRenderer;
use crate::utility::{acc_x_offset, std2d};

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
                        draw_rest(
                            anote,
                            note_x,
                            note_y,
                            l_dur,
                            lnspace,
                            half_sp,
                            note_ctx.staff_height,
                            renderer,
                        );
                    }

                    // === MODIFIERS (articulations, ornaments, etc.) ===
                    // Port of DrawModNR() call from DrawNote()/DrawRest()
                    // Source: DrawNRGR.cp line 780 (DrawNote), line 1458 (DrawRest)
                    draw_modnrs(score, anote, note_ctx, note_x, note_y, lnspace, renderer);
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
    // Reference: DrawNRGR.cp DrawAcc() lines 314-424
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
            // Use xmove_acc for accidental horizontal offset.
            // Double flat is wider — push it 2 units further left.
            // Reference: DrawNRGR.cp DrawAcc() lines 333-335
            let xmove = if anote.accident == AC_DBLFLAT {
                (anote.xmove_acc as i16 + 2).min(31)
            } else {
                anote.xmove_acc as i16
            };
            // acc_x_offset returns a DDIST offset; convert to render coords.
            // Reference: DrawNRGR.cp line 336: AccXDOffset(xmoveAcc, pContext)
            let offset_ddist =
                acc_x_offset(xmove, note_ctx.staff_height, note_ctx.staff_lines as i16);
            let acc_x = acc_anchor - ddist_to_render(offset_ddist);
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

        // Stem endpoints: from near notehead to ystem.
        // For beamed notes, extend the stem slightly past ystem into the beam
        // to ensure overlap (no visible gap). The OG code adds 8 DDIST (0.5pt)
        // for beamed up-stems (PS_Stdio.cp:1729) and uses stemFudge at the
        // notehead end. We add a 0.5pt extension at the beam end.
        // Reference: PS_Stdio.cp, PS_NoteStem(), line 1725-1731
        let beam_extend = if anote.beamed { 0.5_f32 } else { 0.0 };
        let stem_top = if stem_down {
            // Stem down: ystem is below (larger y), yd is above.
            // Top = notehead, bottom = ystem + extension
            d2r_sum(note_ctx.staff_top, anote.ystem.min(stem_near_yd))
        } else {
            // Stem up: ystem is above (smaller y), extend further up (subtract)
            d2r_sum(note_ctx.staff_top, anote.ystem.min(stem_near_yd)) - beam_extend
        };
        let stem_bottom = if stem_down {
            // Stem down: extend further down (add)
            d2r_sum(note_ctx.staff_top, anote.ystem.max(stem_near_yd)) + beam_extend
        } else {
            d2r_sum(note_ctx.staff_top, anote.ystem.max(stem_near_yd))
        };

        // Stem width: 8% of staff interline space.
        // Reference: PS_Stdio.cp:952, STEMLW_DFLT = 8 (% of lnSpace)
        let stem_width = lnspace * 0.08;

        renderer.note_stem(stem_x, stem_top, stem_bottom, stem_width);

        // Draw flag if unbeamed eighth/sixteenth
        // Check if note is beamed: beamed flag is in anote.beamed
        if !anote.beamed {
            if let Some(flag) = flag_glyph(l_dur, !stem_down) {
                // Sonata flag correction:
                // X: Sonata flags are drawn at xd (notehead X), not stem X.
                //    The Sonata glyph includes internal horizontal positioning.
                //    SMuFL flags go at the stem X (already adjusted for stem side).
                // Y: Sonata flag glyphs assume drawing from notehead position with
                //    octave-length stem baked in. OG shifts from stem tip toward
                //    notehead by 7*halfSp. SMuFL flags go at the stem tip.
                // Reference: DrawNRGR.cp lines 1058-1073.
                let (flag_x, flag_y) = if renderer.uses_sonata_font() {
                    let octave_len = 7.0 * half_sp;
                    let fy = if stem_down {
                        d2r_sum(note_ctx.staff_top, anote.ystem) - octave_len
                    } else {
                        d2r_sum(note_ctx.staff_top, anote.ystem) + octave_len
                    };
                    (xd_norm, fy) // Sonata: notehead X, adjusted Y
                } else {
                    (stem_x, d2r_sum(note_ctx.staff_top, anote.ystem)) // SMuFL: stem X, stem tip Y
                };
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

/// Draw a rest glyph with augmentation dots and pseudo-ledger lines.
///
/// Reference: DrawNRGR.cp, DrawRest() (line 1273-1470)
#[allow(clippy::too_many_arguments)]
fn draw_rest(
    anote: &crate::obj_types::ANote,
    note_x: f32,
    note_y: f32,
    l_dur: i8,
    lnspace: f32,
    half_sp: f32,
    staff_height: i16,
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
    // SMuFL/Bravura glyph origin correction for whole rests:
    //
    // Sonata whole rest (0xB7) has NO outline in the TTF conversion — the glyph
    // is always rendered by Bravura fallback, even when Sonata font is loaded.
    // Sonata convention: yd places the baseline at the bottom of the rest block.
    // Bravura convention: restWhole origin is near the top (yMax≈0, block hangs
    // down to yMin≈-135). Must shift up by 1 lnSpace to compensate.
    //
    // Half rests need no correction: both Sonata and Bravura have the origin
    // at the bottom of the block (yMin≈0, extends upward).
    //
    // Reference: Sonata.ttf glyph analysis — 0xB7 has advance=150 but no bbox.
    let smufl_rest_correction = match draw_l_dur {
        x if x == WHOLE_L_DUR => -lnspace,
        _ => 0.0,
    };
    let rest_y = note_y + rest_y_off + smufl_rest_correction;

    // Draw rest glyph
    let rest_glyph = rest_glyph_for_duration(draw_l_dur);
    renderer.music_char(note_x, rest_y, MusicGlyph::smufl(rest_glyph), 100.0);

    // Draw pseudo-ledger line for whole/half rests outside the staff.
    // OG Nightingale draws a short horizontal line when the rest sits on or
    // hangs from a position beyond the staff boundaries, so the performer
    // can tell which line it's referencing.
    // Reference: DrawNRGR.cp, DrawRest() lines 1329-1342
    if draw_l_dur == HALF_L_DUR || draw_l_dur == WHOLE_L_DUR {
        // yrest = rest yd relative to staff top (DDIST space)
        // For half rest: check the yd directly (rest sits ON the line at yd)
        // For whole rest: check yd - lnSpace (rest hangs BELOW the line)
        let yrest_ddist = if draw_l_dur == HALF_L_DUR {
            anote.yd as i32
        } else {
            // Whole rest: the line it hangs from is one lnSpace above the yd
            anote.yd as i32 - (staff_height as i32 / 4) // lnSpace = staffHeight/4 for 5-line staff
        };

        if yrest_ddist < 0 || yrest_ddist > staff_height as i32 {
            // LedgerOtherLen(lnSpace) - (lnSpace/12): slightly shorter than a note ledger
            // Reference: DrawNRGR.cp line 1337 — xledg = LedgerOtherLen(lnSpace) - (lnSpace/12)
            let ledg_half_width = lnspace * 0.65;
            // The ledger line Y is at the line the rest references.
            // Half rest: sits on note_y (the rest baseline)
            // Whole rest: hangs from note_y + lnSpace (below the line = note_y)
            let ledg_y = if draw_l_dur == HALF_L_DUR {
                note_y + rest_y_off
            } else {
                // Whole rest: line is at rest_y + lnSpace (above the hanging block)
                // Actually rest_y already has the SMuFL correction applied.
                // The ledger line goes through the TOP of the whole rest block.
                rest_y
            };
            // x_center: center the ledger line on the rest glyph
            // The rest glyph is roughly 1 lnSpace wide, so center at note_x + lnspace/2
            let ledg_x_center = note_x + lnspace / 2.0;
            renderer.ledger_line(ledg_y, ledg_x_center, ledg_half_width);
        }
    }

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

/// Draw a GRSync object (grace notes).
///
/// Grace notes are structurally identical to regular notes (same ANote struct)
/// but rendered at 70% size with an optional diagonal slash across the stem.
///
/// Port of DrawNRGR.cp DrawGRSYNC() (line 2221) + DrawGRNote() (line 1695).
///
/// GRACESIZE = 7*size/10 (style.h:15)
pub fn draw_grsync(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    // Grace notes are stored in score.grnotes (keyed by first_sub_obj)
    if let Some(grnote_list) = score.grnotes.get(&obj.header.first_sub_obj) {
        for grnote in grnote_list {
            if let Some(note_ctx) = ctx.get(grnote.header.staffn) {
                if !note_ctx.visible || !grnote.header.visible {
                    continue;
                }

                // Grace note size: 70% of normal (GRACESIZE = 7*size/10)
                let grace_size_pct = 70.0_f32;

                let lnspace = lnspace_for_staff(note_ctx.staff_height, note_ctx.staff_lines);
                let half_sp = lnspace / 2.0;
                let head_width = 1.125 * lnspace; // HeadWidth (defs.h:355)
                                                  // Grace note head width is smaller (scaled by grace size)
                let grace_head_width = head_width * grace_size_pct / 100.0;

                // X position: measure_left + grsync xd + grnote xd
                // Reference: DrawUtils.cp GRNoteXLoc() line 1482-1509
                let xd_norm = d2r_sum3(note_ctx.measure_left, obj.header.xd, grnote.xd);

                // Apply otherStemSide offset (for grace note chords)
                let note_x = if grnote.other_stem_side && !grnote.rest {
                    let stem_down = find_stem_down(grnote, grnote_list);
                    if stem_down {
                        xd_norm - grace_head_width
                    } else {
                        xd_norm + grace_head_width
                    }
                } else {
                    xd_norm
                };

                // Y position: staff_top + grnote yd
                let note_y = d2r_sum(note_ctx.staff_top, grnote.yd);

                let l_dur = grnote.header.sub_type;

                // === Draw notehead at 70% size ===
                let notehead = notehead_glyph(grnote.head_shape, l_dur);
                if notehead != 0 {
                    renderer.music_char(
                        note_x,
                        note_y,
                        MusicGlyph::smufl(notehead),
                        grace_size_pct,
                    );
                }

                // === Draw accidental at 70% size ===
                if grnote.accident != 0 {
                    if let Some(acc_glyph) = accidental_glyph(grnote.accident) {
                        let chord_note_to_l = chord_note_to_left(grnote, grnote_list);
                        let acc_anchor = if chord_note_to_l {
                            xd_norm - grace_head_width
                        } else {
                            xd_norm
                        };
                        let xmove = if grnote.accident == AC_DBLFLAT {
                            (grnote.xmove_acc as i16 + 2).min(31)
                        } else {
                            grnote.xmove_acc as i16
                        };
                        let offset_ddist =
                            acc_x_offset(xmove, note_ctx.staff_height, note_ctx.staff_lines as i16);
                        let acc_x = acc_anchor - ddist_to_render(offset_ddist);
                        renderer.music_char(
                            acc_x,
                            note_y,
                            MusicGlyph::smufl(acc_glyph),
                            grace_size_pct,
                        );
                    }
                }

                // === Draw ledger lines (at grace note size) ===
                let ledgers = ledger_lines_for_note(grnote.yd, note_ctx.staff_height);
                if !ledgers.is_empty() {
                    let stem_down = grnote.ystem > grnote.yd;
                    // Scale ledger line dimensions for grace notes
                    let d_l_len = 1.5 * lnspace * grace_size_pct / 100.0;
                    let d_l_other_len = 0.375 * lnspace * grace_size_pct / 100.0;
                    let d_sticks_out = d_l_len - grace_head_width;

                    let ledger_left = if stem_down {
                        note_x - d_l_other_len
                    } else {
                        note_x - d_sticks_out
                    };
                    let ledger_len = d_l_len + d_l_other_len;
                    let ledger_center_x = ledger_left + ledger_len / 2.0;
                    let ledger_half_width = ledger_len / 2.0;

                    for halfline in ledgers {
                        let ledger_y = ddist_wide_to_render(
                            note_ctx.staff_top as i32
                                + (halfline as i32 * note_ctx.staff_height as i32 / 8),
                        );
                        renderer.ledger_line(ledger_y, ledger_center_x, ledger_half_width);
                    }
                }

                // === Draw stem + flag (grace notes always have stems except whole/breve) ===
                if l_dur > WHOLE_L_DUR && grnote.ystem != grnote.yd {
                    let stem_down = grnote.ystem > grnote.yd;

                    // Stem X uses grace note head width for upstem offset
                    let stem_x = if stem_down {
                        xd_norm
                    } else {
                        xd_norm + grace_head_width
                    };

                    let stem_near_yd = if grnote.in_chord {
                        let mut near_yd = grnote.yd;
                        for sibling in grnote_list {
                            if sibling.header.staffn == grnote.header.staffn
                                && sibling.voice == grnote.voice
                                && !sibling.rest
                            {
                                if stem_down {
                                    if sibling.yd < near_yd {
                                        near_yd = sibling.yd;
                                    }
                                } else if sibling.yd > near_yd {
                                    near_yd = sibling.yd;
                                }
                            }
                        }
                        near_yd
                    } else {
                        grnote.yd
                    };

                    let beam_extend = if grnote.beamed { 0.5_f32 } else { 0.0 };
                    let stem_top = if stem_down {
                        d2r_sum(note_ctx.staff_top, grnote.ystem.min(stem_near_yd))
                    } else {
                        d2r_sum(note_ctx.staff_top, grnote.ystem.min(stem_near_yd)) - beam_extend
                    };
                    let stem_bottom = if stem_down {
                        d2r_sum(note_ctx.staff_top, grnote.ystem.max(stem_near_yd)) + beam_extend
                    } else {
                        d2r_sum(note_ctx.staff_top, grnote.ystem.max(stem_near_yd))
                    };

                    // Stem width: 8% of staff interline space (same as regular notes).
                    // Reference: PS_Stdio.cp:952, STEMLW_DFLT = 8 (% of lnSpace)
                    let stem_width = lnspace * 0.08;
                    renderer.note_stem(stem_x, stem_top, stem_bottom, stem_width);

                    // Flag for unbeamed grace notes
                    let flag_count = crate::utility::nflags(l_dur);
                    if !grnote.beamed && flag_count > 0 {
                        if let Some(flag) = flag_glyph(l_dur, !stem_down) {
                            // Sonata: flags at notehead X with octave Y correction
                            // SMuFL: flags at stem X, stem tip Y
                            let (flag_x, flag_y) = if renderer.uses_sonata_font() {
                                let scale_f = grace_size_pct / 100.0;
                                let octave_len = 7.0 * half_sp * scale_f;
                                let fy = if stem_down {
                                    d2r_sum(note_ctx.staff_top, grnote.ystem) - octave_len
                                } else {
                                    d2r_sum(note_ctx.staff_top, grnote.ystem) + octave_len
                                };
                                (xd_norm, fy)
                            } else {
                                (stem_x, d2r_sum(note_ctx.staff_top, grnote.ystem))
                            };
                            renderer.music_char(
                                flag_x,
                                flag_y,
                                MusicGlyph::smufl(flag),
                                grace_size_pct,
                            );
                        }
                    }

                    // === Slash across stem (grace note characteristic slash) ===
                    // OG: if (config.slashGraceStems!=0 && flagCount==1) slashStem = True
                    // Also slashes beamed first-in-beam if config > 1.
                    // Default: always slash single-flag (eighth) grace notes.
                    // Reference: DrawNRGR.cp lines 1773-1799
                    let slash_stem = if flag_count == 1 && !grnote.beamed {
                        true
                    } else {
                        // Slash first note of beamed grace group when config > 1
                        // We don't track beam position, so skip beamed slashing for now
                        false
                    };

                    if slash_stem {
                        // Slash geometry:
                        // xdStem2Slash = -3*dhalfSp/4
                        // xdSlash = xd + (stemDown? 0 : headWidth) + xdStem2Slash
                        // ydSlash = dTop + ystem + (stemDown? -2*dhalfSp : 7*dhalfSp/2)
                        // slashLen = 5*dhalfSp/2
                        // slYDelta = slashLen (GRNOTE_SLASH_YDELTA = slashLen for 45° angle)
                        // Reference: DrawNRGR.cp lines 1773-1781
                        let xd_stem2slash = -3.0 * half_sp / 4.0;
                        let slash_x = if stem_down {
                            note_x + xd_stem2slash
                        } else {
                            note_x + grace_head_width + xd_stem2slash
                        };
                        let ystem_y = d2r_sum(note_ctx.staff_top, grnote.ystem);
                        let slash_y = if stem_down {
                            ystem_y - 2.0 * half_sp
                        } else {
                            ystem_y + 3.5 * half_sp
                        };
                        let slash_len = 2.5 * half_sp;
                        let sl_y_delta = slash_len; // ~45° angle
                                                    // Slash line weight: config.graceSlashLW% of lnSpace, default ~13%
                        let slash_thick = 0.13 * lnspace;

                        renderer.line(
                            slash_x,
                            slash_y,
                            slash_x + slash_len,
                            slash_y - sl_y_delta,
                            slash_thick,
                        );
                    }
                }

                // === Augmentation dots (rare for grace notes, but possible) ===
                if grnote.ndots > 0 && grnote.y_move_dots != 0 {
                    draw_aug_dots_note(grnote, note_x, note_y, l_dur, lnspace, half_sp, renderer);
                }
            }
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

/// Collect slur endpoint information from a Sync object's notes.
///
/// Port of IICreateAllSlurs matching logic (InternalInput.cp:881-918).
/// Slurs match by voice (first slurred_r → next slurred_l in same voice),
/// NOT by pitch like ties.
///
/// Reference: NotelistOpen.cp line 294 (IICreateAllSlurs call)
pub fn collect_slur_endpoints(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    slur_starts: &mut Vec<TieEndpoint>,
    slur_ends: &mut Vec<TieEndpoint>,
) {
    if let Some(anote_list) = score.notes.get(&obj.header.first_sub_obj) {
        for anote in anote_list {
            if anote.rest || (!anote.slurred_l && !anote.slurred_r) {
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

                // Determine stem direction for slur curvature direction
                // OG: curveUp = (NoteYSTEM > NoteYD) i.e. stem goes down → slur curves up
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

                if anote.slurred_r {
                    slur_starts.push(ep.clone());
                }
                if anote.slurred_l {
                    slur_ends.push(ep);
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

// =============================================================================
// MODNR (Note Modifier) Rendering
// Port of DrawNRGR.cp DrawModNR() (lines 195-245) + Draw1ModNR() (lines 105-188)
// =============================================================================

/// SMuFL codepoints for note modifier glyphs.
///
/// Mapped from OG Sonata MCH_* constants (defs.h:171-190) to SMuFL equivalents.
mod mod_glyphs {
    // Fermatas
    pub const FERMATA_ABOVE: u32 = 0xE4C0; // fermataAbove
    pub const FERMATA_BELOW: u32 = 0xE4C1; // fermataBelow

    // Articulations
    pub const ACCENT_ABOVE: u32 = 0xE4A0; // articAccentAbove
    pub const ACCENT_BELOW: u32 = 0xE4A1; // articAccentBelow
    pub const MARCATO_ABOVE: u32 = 0xE4AC; // articMarcatoAbove (heavyAccent)
    pub const MARCATO_BELOW: u32 = 0xE4AD; // articMarcatoBelow
    pub const STACCATO_ABOVE: u32 = 0xE4A2; // articStaccatoAbove
    pub const STACCATO_BELOW: u32 = 0xE4A3; // articStaccatoBelow
    pub const STACCATISSIMO_ABOVE: u32 = 0xE4A8; // articStaccatissimoAbove (wedge)
    pub const STACCATISSIMO_BELOW: u32 = 0xE4A9; // articStaccatissimoBelow
    pub const TENUTO_ABOVE: u32 = 0xE4A4; // articTenutoAbove
    pub const TENUTO_BELOW: u32 = 0xE4A5; // articTenutoBelow
    pub const MARCATO_STACCATO_ABOVE: u32 = 0xE4AE; // articMarcatoStaccatoAbove
    pub const MARCATO_STACCATO_BELOW: u32 = 0xE4AF; // articMarcatoStaccatoBelow

    // Ornaments
    pub const TRILL: u32 = 0xE566; // ornamentTrill
    pub const MORDENT: u32 = 0xE56C; // ornamentMordent
    pub const INV_MORDENT: u32 = 0xE56D; // ornamentShortTrill (inverted mordent)
    pub const TURN: u32 = 0xE567; // ornamentTurn
    pub const LONG_INV_MORDENT: u32 = 0xE56E; // ornamentTremblement

    // Bowing marks
    pub const UPBOW: u32 = 0xE612; // stringsUpBow
    pub const DOWNBOW: u32 = 0xE610; // stringsDownBow

    // Other
    pub const PLUS: u32 = 0xE633; // pluckedLeftHandPizzicato (+)
    pub const CIRCLE: u32 = 0xE614; // stringsHarmonic (natural harmonic circle)
}

/// Modifier code constants matching OG NObjTypes.h lines 525-549.
/// These are matched against AModNr.mod_code.
const MOD_FERMATA: u8 = 10;
const MOD_TRILL: u8 = 11;
const MOD_ACCENT: u8 = 12;
const MOD_HEAVYACCENT: u8 = 13;
const MOD_STACCATO: u8 = 14;
const MOD_WEDGE: u8 = 15;
const MOD_TENUTO: u8 = 16;
const MOD_MORDENT: u8 = 17;
const MOD_INV_MORDENT: u8 = 18;
const MOD_TURN: u8 = 19;
const MOD_PLUS: u8 = 20;
const MOD_CIRCLE: u8 = 21;
const MOD_UPBOW: u8 = 22;
const MOD_DOWNBOW: u8 = 23;
const MOD_TREMOLO1: u8 = 24;
const MOD_TREMOLO6: u8 = 29;
const MOD_HEAVYACC_STACC: u8 = 30;
const MOD_LONG_INVMORDENT: u8 = 31;

/// Size percentages from GetModNRInfo (DrawUtils.cp:1338-1448).
const CIRCLE_SIZEPCT: f32 = 150.0;
const _FINGERING_SIZEPCT: f32 = 65.0; // Will be used when fingering rendering is added

/// Get glyph and positioning info for a modifier code.
///
/// Port of GetModNRInfo() from DrawUtils.cp lines 1338-1448.
///
/// Returns (glyph, x_offset, y_offset, size_pct) where offsets are in eighth-spaces.
/// `above` determines which variant to use for directional glyphs.
fn get_modnr_info(code: u8, above: bool, small: bool) -> Option<(u32, i16, i16, f32)> {
    let size_pct = if small { 75.0 } else { 100.0 };

    match code {
        // Fingering digits 0-5
        0..=5 => {
            // Fingerings: '0' + code, positioned above note, scaled down
            // We use regular digit characters (not SMuFL music glyphs)
            // For now, skip fingerings (they need text rendering, not music_char)
            None
        }

        MOD_FERMATA => {
            let glyph = if above {
                mod_glyphs::FERMATA_ABOVE
            } else {
                mod_glyphs::FERMATA_BELOW
            };
            Some((glyph, -5, if above { 4 } else { -4 }, size_pct))
        }

        MOD_TRILL => Some((mod_glyphs::TRILL, 0, if above { 4 } else { 0 }, size_pct)),

        MOD_ACCENT => {
            let glyph = if above {
                mod_glyphs::ACCENT_ABOVE
            } else {
                mod_glyphs::ACCENT_BELOW
            };
            Some((glyph, 0, if above { 3 } else { 0 }, size_pct))
        }

        MOD_HEAVYACCENT => {
            let glyph = if above {
                mod_glyphs::MARCATO_ABOVE
            } else {
                mod_glyphs::MARCATO_BELOW
            };
            Some((glyph, 0, if above { 5 } else { 0 }, size_pct))
        }

        MOD_STACCATO => {
            let glyph = if above {
                mod_glyphs::STACCATO_ABOVE
            } else {
                mod_glyphs::STACCATO_BELOW
            };
            Some((glyph, 4, 0, size_pct))
        }

        MOD_WEDGE => {
            let glyph = if above {
                mod_glyphs::STACCATISSIMO_ABOVE
            } else {
                mod_glyphs::STACCATISSIMO_BELOW
            };
            Some((glyph, 2, if above { 2 } else { 0 }, size_pct))
        }

        MOD_TENUTO => {
            let glyph = if above {
                mod_glyphs::TENUTO_ABOVE
            } else {
                mod_glyphs::TENUTO_BELOW
            };
            Some((glyph, 0, 0, size_pct))
        }

        MOD_MORDENT => Some((mod_glyphs::MORDENT, -4, if above { 4 } else { 0 }, size_pct)),
        MOD_INV_MORDENT => Some((
            mod_glyphs::INV_MORDENT,
            -4,
            if above { 4 } else { 0 },
            size_pct,
        )),
        MOD_TURN => Some((mod_glyphs::TURN, -4, 0, size_pct)),
        MOD_PLUS => Some((mod_glyphs::PLUS, 0, 0, size_pct)),

        MOD_CIRCLE => {
            let circle_size = if small {
                CIRCLE_SIZEPCT * 0.75
            } else {
                CIRCLE_SIZEPCT
            };
            Some((mod_glyphs::CIRCLE, 2, 0, circle_size))
        }

        MOD_UPBOW => Some((mod_glyphs::UPBOW, 1, if above { 6 } else { 0 }, size_pct)),
        MOD_DOWNBOW => Some((mod_glyphs::DOWNBOW, 0, if above { 5 } else { 0 }, size_pct)),

        // Tremolos (24-29): drawn as slashes, not glyphs — handled separately
        MOD_TREMOLO1..=MOD_TREMOLO6 => None,

        MOD_HEAVYACC_STACC => {
            let glyph = if above {
                mod_glyphs::MARCATO_STACCATO_ABOVE
            } else {
                mod_glyphs::MARCATO_STACCATO_BELOW
            };
            Some((glyph, 0, if above { 5 } else { 0 }, size_pct))
        }

        MOD_LONG_INVMORDENT => Some((
            mod_glyphs::LONG_INV_MORDENT,
            -6,
            if above { 4 } else { 0 },
            size_pct,
        )),

        _ => None, // Unknown modifier code
    }
}

/// Draw tremolo slashes on a stem.
///
/// Port of DrawSlashes() from DrawNRGR.cp lines 33-96.
///
/// Draws `n_slashes` diagonal slash marks across the stem at the note's ystem position.
/// Slash angle is ~45°, width matches notehead width.
#[allow(clippy::too_many_arguments)]
fn draw_tremolo_slashes(
    note_x: f32,
    _note_y: f32,
    ystem_y: f32,
    n_slashes: u8,
    stem_down: bool,
    is_whole: bool,
    lnspace: f32,
    renderer: &mut dyn MusicRenderer,
) {
    let eighth_sp = lnspace / 8.0;

    // Slash dimensions (DrawNRGR.cp:49-52)
    let slash_width = 1.125 * lnspace; // HeadWidth
    let slash_height = lnspace / 2.0;
    // TREMSLASHLW_DFLT = 25 (Initialize.cp), config.tremSlashLW * lnSpace / 100
    let slash_thick = 25.0 * lnspace / 100.0;

    // Slash spacing (6 eighth-spaces between slash centers)
    let slash_leading = if stem_down {
        6.0 * eighth_sp
    } else {
        -6.0 * eighth_sp
    };

    // Horizontal offset from stem position
    let dxpos = if is_whole {
        0.0 // centered for whole notes
    } else if stem_down {
        4.0 * eighth_sp
    } else {
        -5.0 * eighth_sp
    };

    // Vertical offset from ystem
    let dypos = if stem_down {
        8.0 * eighth_sp
    } else {
        -8.0 * eighth_sp
    };

    let base_x = note_x + dxpos;
    let base_y = ystem_y + dypos;

    for i in 0..n_slashes {
        let cy = base_y + (i as f32) * slash_leading;
        // Slash goes from (x, cy + half_height) to (x + width, cy - half_height)
        // for stem-up, or mirrored for stem-down
        if stem_down {
            renderer.line(
                base_x,
                cy + slash_height,
                base_x + slash_width,
                cy - slash_height,
                slash_thick,
            );
        } else {
            renderer.line(
                base_x,
                cy - slash_height,
                base_x + slash_width,
                cy + slash_height,
                slash_thick,
            );
        }
    }
}

/// Draw all modifiers (articulations, ornaments, etc.) for a note or rest.
///
/// Port of DrawModNR() from DrawNRGR.cp lines 195-245.
///
/// Walks the MODNR linked list for this note (via first_mod) and draws each modifier
/// glyph at its computed position. Uses GetModNRInfo for glyph selection and offsets.
pub fn draw_modnrs(
    score: &InterpretedScore,
    anote: &ANote,
    note_ctx: &Context,
    note_x: f32,
    note_y: f32,
    lnspace: f32,
    renderer: &mut dyn MusicRenderer,
) {
    use crate::basic_types::NILINK;

    if anote.first_mod == NILINK || anote.first_mod == 0 {
        return;
    }

    let mods = match score.modnrs.get(&anote.first_mod) {
        Some(m) => m,
        None => return,
    };

    let staff_height = note_ctx.staff_height;
    let staff_lines = note_ctx.staff_lines as i16;
    let eighth_sp = lnspace / 8.0;

    for modnr in mods {
        if !modnr.visible {
            continue;
        }

        let code = modnr.mod_code;

        // Convert xstd (biased by XSTD_OFFSET) to DDIST offset from note position
        // Source: DrawNRGR.cp line 218: xdMod = xd + std2d(aModNR->xstd - XSTD_OFFSET, ...)
        let xstd_signed = modnr.xstd as i16 - XSTD_OFFSET as i16;
        let xd_mod_ddist = std2d(xstd_signed, staff_height, staff_lines);
        let xd_mod = note_x + ddist_to_render(xd_mod_ddist);

        // Convert ystdpit to DDIST — this is an absolute staff position
        // Source: DrawNRGR.cp line 219: ydMod = std2d(aModNR->ystdpit, ...)
        let yd_mod_ddist = std2d(modnr.ystdpit as i16, staff_height, staff_lines);
        let yd_mod = ddist_to_render(yd_mod_ddist);

        // Determine if modifier is above or below the note
        // Source: DrawNRGR.cp line 225: above = (ydMod <= aNote->yd)
        // (In Nightingale, smaller y = higher on page)
        let above = yd_mod_ddist <= anote.yd;

        // Handle tremolos separately
        if (MOD_TREMOLO1..=MOD_TREMOLO6).contains(&code) {
            let n_slashes = code - MOD_TREMOLO1 + 1;
            let ystem_y = d2r_sum(note_ctx.staff_top, anote.ystem);
            let stem_down = anote.ystem > anote.yd;
            let is_whole = anote.header.sub_type <= WHOLE_L_DUR;
            draw_tremolo_slashes(
                note_x,
                note_y,
                ystem_y + yd_mod, // ydMod offsets from ystem for tremolos
                n_slashes,
                stem_down,
                is_whole,
                lnspace,
                renderer,
            );
            continue;
        }

        // Get glyph and offsets for this modifier type
        if let Some((glyph, x_offset, y_offset, size_pct)) =
            get_modnr_info(code, above, anote.small)
        {
            // Apply offsets (in eighth-spaces)
            // Source: DrawNRGR.cp lines 227-228
            let final_x = xd_mod + (eighth_sp * x_offset as f32);
            let final_y = d2r_sum(note_ctx.staff_top, 0) + yd_mod + (eighth_sp * y_offset as f32);

            renderer.music_char(final_x, final_y, MusicGlyph::smufl(glyph), size_pct);
        }
    }
}
