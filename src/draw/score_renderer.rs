//! Score rendering implementation — ported from DrawHighLevel.cp and DrawObject.cp.
//!
//! This module provides the main `render_score()` function and type-specific drawing
//! functions for each object type. Supports Staff, Measure, Sync (notes/rests),
//! Clef, TimeSig, BeamSet, and Connect objects.
//!
//! Reference C++ files:
//! - DrawHighLevel.cp: DrawScoreRange() (lines 1145-1289)
//! - DrawObject.cp: Draw1Staff() (line 591), DrawMEASURE() (line 2772),
//!   DrawCLEF() (line 1075), DrawTIMESIG() (line 1248), etc.
//! - DrawNRGR.cp: DrawSYNC() (line 1509), DrawNote() (line 662)
//! - DrawBeam.cp: DrawBEAMSET() (line 89)

use crate::context::ContextState;
use crate::defs::*;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::render::types::{ddist_to_render, BarLineType, MusicGlyph, Point};
use crate::render::MusicRenderer;

/// Information about a note's rendered position, used for tie matching.
///
/// Collected during the draw pass and matched after all objects are drawn.
/// Each TieEndpoint records the rendered (x, y) of the notehead center,
/// along with identifying info (staff, voice, note_num) and stem direction.
#[derive(Debug, Clone)]
struct TieEndpoint {
    /// Rendered X of the notehead origin (left edge of glyph)
    x: f32,
    /// Rendered Y of the notehead center
    y: f32,
    /// Note width (for endpoint offset computation)
    head_width: f32,
    /// True if stem goes down (=> tie curves up above note)
    stem_down: bool,
    /// Staff number (for matching)
    staff: i8,
    /// Voice number (for matching)
    voice: i8,
    /// MIDI note number (for pitch matching)
    note_num: u8,
    /// Line spacing at this note's staff
    lnspace: f32,
    /// Right edge of this note's staff (for cross-system partial ties)
    staff_right: f32,
    /// Left edge of this note's staff (for cross-system partial ties)
    staff_left: f32,
}

/// Count the number of staves in a score by examining the first Staff object.
///
/// Returns 0 if no Staff object is found.
fn count_staves(score: &InterpretedScore) -> usize {
    for obj in score.walk() {
        if let ObjData::Staff(_) = &obj.data {
            if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
                return astaff_list.len();
            }
        }
    }
    0
}

/// Map l_dur (logical duration) to notehead glyph for notes.
///
/// Reference: DrawNRGR.cp, MusCharXLoc() and GetMusicAscDesc()
/// SMuFL noteheads:
/// - BREVE_L_DUR (1): 0xE0A0 (noteheadDoubleWhole)
/// - WHOLE_L_DUR (2): 0xE0A2 (noteheadWhole)
/// - HALF_L_DUR (3): 0xE0A3 (noteheadHalf)
/// - QTR_L_DUR+ (4+): 0xE0A4 (noteheadBlack)
fn notehead_glyph_for_duration(l_dur: i8) -> u32 {
    match l_dur {
        x if x == BREVE_L_DUR => 0xE0A0, // noteheadDoubleWhole
        x if x == WHOLE_L_DUR => 0xE0A2, // noteheadWhole
        x if x == HALF_L_DUR => 0xE0A3,  // noteheadHalf
        _ => 0xE0A4,                     // noteheadBlack (quarter and shorter)
    }
}

/// Map l_dur (logical duration) to rest glyph.
///
/// Reference: DrawNRGR.cp, DrawRest() (line 1402)
/// SMuFL rests:
/// - BREVE_L_DUR (1): 0xE4E2 (restDoubleWhole)
/// - WHOLE_L_DUR (2): 0xE4E3 (restWhole)
/// - HALF_L_DUR (3): 0xE4E4 (restHalf)
/// - QTR_L_DUR (4): 0xE4E5 (restQuarter)
/// - EIGHTH_L_DUR (5): 0xE4E6 (restEighth)
/// - SIXTEENTH_L_DUR (6): 0xE4E7 (rest16th)
fn rest_glyph_for_duration(l_dur: i8) -> u32 {
    match l_dur {
        x if x == BREVE_L_DUR => 0xE4E2,
        x if x == WHOLE_L_DUR => 0xE4E3,
        x if x == HALF_L_DUR => 0xE4E4,
        x if x == QTR_L_DUR => 0xE4E5,
        x if x == EIGHTH_L_DUR => 0xE4E6,
        x if x == SIXTEENTH_L_DUR => 0xE4E7,
        _ => 0xE4E5, // Default to quarter rest
    }
}

