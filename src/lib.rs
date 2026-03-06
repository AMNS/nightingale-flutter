//! Nightingale music notation engine — Rust port.
//!
//! Ported from the C++ codebase at <https://github.com/AMNS/Nightingale>
//! (branch: `develop`, commit 969e320).
//!
//! This crate provides:
//! - Binary .ngl file reading (N103/N105 formats)
//! - Score data model (object list with typed heaps)
//! - Engraving algorithms (beams, slurs, spacing, etc.)
//! - Platform-agnostic rendering commands
//!
//! See CLAUDE.md for architecture decisions and porting plan.
//! See PROGRESS.md for current status and next steps.

pub mod basic_types;
pub mod beam;
pub mod comparison;
pub mod context;
pub mod defs;
pub mod doc_types;
pub mod draw;
pub mod duration;
pub mod layout;
pub mod limits;
pub mod music_font;
pub mod musicxml;
pub mod ngl;
pub mod notelist;
pub mod obj_types;
pub mod objects;
pub mod og_render;
pub mod pitch_utils;
pub mod render;
pub mod space_time;
pub mod utility;
