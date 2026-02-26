//! Score drawing module — renders InterpretedScore through MusicRenderer.
//!
//! This module implements the main score rendering pipeline, organized to mirror
//! the OG Nightingale C++ source files:
//!
//! - `draw_high_level` ← DrawHighLevel.cp: `render_score()` main loop
//! - `draw_object` ← DrawObject.cp: staff, measure, connect, clef, keysig, timesig, ties
//! - `draw_nrgr` ← DrawNRGR.cp: sync (notes/rests), ledger lines, tie endpoints
//! - `draw_utils` ← DrawUtils.cp: glyph mapping, key signature Y offsets
//! - `draw_beam` ← DrawBeam.cp: beam sets
//! - `draw_tuplet` ← Tuplet.cp: tuplet brackets and numbers
//! - `helpers` — shared coordinate conversion utilities
//!
//! # Coordinate System
//!
//! - All object positions are in DDIST (1/16 point resolution)
//! - Context stores absolute page-relative DDIST coordinates
//! - Before calling MusicRenderer methods, convert to f32 points via `ddist_to_render()`
//!
//! Reference: DrawHighLevel.cp DrawScoreRange() (lines 1145-1289)

pub mod draw_beam;
pub mod draw_high_level;
pub mod draw_nrgr;
pub mod draw_object;
pub mod draw_tuplet;
pub mod draw_utils;
pub mod helpers;

// Keep score_renderer as a backward-compatible re-export shim.
pub mod score_renderer;

pub use score_renderer::render_score;