/// Map accidental code to SMuFL glyph.
///
/// Accidental codes (from NObjTypes.h, ANote.accident field):
/// - 0: none
/// - 1: double flat (0xE264)
/// - 2: flat (0xE260)
/// - 3: natural (0xE261)
/// - 4: sharp (0xE262)
/// - 5: double sharp (0xE263)
fn accidental_glyph(accident_code: u8) -> Option<u32> {
    match accident_code {
        1 => Some(0xE264), // accidentalDoubleFlat
        2 => Some(0xE260), // accidentalFlat
        3 => Some(0xE261), // accidentalNatural
        4 => Some(0xE262), // accidentalSharp
        5 => Some(0xE263), // accidentalDoubleSharp
        _ => None,
    }
}

/// Map clef type to SMuFL glyph.
///
/// Reference: DrawObject.cp, DrawCLEF() (line 1075)
/// Clef types from obj_types.rs:
/// - 1: Treble 8va bassa (0xE052)
/// - 3: Treble (0xE050)
/// - 6: Alto (0xE05C)
/// - 8: Tenor (0xE05C, same glyph as alto)
/// - 10: Bass (0xE062)
/// - 12: Percussion (0xE069)
fn clef_glyph(clef_type: i8) -> u32 {
    match clef_type {
        1 => 0xE052,  // gClef8vb
        3 => 0xE050,  // gClef
        6 => 0xE05C,  // cClef (alto)
        8 => 0xE05C,  // cClef (tenor, same glyph)
        10 => 0xE062, // fClef
        12 => 0xE069, // unpitchedPercussionClef1
        _ => 0xE050,  // Default to treble
    }
}

/// Get the Y position (in half-lines from staff top) for a clef glyph origin.
///
/// Reference: DrawObject.cp, DrawCLEF() (line 1088-1094)
/// Half-lines are counted from staff top: 0 = top line, 8 = bottom line (5-line staff)
///
/// Returns half-line position where the clef glyph origin should be placed:
/// - Treble (3): 6 (sits on second line from bottom, G line)
/// - Bass (10): 2 (sits on fourth line from top, F line)
/// - Alto (6): 4 (sits on middle line)
/// - Tenor (8): 4 (sits on middle line)
/// - Treble 8vb (1): 6 (same as treble)
/// - Percussion (12): 4 (middle)
fn clef_halfline_position(clef_type: i8) -> i16 {
    match clef_type {
        1 => 6,  // Treble 8vb
        3 => 6,  // Treble
        6 => 4,  // Alto
        8 => 4,  // Tenor
        10 => 2, // Bass
        12 => 4, // Percussion
        _ => 4,  // Default to middle
    }
}

