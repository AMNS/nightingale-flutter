//! High-level score rendering — port of DrawHighLevel.cp.
//!
//! Contains `render_score()`, the main entry point that walks the object list
//! and dispatches to type-specific drawing functions.
//!
//! Reference: Nightingale/src/CFilesBoth/DrawHighLevel.cp

use crate::context::ContextState;
use crate::ngl::interpret::{InterpretedScore, ObjData};
use crate::render::MusicRenderer;

use super::draw_beam::draw_beamset;
use super::draw_nrgr::{collect_slur_endpoints, collect_tie_endpoints, draw_grsync, draw_sync};
use super::draw_object::{
    draw_clef, draw_connect, draw_dynamic, draw_ending, draw_graphic, draw_keysig, draw_measure,
    draw_ottava, draw_page_number, draw_part_names, draw_slur, draw_slurs_from_endpoints,
    draw_staff, draw_tempo, draw_ties, draw_timesig,
};
use super::draw_tuplet::draw_tuplet;
use super::helpers::{count_staves, first_staff_lnspace, TieEndpoint};

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

    // Compute line widths from the first staff's lnSpace, matching OG defaults.
    // OG stores percentages in config struct and computes: width = config_% * lnSpace / 100
    // Defaults: STAFFLW_DFLT=8, LEDGERLW_DFLT=13, STEMLW_DFLT=8, BARLINELW_DFLT=10
    // Reference: Initialize.cp:952-955, PS_Stdio.cp PS_Recompute() lines 2023-2048
    let lnspace = first_staff_lnspace(score);
    let staff_lw = 0.08 * lnspace; // 8% of lnSpace
    let ledger_lw = 0.13 * lnspace; // 13% of lnSpace
    let stem_lw = 0.08 * lnspace; // 8% of lnSpace
    let bar_lw = 0.10 * lnspace; // 10% of lnSpace
    renderer.set_widths(staff_lw, ledger_lw, stem_lw, bar_lw);

    let mut ctx = ContextState::new(num_staves);

    // Collect tie endpoints during the walk for post-draw tie rendering.
    // tie_starts: notes with tied_r (start of tie arc)
    // tie_ends: notes with tied_l (end of tie arc)
    let mut tie_starts: Vec<TieEndpoint> = Vec::new();
    let mut tie_ends: Vec<TieEndpoint> = Vec::new();

    // Collect slur endpoints for Notelist scores (NGL slurs render via draw_slur).
    // slur_starts: notes with slurred_r (start of slur arc)
    // slur_ends: notes with slurred_l (end of slur arc)
    let mut slur_starts: Vec<TieEndpoint> = Vec::new();
    let mut slur_ends: Vec<TieEndpoint> = Vec::new();

    // Page management for multi-page NGL scores.
    // NGL files store system_rect coordinates relative to the page, so systems
    // on different pages have overlapping Y ranges. We must emit page breaks
    // when PAGE objects are encountered in the walk.
    // Reference: PS_Stdio.cp, PS_NewPage() / PS_EndPage()
    let mut current_page: i32 = -1; // -1 = no page started yet

    // Track whether the next Measure object is the first in its system.
    // Used by draw_measure to decide measure number placement.
    // Reference: DrawUtils.cp:2157 — FirstMeasInSys()
    let mut first_meas_in_system = false;

    for obj in score.walk() {
        // Update context BEFORE drawing (matches C++ pipeline)
        ctx.update_from_object(obj, score);

        match &obj.data {
            // PAGE objects trigger page breaks for multi-page NGL scores.
            // Notelist-generated scores have no PAGE objects, so this is a no-op.
            ObjData::Page(page) => {
                let page_num = page.sheet_num as i32;
                if current_page >= 0 {
                    // Draw any accumulated ties/slurs before ending the page
                    draw_ties(&tie_starts, &tie_ends, renderer);
                    draw_slurs_from_endpoints(&slur_starts, &slur_ends, renderer);
                    tie_starts.clear();
                    tie_ends.clear();
                    slur_starts.clear();
                    slur_ends.clear();
                    renderer.end_page();
                }
                renderer.begin_page((page_num + 1) as u32);
                current_page = page_num;

                // Draw page number (skips page 1 by convention)
                // Reference: DrawObject.cp, DrawPAGE(), lines 249-257
                draw_page_number(score, page.sheet_num, renderer);
            }
            // SYSTEM boundaries: draw ties/slurs for the current system, then clear.
            // This prevents ties/slurs from matching across distant systems and
            // producing wild diagonal lines.
            ObjData::System(_) => {
                if !tie_starts.is_empty() || !tie_ends.is_empty() {
                    draw_ties(&tie_starts, &tie_ends, renderer);
                    tie_starts.clear();
                    tie_ends.clear();
                }
                if !slur_starts.is_empty() || !slur_ends.is_empty() {
                    draw_slurs_from_endpoints(&slur_starts, &slur_ends, renderer);
                    slur_starts.clear();
                    slur_ends.clear();
                }
            }
            ObjData::Staff(_) => {
                draw_staff(score, obj, &ctx, renderer);
                // Draw part names once per system, at the Staff object.
                // Get system_num from the first staff's context.
                // Reference: DrawObject.cp, DrawSTAFF(), lines 636-651
                let system_num = ctx.get(1).map_or(0, |c| c.system_num);
                draw_part_names(score, &ctx, renderer, system_num);
                // The next Measure after a Staff is the first in the system.
                first_meas_in_system = true;
            }
            ObjData::Measure(_) => {
                draw_measure(score, obj, &ctx, renderer, first_meas_in_system);
                first_meas_in_system = false;
            }
            ObjData::Sync(_) => {
                draw_sync(score, obj, &ctx, renderer);
                collect_tie_endpoints(score, obj, &ctx, &mut tie_starts, &mut tie_ends);
                collect_slur_endpoints(score, obj, &ctx, &mut slur_starts, &mut slur_ends);
            }
            ObjData::Connect(_) => draw_connect(score, obj, &ctx, renderer),
            ObjData::Clef(_) => draw_clef(score, obj, &ctx, renderer),
            ObjData::KeySig(_) => draw_keysig(score, obj, &ctx, renderer),
            ObjData::TimeSig(_) => draw_timesig(score, obj, &ctx, renderer),
            ObjData::BeamSet(_) => draw_beamset(score, obj, &ctx, renderer),
            ObjData::Tuplet(_) => draw_tuplet(score, obj, &ctx, renderer),
            ObjData::Slur(_) => draw_slur(score, obj, &ctx, renderer),
            ObjData::Dynamic(_) => draw_dynamic(score, obj, &ctx, renderer),
            ObjData::Graphic(_) => draw_graphic(score, obj, &ctx, renderer),
            ObjData::Tempo(_) => draw_tempo(score, obj, &ctx, renderer),
            ObjData::Ending(_) => draw_ending(score, obj, &ctx, renderer),
            ObjData::Ottava(_) => draw_ottava(score, obj, &ctx, renderer),
            ObjData::GrSync(_) => draw_grsync(score, obj, &ctx, renderer),
            _ => {}
        }
    }

    // Draw ties and slurs after all objects (so they layer on top of the last page)
    draw_ties(&tie_starts, &tie_ends, renderer);
    draw_slurs_from_endpoints(&slur_starts, &slur_ends, renderer);

    // End the last page if we were in multi-page mode
    if current_page >= 0 {
        renderer.end_page();
    }
}
