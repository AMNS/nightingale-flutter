//! Score drawing module — renders InterpretedScore through MusicRenderer.
//!
//! This module implements the main score rendering pipeline, porting the C++ drawing
//! functions from DrawHighLevel.cp, DrawObject.cp, and DrawNRGR.cp.
//!
//! # Architecture
//!
//! 1. `score_renderer::render_score()` — walks the object list, calls type-specific draw functions
//! 2. `draw_staff()`, `draw_measure()`, `draw_sync()`, etc. — render individual object types
//! 3. MusicRenderer trait — platform-agnostic rendering backend (PDF, Flutter, etc.)
//!
//! # Coordinate System
//!
//! - All object positions are in DDIST (1/16 point resolution)
//! - Context stores absolute page-relative DDIST coordinates
//! - Before calling MusicRenderer methods, convert to f32 points via `ddist_to_render()`
//!
//! Reference: DrawHighLevel.cp DrawScoreRange() (lines 1145-1289)

pub mod score_renderer;

pub use score_renderer::render_score;
