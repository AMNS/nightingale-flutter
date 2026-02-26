//! Beam drawing — port of DrawBeam.cp.
//!
//! Draws BeamSet objects: primary and secondary beam segments connecting
//! notes in a beam group.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawBeam.cp

use crate::context::ContextState;
use crate::defs::*;
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

        // Find the stem-bearing beamed note in this sync for the beamset voice.
        // In chords, multiple notes share beamed=true but only the "far" note
        // (farthest from the beam) has a meaningful ystem != yd. Prefer that note
        // so beam endpoints connect to actual stem tips.
        if let Some(anote_list) = score.notes.get(&sync_obj.header.first_sub_obj) {
            let matching: Vec<&_> = anote_list
                .iter()
                .filter(|n| {
                    n.beamed
                        && n.voice == beamset.voice
                        && n.header.staffn == beamset.ext_header.staffn
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
                let note_x = d2r_sum3(staff_ctx.measure_left, sync_obj.header.xd, note.xd);
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