/// Get flag glyph for unbeamed eighth/sixteenth notes.
///
/// Reference: DrawNRGR.cp, DrawModNR() (line 1158)
/// SMuFL flags:
/// - 8th up: 0xE240, down: 0xE241
/// - 16th up: 0xE242, down: 0xE243
fn flag_glyph(l_dur: i8, stem_up: bool) -> Option<u32> {
    match l_dur {
        x if x == EIGHTH_L_DUR => {
            if stem_up {
                Some(0xE240) // flag8thUp
            } else {
                Some(0xE241) // flag8thDown
            }
        }
        x if x == SIXTEENTH_L_DUR => {
            if stem_up {
                Some(0xE242) // flag16thUp
            } else {
                Some(0xE243) // flag16thDown
            }
        }
        _ => None,
    }
}

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
fn ledger_lines_for_note(yd: i16, staff_height: i16) -> Vec<i16> {
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

/// Render an entire score through a MusicRenderer.
///
/// This is the Rust equivalent of DrawScoreRange() from DrawHighLevel.cp (line 1145).
///
/// Walks the score object list from head to tail, updating context at each object
/// and dispatching to type-specific drawing functions.
///
/// # Arguments
///
/// * `score` - The interpreted score to render
/// * `renderer` - The rendering backend (PDF, Flutter, etc.)
///
/// Reference: DrawHighLevel.cp, DrawScoreRange(), lines 1145-1289
pub fn render_score(score: &InterpretedScore, renderer: &mut dyn MusicRenderer) {
    let num_staves = count_staves(score);
    if num_staves == 0 {
        return;
    }

    // Set default line widths (matches C++ defaults)
    // Source: PS_Stdio.cp, PS_SetWidths() default values
    renderer.set_widths(0.8, 0.8, 0.8, 0.8);

    let mut ctx = ContextState::new(num_staves);

    // Collect tie endpoints during the walk for post-draw tie rendering.
    // tie_starts: notes with tied_r (start of tie arc)
    // tie_ends: notes with tied_l (end of tie arc)
    let mut tie_starts: Vec<TieEndpoint> = Vec::new();
    let mut tie_ends: Vec<TieEndpoint> = Vec::new();

    for obj in score.walk() {
        // Update context BEFORE drawing (matches C++ pipeline)
        ctx.update_from_object(obj, score);

        match &obj.data {
            ObjData::Staff(_) => draw_staff(score, obj, &ctx, renderer),
            ObjData::Measure(_) => draw_measure(score, obj, &ctx, renderer),
            ObjData::Sync(_) => {
                draw_sync(score, obj, &ctx, renderer);
                collect_tie_endpoints(score, obj, &ctx, &mut tie_starts, &mut tie_ends);
            }
            ObjData::Connect(_) => draw_connect(score, obj, &ctx, renderer),
            ObjData::Clef(_) => draw_clef(score, obj, &ctx, renderer),
            ObjData::TimeSig(_) => draw_timesig(score, obj, &ctx, renderer),
            ObjData::BeamSet(_) => draw_beamset(score, obj, &ctx, renderer),
            // TODO: Slur, Tuplet, KeySig, Dynamic, Tempo, Graphic, Ottava, Ending, etc.
            _ => {}
        }
    }

    // Draw ties after all objects (so ties layer on top)
    draw_ties(&tie_starts, &tie_ends, renderer);
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
fn draw_staff(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
        for astaff in astaff_list {
            if let Some(staff_ctx) = ctx.get(astaff.staffn) {
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
fn draw_measure(
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
                    let bottom_y =
                        ddist_to_render(measure_ctx.staff_top + measure_ctx.staff_height);

                    // Map subtype to BarLineType
                    let bar_type = map_barline_type(ameasure.header.sub_type);

                    renderer.bar_line(top_y, bottom_y, x, bar_type);
                }
            }
        }
    }
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
fn draw_sync(
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
                    // Compute note X: measure_left + sync xd + note xd
                    let note_x = ddist_to_render(note_ctx.measure_left + obj.header.xd + anote.xd);

                    // Compute note Y: staff_top + note yd (yd is relative to staff top)
                    let note_y = ddist_to_render(note_ctx.staff_top + anote.yd);

                    // l_dur is stored in header.sub_type
                    let l_dur = anote.header.sub_type;

                    // Line spacing for ledger lines and notehead sizing
                    let lnspace = if note_ctx.staff_lines > 1 {
                        ddist_to_render(note_ctx.staff_height) / (note_ctx.staff_lines as f32 - 1.0)
                    } else {
                        8.0
                    };

                    // Half-space for dot positioning
                    let half_sp = lnspace / 2.0;

                    if !anote.rest {
                        // === NOTES ===

                        // Draw notehead
                        let notehead_glyph = notehead_glyph_for_duration(l_dur);
                        renderer.music_char(
                            note_x,
                            note_y,
                            MusicGlyph::smufl(notehead_glyph),
                            100.0,
                        );

                        // Draw accidental if present
                        if anote.accident != 0 {
                            if let Some(acc_glyph) = accidental_glyph(anote.accident) {
                                // Position accidental to the left of notehead
                                // Typical offset: ~1.5 * lnspace
                                let acc_x = note_x - (1.5 * lnspace);
                                renderer.music_char(
                                    acc_x,
                                    note_y,
                                    MusicGlyph::smufl(acc_glyph),
                                    100.0,
                                );
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
                                let ledger_y = ddist_to_render(
                                    note_ctx.staff_top + (halfline * note_ctx.staff_height / 8),
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

                            // Stem X: on right side for stems-up, left side for stems-down
                            let stem_x = if stem_down {
                                note_x // Stems down: left side of notehead
                            } else {
                                note_x + head_width // Stems up: right side
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
                            let stem_top =
                                ddist_to_render(note_ctx.staff_top + anote.ystem.min(stem_near_yd));
                            let stem_bottom =
                                ddist_to_render(note_ctx.staff_top + anote.ystem.max(stem_near_yd));

                            // Stem width (default from set_widths)
                            let stem_width = 0.8;

                            renderer.note_stem(stem_x, stem_top, stem_bottom, stem_width);

                            // Draw flag if unbeamed eighth/sixteenth
                            // Check if note is beamed: beamed flag is in anote.beamed
                            if !anote.beamed {
                                if let Some(flag) = flag_glyph(l_dur, !stem_down) {
                                    // Flag is positioned at stem endpoint (ystem)
                                    let flag_x = stem_x;
                                    let flag_y = ddist_to_render(note_ctx.staff_top + anote.ystem);
                                    renderer.music_char(
                                        flag_x,
                                        flag_y,
                                        MusicGlyph::smufl(flag),
                                        100.0,
                                    );
                                }
                            }
                        }

                        // Draw augmentation dots if any
                        // Faithful port of DrawAugDots (DrawNRGR.cp:248-307)
                        //                  + AugDotXDOffset (DrawUtils.cp:1532-1582)
                        if anote.ndots > 0 && anote.y_move_dots != 0 {
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
                                renderer.music_char(
                                    dot_x,
                                    yd_dots,
                                    MusicGlyph::smufl(dot_glyph),
                                    100.0,
                                );
                            }
                        }
                    } else {
                        // === RESTS ===

                        // Draw rest glyph
                        let rest_glyph = rest_glyph_for_duration(l_dur);
                        renderer.music_char(note_x, note_y, MusicGlyph::smufl(rest_glyph), 100.0);

                        // Draw augmentation dots on rests
                        // Port of DrawAugDots (DrawNRGR.cp:1388/1458) for rests
                        // AugDotXDOffset for rests: xdDots = dhalfSp, + dhalfSp if IS_WIDEREST
                        if anote.ndots > 0 && anote.y_move_dots != 0 {
                            let mut xd_offset = half_sp;
                            // IS_WIDEREST: whole/half rests are wider
                            if l_dur <= 3 {
                                xd_offset += half_sp;
                            }
                            // xMoveDots fine-tune (same formula as notes)
                            xd_offset += (anote.x_move_dots as f32 - 3.0) * lnspace / 4.0;

                            let yd_dots = note_y + (anote.y_move_dots as f32 - 2.0) * half_sp;
                            let dot_glyph = 0xE1E7_u32;

                            let mut dot_x = note_x + xd_offset;
                            for _ in 0..anote.ndots {
                                dot_x += lnspace; // OG: += 2*dhalfSp before each draw
                                renderer.music_char(
                                    dot_x,
                                    yd_dots,
                                    MusicGlyph::smufl(dot_glyph),
                                    100.0,
                                );
                            }
                        }
                    }
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
fn collect_tie_endpoints(
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
                let note_x = ddist_to_render(note_ctx.measure_left + obj.header.xd + anote.xd);
                let note_y = ddist_to_render(note_ctx.staff_top + anote.yd);

                let lnspace = if note_ctx.staff_lines > 1 {
                    ddist_to_render(note_ctx.staff_height) / (note_ctx.staff_lines as f32 - 1.0)
                } else {
                    8.0
                };
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
                    staff_right: ddist_to_render(note_ctx.staff_right),
                    staff_left: ddist_to_render(note_ctx.staff_left),
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
fn draw_ties(
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
fn draw_connect(
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
                    let x = ddist_to_render(top_ctx.staff_left + aconnect.xd);

                    // Top Y from top staff top
                    let y_top = ddist_to_render(top_ctx.staff_top);

                    // Bottom Y from bottom staff top + height
                    let y_bottom = ddist_to_render(bottom_ctx.staff_top + bottom_ctx.staff_height);

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
fn draw_clef(
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
                    let clef_x = ddist_to_render(base_x + obj.header.xd + aclef.xd);

                    // Y position: staff_top + half-line offset
                    let halfline = clef_halfline_position(clef_type);
                    let lnspace = if clef_ctx.staff_lines > 1 {
                        ddist_to_render(clef_ctx.staff_height) / (clef_ctx.staff_lines as f32 - 1.0)
                    } else {
                        8.0
                    };
                    let clef_y =
                        ddist_to_render(clef_ctx.staff_top) + (halfline as f32 * lnspace / 2.0);

                    renderer.music_char(clef_x, clef_y, MusicGlyph::smufl(glyph), 100.0);
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
fn draw_timesig(
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
                    let base_x = ddist_to_render(origin_x + obj.header.xd + atimesig.xd);

                    // Line spacing
                    let lnspace = if timesig_ctx.staff_lines > 1 {
                        ddist_to_render(timesig_ctx.staff_height)
                            / (timesig_ctx.staff_lines as f32 - 1.0)
                    } else {
                        8.0
                    };

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

/// Draw a BeamSet object (beam lines connecting notes).
///
/// Port of DrawBeam.cp DrawBEAMSET() (line 89).
///
/// A BeamSet contains ANoteBeam subobjects that link to syncs in the beam group.
/// We resolve each bp_sync link to find the beamed note's stem endpoint, then
/// draw horizontal (or slightly angled) beam segments connecting them.
///
/// The beam is drawn at the stem endpoints (ystem) of the notes in the group.
/// Primary beam connects all notes; secondary beams connect subgroups.
///
/// Reference: DrawBeam.cp, DrawBEAMSET(), line 89
fn draw_beamset(
    score: &InterpretedScore,
    obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    let beamset = match &obj.data {
        ObjData::BeamSet(bs) => bs,
        _ => return,
    };

    let notebeam_list = match score.notebeams.get(&obj.header.first_sub_obj) {
        Some(list) => list,
        None => return,
    };

    if notebeam_list.is_empty() {
        return;
    }

    // Resolve each ANoteBeam to (x, ystem_y) position.
    // We need the stem endpoint position and the note's l_dur for secondary beams.
    #[allow(dead_code)]
    struct BeamPoint {
        x: f32,
        ystem_y: f32,
        note_yd: f32, // Notehead Y — needed to determine stem direction (ystem > yd → stem down)
        l_dur: i8,
        startend: i8,
    }

    let mut points: Vec<BeamPoint> = Vec::new();

    for notebeam in notebeam_list {
        // Find the Sync object for bp_sync
        let sync_obj = match score.objects.iter().find(|o| o.index == notebeam.bp_sync) {
            Some(o) => o,
            None => continue,
        };

        // Get the staff context for this beamset
        let staff_ctx = match ctx.get(beamset.ext_header.staffn) {
            Some(c) => c,
            None => continue,
        };

        // Find a beamed note in this sync that matches the beamset voice
        if let Some(anote_list) = score.notes.get(&sync_obj.header.first_sub_obj) {
            // Find the first beamed note in this sync for this voice
            if let Some(note) = anote_list.iter().find(|n| {
                n.beamed && n.voice == beamset.voice && n.header.staffn == beamset.ext_header.staffn
            }) {
                let lnspace = if staff_ctx.staff_lines > 1 {
                    ddist_to_render(staff_ctx.staff_height) / (staff_ctx.staff_lines as f32 - 1.0)
                } else {
                    8.0
                };

                // Stem direction: stem_down = (ystem > yd)
                let stem_down = note.ystem > note.yd;

                // CalcXStem (Beam.cp:1238): PostScript path
                // HeadWidth (defs.h:355): 9*lnSp*4/32 = 1.125 * lnSpace
                let head_width = 1.125 * lnspace;

                // X position: same calculation as in draw_sync
                let note_x = ddist_to_render(staff_ctx.measure_left + sync_obj.header.xd + note.xd);
                let stem_x = if stem_down {
                    note_x
                } else {
                    note_x + head_width
                };

                // Y position at stem endpoint and notehead
                let ystem_y = ddist_to_render(staff_ctx.staff_top + note.ystem);
                let note_yd_y = ddist_to_render(staff_ctx.staff_top + note.yd);

                points.push(BeamPoint {
                    x: stem_x,
                    ystem_y,
                    note_yd: note_yd_y,
                    l_dur: note.header.sub_type,
                    startend: notebeam.startend,
                });
            }
        }
    }

    if points.len() < 2 {
        return;
    }

    // Beam thickness: standard is ~0.5 * lnspace
    let staff_ctx = match ctx.get(beamset.ext_header.staffn) {
        Some(c) => c,
        None => return,
    };
    let lnspace = if staff_ctx.staff_lines > 1 {
        ddist_to_render(staff_ctx.staff_height) / (staff_ctx.staff_lines as f32 - 1.0)
    } else {
        8.0
    };
    let beam_thickness = lnspace * 0.5;
    let beam_gap = lnspace * 0.25; // Gap between primary and secondary beams

    // Determine stem direction from first note's ystem vs yd.
    // stem_down = (ystem > yd) in DDIST coords, which means (ystem_y > note_yd) in render coords
    // (Y increases downward). This matches the OG Nightingale convention used in draw_sync.
    let stem_up = points[0].ystem_y < points[0].note_yd;

    // Draw primary beam: connects first point to last point
    let first = &points[0];
    let last = &points[points.len() - 1];
    renderer.beam(
        first.x,
        first.ystem_y,
        last.x,
        last.ystem_y,
        beam_thickness,
        stem_up,
        stem_up,
    );

    // Draw secondary beams for 16th notes (l_dur >= SIXTEENTH_L_DUR)
    // A secondary beam connects consecutive notes that both have l_dur >= 6
    let mut i = 0;
    while i < points.len() {
        if points[i].l_dur >= SIXTEENTH_L_DUR {
            // Find extent of consecutive 16th+ notes
            let start = i;
            while i < points.len() && points[i].l_dur >= SIXTEENTH_L_DUR {
                i += 1;
            }
            let end = i - 1;

            if start < end {
                // Draw secondary beam segment
                let y_offset = if stem_up {
                    beam_thickness + beam_gap
                } else {
                    -(beam_thickness + beam_gap)
                };

                // Interpolate Y positions along the primary beam
                let p0 = &points[start];
                let p1 = &points[end];
                renderer.beam(
                    p0.x,
                    p0.ystem_y + y_offset,
                    p1.x,
                    p1.ystem_y + y_offset,
                    beam_thickness,
                    stem_up,
                    stem_up,
                );
            } else {
                // Single 16th note — draw fractional beam
                // Point toward the adjacent note
                let p = &points[start];
                let (frac_x, frac_y) = if start > 0 {
                    // Point left toward previous note
                    let prev = &points[start - 1];
                    let frac_len = (p.x - prev.x).min(lnspace);
                    (p.x - frac_len, p.ystem_y) // Simplified: horizontal frac beam
                } else if start + 1 < points.len() {
                    // Point right toward next note
                    let next = &points[start + 1];
                    let frac_len = (next.x - p.x).min(lnspace);
                    (p.x + frac_len, p.ystem_y)
                } else {
                    continue;
                };

                let y_offset = if stem_up {
                    beam_thickness + beam_gap
                } else {
                    -(beam_thickness + beam_gap)
                };

                renderer.beam(
                    p.x,
                    p.ystem_y + y_offset,
                    frac_x,
                    frac_y + y_offset,
                    beam_thickness,
                    stem_up,
                    stem_up,
                );
            }
        } else {
            i += 1;
        }
    }
}
