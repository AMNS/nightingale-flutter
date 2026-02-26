//! Score rendering — backward-compatible re-export shim.
//!
//! The implementation has been split into OG-faithful submodules:
//! - `draw_high_level` ← DrawHighLevel.cp
//! - `draw_object` ← DrawObject.cp
//! - `draw_nrgr` ← DrawNRGR.cp
//! - `draw_utils` ← DrawUtils.cp
//! - `draw_beam` ← DrawBeam.cp
//! - `draw_tuplet` ← Tuplet.cp
//! - `helpers` — shared coordinate helpers
//!
//! This module re-exports `render_score` so existing callers don't break.

pub use super::draw_high_level::render_score;
