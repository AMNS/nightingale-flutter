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
use super::draw_nrgr::{collect_tie_endpoints, draw_sync};
use super::draw_object::{
    draw_clef, draw_connect, draw_keysig, draw_measure, draw_staff, draw_ties, draw_timesig,
};
use super::draw_tuplet::draw_tuplet;
use super::helpers::{count_staves, TieEndpoint};

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

    // Page management for multi-page NGL scores.
    // NGL files store system_rect coordinates relative to the page, so systems
    // on different pages have overlapping Y ranges. We must emit page breaks
    // when PAGE objects are encountered in the walk.
    // Reference: PS_Stdio.cp, PS_NewPage() / PS_EndPage()
    let mut current_page: i32 = -1; // -1 = no page started yet

    for obj in score.walk() {
        // Update context BEFORE drawing (matches C++ pipeline)
        ctx.update_from_object(obj, score);

        match &obj.data {
            // PAGE objects trigger page breaks for multi-page NGL scores.
            // Notelist-generated scores have no PAGE objects, so this is a no-op.
            ObjData::Page(page) => {
                let page_num = page.sheet_num as i32;
                if current_page >= 0 {
                    // Draw any accumulated ties before ending the page
                    draw_ties(&tie_starts, &tie_ends, renderer);
                    tie_starts.clear();
                    tie_ends.clear();
                    renderer.end_page();
                }
                renderer.begin_page((page_num + 1) as u32);
                current_page = page_num;
            }
            // SYSTEM boundaries: draw ties for the current system, then clear.
            // This prevents ties from matching across distant systems and
            // producing wild diagonal lines.
            ObjData::System(_) => {
                if !tie_starts.is_empty() || !tie_ends.is_empty() {
                    draw_ties(&tie_starts, &tie_ends, renderer);
                    tie_starts.clear();
                    tie_ends.clear();
                }
            }
            ObjData::Staff(_) => draw_staff(score, obj, &ctx, renderer),
            ObjData::Measure(_) => draw_measure(score, obj, &ctx, renderer),
            ObjData::Sync(_) => {
                draw_sync(score, obj, &ctx, renderer);
                collect_tie_endpoints(score, obj, &ctx, &mut tie_starts, &mut tie_ends);
            }
            ObjData::Connect(_) => draw_connect(score, obj, &ctx, renderer),
            ObjData::Clef(_) => draw_clef(score, obj, &ctx, renderer),
            ObjData::KeySig(_) => draw_keysig(score, obj, &ctx, renderer),
            ObjData::TimeSig(_) => draw_timesig(score, obj, &ctx, renderer),
            ObjData::BeamSet(_) => draw_beamset(score, obj, &ctx, renderer),
            ObjData::Tuplet(_) => draw_tuplet(score, obj, &ctx, renderer),
            // TODO: Dynamic, Tempo, Graphic, Ottava, Ending, etc.
            _ => {}
        }
    }

    // Draw ties after all objects (so ties layer on top of the last page)
    draw_ties(&tie_starts, &tie_ends, renderer);

    // End the last page if we were in multi-page mode
    if current_page >= 0 {
        renderer.end_page();
    }
}
