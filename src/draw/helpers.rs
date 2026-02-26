//! Shared drawing helpers — coordinate conversion and utilities.
//!
//! Small functions used across multiple draw submodules.

use crate::ngl::interpret::{InterpretedScore, ObjData};
use crate::render::types::{ddist_to_render, ddist_wide_to_render};

/// Safe DDIST addition that widens to i32 then converts to render coords.
/// Prevents overflow when staff_top + offset exceeds i16 range.
#[inline]
pub fn d2r_sum(a: i16, b: i16) -> f32 {
    ddist_wide_to_render(a as i32 + b as i32)
}

/// Safe DDIST three-way addition.
#[inline]
pub fn d2r_sum3(a: i16, b: i16, c: i16) -> f32 {
    ddist_wide_to_render(a as i32 + b as i32 + c as i32)
}

/// Count the number of staves in a score by examining the first Staff object.
///
/// Returns 0 if no Staff object is found.
pub fn count_staves(score: &InterpretedScore) -> usize {
    for obj in score.walk() {
        if let ObjData::Staff(_) = &obj.data {
            if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
                return astaff_list.len();
            }
        }
    }
    0
}

/// Information about a note's rendered position, used for tie matching.
///
/// Collected during the draw pass and matched after all objects are drawn.
/// Each TieEndpoint records the rendered (x, y) of the notehead center,
/// along with identifying info (staff, voice, note_num) and stem direction.
#[derive(Debug, Clone)]
pub struct TieEndpoint {
    /// Rendered X of the notehead origin (left edge of glyph)
    pub x: f32,
    /// Rendered Y of the notehead center
    pub y: f32,
    /// Note width (for endpoint offset computation)
    pub head_width: f32,
    /// True if stem goes down (=> tie curves up above note)
    pub stem_down: bool,
    /// Staff number (for matching)
    pub staff: i8,
    /// Voice number (for matching)
    pub voice: i8,
    /// MIDI note number (for pitch matching)
    pub note_num: u8,
    /// Line spacing at this note's staff
    pub lnspace: f32,
    /// Right edge of this note's staff (for cross-system partial ties)
    pub staff_right: f32,
    /// Left edge of this note's staff (for cross-system partial ties)
    pub staff_left: f32,
}

/// Compute line spacing for a staff context.
///
/// Returns the distance between adjacent staff lines in render coordinates.
#[inline]
pub fn lnspace_for_staff(staff_height: i16, staff_lines: i8) -> f32 {
    if staff_lines > 1 {
        ddist_to_render(staff_height) / (staff_lines as f32 - 1.0)
    } else {
        8.0
    }
}
