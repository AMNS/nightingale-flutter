//! Beam drawing — port of DrawBeam.cp.
//!
//! Draws BeamSet objects: primary and secondary beam segments connecting
//! notes in a beam group.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawBeam.cp

use crate::context::ContextState;
use crate::defs::{SIXTEENTH_L_DUR, THIRTY2ND_L_DUR};
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::render::types::ddist_to_render;
use crate::render::MusicRenderer;

use super::helpers::{d2r_sum, d2r_sum3, lnspace_for_staff};

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
pub fn draw_beamset(
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

    // Grace note beams use a separate code path with 70% scaling (Beam.cp:1693-1700, GRBeam.cp)
    // N105 files may not reliably set the grace flag, so also check if ANoteBeams point to GRSYNC objects
    let is_grace_beam = beamset.grace != 0 || {
        notebeam_list.iter().any(|nb| {
            score
                .objects
                .iter()
                .find(|o| o.index == nb.bp_sync)
                .map(|o| o.header.obj_type as u8 == crate::defs::GRSYNC_TYPE)
                .unwrap_or(false)
        })
    };

    if is_grace_beam {
        return draw_grace_beamset(score, beamset, notebeam_list, obj, ctx, renderer);
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

    // Cross-staff beam handling (port of Beam.cp DrawBEAMSET cross-staff logic).
    // For cross-staff beams, beamset.ext_header.staffn is the TOP staff.
    // Notes on the bottom staff have their ystem values transformed to the top
    // staff's coordinate system by adding heightDiff (the Y distance between
    // the two staves' staff_top values).
    // Reference: Beam.cp Staff2TopStaff() line 275, NoteXStfYStem() line 1332
    let is_cross_staff = beamset.cross_staff != 0;
    let top_staffn = beamset.ext_header.staffn;

    // For cross-staff beams, compute heightDiff = bottom_staff_top - top_staff_top
    // (in DDIST, positive because bottom staff is lower on the page = larger Y)
    let height_diff_ddist: i16 = if is_cross_staff {
        let bottom_staffn = top_staffn + 1; // Cross-staff always spans adjacent staves
        match (ctx.get(top_staffn), ctx.get(bottom_staffn)) {
            (Some(top_ctx), Some(bot_ctx)) => bot_ctx.staff_top - top_ctx.staff_top,
            _ => 0,
        }
    } else {
        0
    };

    for notebeam in notebeam_list {
        // Find the Sync object for bp_sync
        let sync_obj = match score.objects.iter().find(|o| o.index == notebeam.bp_sync) {
            Some(o) => o,
            None => continue,
        };

        // Get the staff context for the top staff (beam's reference staff)
        let staff_ctx = match ctx.get(top_staffn) {
            Some(c) => c,
            None => continue,
        };

        // Find the stem-bearing beamed note in this sync for the beamset voice.
        // In chords, multiple notes share beamed=true but only the "far" note
        // (farthest from the beam) has a meaningful ystem != yd. Prefer that note
        // so beam endpoints connect to actual stem tips.
        //
        // For cross-staff beams, don't filter by staffn — include notes on either
        // staff (they share the same voice). For normal beams, filter to the beam's staff.
        // Reference: Beam.cp DrawBEAMSET() line 1680 — GetBeamNotes collects from all staves
        if let Some(anote_list) = score.notes.get(&sync_obj.header.first_sub_obj) {
            let matching: Vec<&_> = anote_list
                .iter()
                .filter(|n| {
                    n.beamed
                        && n.voice == beamset.voice
                        && (is_cross_staff || n.header.staffn == top_staffn)
                })
                .collect();
            // Prefer the note with a real stem (ystem != yd); fall back to first match
            if let Some(note) = matching
                .iter()
                .find(|n| n.ystem != n.yd)
                .or(matching.first())
                .copied()
            {
                let lnspace = lnspace_for_staff(staff_ctx.staff_height, staff_ctx.staff_lines);

                // Stem direction: stem_down = (ystem > yd)
                let stem_down = note.ystem > note.yd;

                // CalcXStem (Beam.cp:1238): PostScript path
                // HeadWidth (defs.h:355): 9*lnSp*4/32 = 1.125 * lnSpace
                let head_width = 1.125 * lnspace;

                // X position: same calculation as in draw_sync
                // For cross-staff notes, use top staff's measure_left (same system, same X)
                let note_x = d2r_sum3(staff_ctx.measure_left, sync_obj.header.xd, note.xd);
                let stem_x = if stem_down {
                    note_x
                } else {
                    note_x + head_width
                };

                // Y position at stem endpoint and notehead.
                // For cross-staff beams, transform ystem/yd to the top staff's coordinate
                // system using Staff2TopStaff: if note is on the bottom staff, add heightDiff.
                // Reference: Beam.cp NoteXStfYStem() line 1332, Staff2TopStaff() line 275
                let ystem_ddist = if is_cross_staff && note.header.staffn != top_staffn {
                    note.ystem + height_diff_ddist
                } else {
                    note.ystem
                };
                let yd_ddist = if is_cross_staff && note.header.staffn != top_staffn {
                    note.yd + height_diff_ddist
                } else {
                    note.yd
                };
                let ystem_y = d2r_sum(staff_ctx.staff_top, ystem_ddist);
                let note_yd_y = d2r_sum(staff_ctx.staff_top, yd_ddist);

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
    // OG Beam.cp lines 1693-1700: For downstems, ystem is the BOTTOM of the beam;
    // for upstems, ystem is the TOP. PS_Beam always expects TOP edge Y coordinates.
    // OG Draw1Beam line 1921: if (!topEdge) { yl -= beamThick; }
    let first = &points[0];
    let last = &points[points.len() - 1];
    let y0_top = if stem_up {
        first.ystem_y
    } else {
        first.ystem_y - beam_thickness
    };
    let y1_top = if stem_up {
        last.ystem_y
    } else {
        last.ystem_y - beam_thickness
    };
    renderer.beam(
        first.x,
        y0_top,
        last.x,
        y1_top,
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
                // Convert to top edge if downstems
                let y0_base = if stem_up {
                    p0.ystem_y
                } else {
                    p0.ystem_y - beam_thickness
                };
                let y1_base = if stem_up {
                    p1.ystem_y
                } else {
                    p1.ystem_y - beam_thickness
                };
                renderer.beam(
                    p0.x,
                    y0_base + y_offset,
                    p1.x,
                    y1_base + y_offset,
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

                // Convert to top edge if downstems
                let y_base = if stem_up {
                    p.ystem_y
                } else {
                    p.ystem_y - beam_thickness
                };
                let fy_base = if stem_up {
                    frac_y
                } else {
                    frac_y - beam_thickness
                };
                renderer.beam(
                    p.x,
                    y_base + y_offset,
                    frac_x,
                    fy_base + y_offset,
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

/// Draw a BeamSet for grace notes (beamset.grace == true).
///
/// Grace note beams are similar to regular beams but use 70% scaling.
/// Reference: GRBeam.cp DrawGRBEAMSET(), Beam.cp:1693-1700
fn draw_grace_beamset(
    score: &InterpretedScore,
    beamset: &crate::obj_types::BeamSet,
    notebeam_list: &[crate::obj_types::ANoteBeam],
    _obj: &InterpretedObject,
    ctx: &ContextState,
    renderer: &mut dyn MusicRenderer,
) {
    // Resolve each ANoteBeam to (x, ystem_y) position using grace notes
    #[allow(dead_code)]
    struct BeamPoint {
        x: f32,
        ystem_y: f32,
        note_yd: f32,
        l_dur: i8,
        startend: i8,
    }

    let mut points: Vec<BeamPoint> = Vec::new();

    for notebeam in notebeam_list {
        // For grace notes, find the GRSYNC object at bp_sync
        let grsync_obj = match score.objects.iter().find(|o| o.index == notebeam.bp_sync) {
            Some(o) => o,
            None => continue,
        };

        // Get the staff context for this beamset
        let staff_ctx = match ctx.get(beamset.ext_header.staffn) {
            Some(c) => c,
            None => continue,
        };

        // Find the grace note(s) in this GRSYNC for the beamset voice
        if let Some(grnote_list) = score.grnotes.get(&grsync_obj.header.first_sub_obj) {
            let matching: Vec<&_> = grnote_list
                .iter()
                .filter(|n| {
                    n.beamed
                        && n.voice == beamset.voice
                        && n.header.staffn == beamset.ext_header.staffn
                })
                .collect();

            // Use the note with a real stem (ystem != yd) if available
            if let Some(note) = matching
                .iter()
                .find(|n| n.ystem != n.yd)
                .or(matching.first())
                .copied()
            {
                let lnspace = lnspace_for_staff(staff_ctx.staff_height, staff_ctx.staff_lines);

                // Stem direction: stem_down = (ystem > yd)
                let stem_down = note.ystem > note.yd;

                // Grace note head width is 70% of regular (Defs.h GRACESIZE macro = 7*size/10)
                let head_width = 1.125 * lnspace * 0.7;

                // X position: same calculation as regular beams
                let note_x = d2r_sum3(staff_ctx.measure_left, grsync_obj.header.xd, note.xd);
                let stem_x = if stem_down {
                    note_x
                } else {
                    note_x + head_width
                };

                // Y position at stem endpoint and notehead
                let ystem_y = d2r_sum(staff_ctx.staff_top, note.ystem);
                let note_yd_y = d2r_sum(staff_ctx.staff_top, note.yd);

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

    // Beam thickness for grace notes: 70% of regular thickness (GRBeam.cp:882)
    let staff_ctx = match ctx.get(beamset.ext_header.staffn) {
        Some(c) => c,
        None => return,
    };
    let lnspace = if staff_ctx.staff_lines > 1 {
        ddist_to_render(staff_ctx.staff_height) / (staff_ctx.staff_lines as f32 - 1.0)
    } else {
        8.0
    };
    let beam_thickness = lnspace * 0.5 * 0.7; // 70% of regular beam thickness

    // Determine stem direction
    let stem_up = points[0].ystem_y < points[0].note_yd;

    let beam_gap = (lnspace * 0.25) * 0.7; // Gap between primary and secondary beams (70% scaled)

    // Draw primary beam connecting first to last point
    let first = &points[0];
    let last = &points[points.len() - 1];
    let y0_top = if stem_up {
        first.ystem_y
    } else {
        first.ystem_y - beam_thickness
    };
    let y1_top = if stem_up {
        last.ystem_y
    } else {
        last.ystem_y - beam_thickness
    };
    renderer.beam(
        first.x,
        y0_top,
        last.x,
        y1_top,
        beam_thickness,
        stem_up,
        stem_up,
    );

    // Helper function to draw secondary or tertiary beams at a given offset level
    let mut draw_secondary_tertiary_beams = |min_dur: i8, y_level: i32| {
        let mut i = 0;
        while i < points.len() {
            if points[i].l_dur >= min_dur {
                // Find extent of consecutive notes at this level
                let start = i;
                while i < points.len() && points[i].l_dur >= min_dur {
                    i += 1;
                }
                let end = i - 1;

                if start < end {
                    // Draw beam segment at this level
                    let y_offset = if stem_up {
                        (y_level as f32) * (beam_thickness + beam_gap)
                    } else {
                        -((y_level as f32) * (beam_thickness + beam_gap))
                    };

                    // Interpolate Y positions
                    let p0 = &points[start];
                    let p1 = &points[end];
                    let y0_base = if stem_up {
                        p0.ystem_y
                    } else {
                        p0.ystem_y - beam_thickness
                    };
                    let y1_base = if stem_up {
                        p1.ystem_y
                    } else {
                        p1.ystem_y - beam_thickness
                    };
                    renderer.beam(
                        p0.x,
                        y0_base + y_offset,
                        p1.x,
                        y1_base + y_offset,
                        beam_thickness,
                        stem_up,
                        stem_up,
                    );
                } else {
                    // Single note at this level — draw fractional beam
                    let p = &points[start];
                    let (frac_x, frac_y) = if start > 0 {
                        let prev = &points[start - 1];
                        let frac_len = (p.x - prev.x).min(lnspace * 0.7);
                        (p.x - frac_len, p.ystem_y)
                    } else if start + 1 < points.len() {
                        let next = &points[start + 1];
                        let frac_len = (next.x - p.x).min(lnspace * 0.7);
                        (p.x + frac_len, p.ystem_y)
                    } else {
                        i += 1;
                        continue;
                    };

                    let y_offset = if stem_up {
                        (y_level as f32) * (beam_thickness + beam_gap)
                    } else {
                        -((y_level as f32) * (beam_thickness + beam_gap))
                    };

                    let y_base = if stem_up {
                        p.ystem_y
                    } else {
                        p.ystem_y - beam_thickness
                    };
                    let fy_base = if stem_up {
                        frac_y
                    } else {
                        frac_y - beam_thickness
                    };
                    renderer.beam(
                        p.x,
                        y_base + y_offset,
                        frac_x,
                        fy_base + y_offset,
                        beam_thickness,
                        stem_up,
                        stem_up,
                    );
                }
            } else {
                i += 1;
            }
        }
    };

    // Draw secondary beams for 16th+ notes (SIXTEENTH_L_DUR = 6)
    draw_secondary_tertiary_beams(SIXTEENTH_L_DUR, 1);

    // Draw tertiary beams for 32nd+ notes (THIRTY2ND_L_DUR = 7)
    draw_secondary_tertiary_beams(THIRTY2ND_L_DUR, 2);
}
