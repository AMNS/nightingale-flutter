//! SMuFL (Standard Music Font Layout) metadata loader
//!
//! Provides access to Bravura font metadata including:
//! - Engraving defaults (line thicknesses, spacing)
//! - Glyph bounding boxes and advance widths
//! - Glyph anchors (for connecting elements)
//!
//! Source: https://github.com/steinbergmedia/bravura
//! Spec: https://w3c.github.io/smufl/latest/specification/font-specific-metadata.html
//!
//! ## Current Status: SKELETON ONLY
//!
//! This module provides the data structures and loading infrastructure but does NOT
//! yet integrate with the rendering pipeline. Integration requires:
//!
//! 1. **Add serde_json dependency** to Cargo.toml
//! 2. **Load metadata at startup** (once per process)
//! 3. **Convert staff-space values to points** dynamically based on staff height
//! 4. **Update renderer state** to use metadata values instead of hardcoded defaults
//! 5. **Test visual output** to validate improvements
//!
//! ## Current Hardcoded Values (src/render/pdf_renderer.rs:66-69)
//!
//! ```ignore
//! staff_line_width: 0.4,    // OG default 8% of lnSpace ≈ 0.48
//! ledger_line_width: 0.64,  // PS_Stdio.cp default
//! stem_width: 0.8,          // PS_Stdio.cp default
//! bar_line_width: 1.0,      // PS_Stdio.cp default
//! ```
//!
//! ## Bravura Metadata Values (staff spaces)
//!
//! From assets/fonts/bravura_metadata.json:
//! ```json
//! "engravingDefaults": {
//!   "staffLineThickness": 0.13,
//!   "legerLineThickness": 0.16,
//!   "stemThickness": 0.12,
//!   "beamThickness": 0.5,
//!   "thinBarlineThickness": 0.16,
//!   "thickBarlineThickness": 0.5,
//!   "slurEndpointThickness": 0.1,
//!   "slurMidpointThickness": 0.22,
//!   ...
//! }
//! ```
//!
//! ## Integration Strategy
//!
//! The challenge is unit conversion:
//! - **Current approach**: Absolute line widths in points (device-independent)
//! - **SMuFL approach**: Relative line widths in staff spaces
//!
//! **Solution**: Compute line widths dynamically based on staff height:
//! ```ignore
//! let staff_height_pt = staff_ctx.staff_height as f32 / 16.0; // DDIST → pt
//! let staff_space = staff_height_pt / 4.0; // 4 spaces per 5-line staff
//! let staff_line_width = metadata.staff_line_thickness * staff_space;
//! renderer.set_widths(staff_line_width, ledger_width, stem_width, bar_width);
//! ```
//!
//! This requires calling `set_widths()` whenever staff height changes (currently
//! only at staff object rendering time).

use std::collections::HashMap;
use std::path::Path;

/// SMuFL font metadata (top-level structure)
///
/// TODO: Add #[derive(Deserialize)] once serde_json is added to Cargo.toml
#[allow(dead_code)]
pub struct SmuflMetadata {
    pub font_name: String,
    pub font_version: String,
    pub engraving_defaults: EngravingDefaults,
    pub glyph_advance_widths: HashMap<String, f32>,
    pub glyph_bboxes: HashMap<String, BBox>,
    pub glyphs_with_anchors: HashMap<String, GlyphAnchors>,
}

/// Engraving defaults (line thicknesses, spacing, etc.)
///
/// All values are in staff spaces (floating point).
/// To convert to points: `value_pt = value_spaces * staff_space_pt`
/// where `staff_space_pt = staff_height_pt / 4.0` for a 5-line staff.
///
/// TODO: Add #[derive(Deserialize)] once serde_json is added
#[allow(dead_code)]
pub struct EngravingDefaults {
    // Line thicknesses
    pub staff_line_thickness: f32,
    pub leger_line_thickness: f32,
    pub stem_thickness: f32,
    pub beam_thickness: f32,
    pub thin_barline_thickness: f32,
    pub thick_barline_thickness: f32,

    // Slur/tie thicknesses
    pub slur_endpoint_thickness: f32,
    pub slur_midpoint_thickness: f32,
    pub tie_endpoint_thickness: f32,
    pub tie_midpoint_thickness: f32,

    // Spacing
    pub beam_spacing: f32,
    pub leger_line_extension: f32,
    pub barline_separation: f32,
    // TODO: Add remaining fields as needed:
    // - hairpinThickness
    // - octaveLineThickness
    // - tupletBracketThickness
    // - textFontFamily
    // - etc.
}

/// Glyph bounding box (in staff spaces)
#[allow(dead_code)]
pub struct BBox {
    pub ne: (f32, f32), // Northeast (top-right)
    pub sw: (f32, f32), // Southwest (bottom-left)
}

/// Glyph anchor points (for connecting stems, etc.)
#[allow(dead_code)]
pub struct GlyphAnchors {
    pub anchors: HashMap<String, (f32, f32)>,
}

/// Load SMuFL metadata from JSON file
///
/// TODO: Implement once serde_json is added to Cargo.toml
///
/// Example usage (future):
/// ```ignore
/// let metadata = SmuflMetadata::load("assets/fonts/bravura_metadata.json")?;
/// ```
#[allow(dead_code)]
impl SmuflMetadata {
    pub fn load<P: AsRef<Path>>(_path: P) -> Result<Self, String> {
        // TODO: Implement JSON parsing with serde_json
        Err("SMuFL metadata loading not yet implemented (needs serde_json)".to_string())
    }

    /// Get line width in points for a given staff height
    ///
    /// Converts SMuFL staff-space values to absolute points.
    ///
    /// # Arguments
    /// * `staff_height_ddist` - Staff height in DDIST units (1/16 point)
    /// * `thickness_spaces` - Line thickness in staff spaces (from metadata)
    ///
    /// # Returns
    /// Line width in points
    ///
    /// # Example
    /// ```ignore
    /// let staff_height = 384; // DDIST (typical 5-line staff)
    /// let staff_line_width = metadata.line_width_pt(
    ///     staff_height,
    ///     metadata.engraving_defaults.staff_line_thickness
    /// );
    /// ```
    #[allow(dead_code)]
    pub fn line_width_pt(&self, staff_height_ddist: i16, thickness_spaces: f32) -> f32 {
        let staff_height_pt = staff_height_ddist as f32 / 16.0; // DDIST → pt
        let staff_space = staff_height_pt / 4.0; // 4 spaces per 5-line staff
        thickness_spaces * staff_space
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "SMuFL metadata loading not yet implemented"]
    fn test_load_bravura_metadata() {
        let metadata =
            SmuflMetadata::load("assets/fonts/bravura_metadata.json").expect("Failed to load");
        assert_eq!(metadata.font_name, "Bravura");
        assert!(metadata.engraving_defaults.staff_line_thickness > 0.0);
    }

    #[test]
    fn test_line_width_conversion() {
        // Mock metadata with known values
        let staff_height_ddist = 384; // 24pt staff (typical 5-line staff)
        let thickness_spaces = 0.13; // Bravura staffLineThickness

        // Staff height: 384 DDIST = 24 pt
        // Staff space: 24 pt / 4 = 6 pt
        // Line width: 0.13 * 6 = 0.78 pt
        let expected_width = 0.78;

        // Use a minimal struct for testing
        let metadata = SmuflMetadata {
            font_name: "Test".to_string(),
            font_version: "1.0".to_string(),
            engraving_defaults: EngravingDefaults {
                staff_line_thickness: thickness_spaces,
                leger_line_thickness: 0.16,
                stem_thickness: 0.12,
                beam_thickness: 0.5,
                thin_barline_thickness: 0.16,
                thick_barline_thickness: 0.5,
                slur_endpoint_thickness: 0.1,
                slur_midpoint_thickness: 0.22,
                tie_endpoint_thickness: 0.1,
                tie_midpoint_thickness: 0.22,
                beam_spacing: 0.25,
                leger_line_extension: 0.4,
                barline_separation: 0.4,
            },
            glyph_advance_widths: HashMap::new(),
            glyph_bboxes: HashMap::new(),
            glyphs_with_anchors: HashMap::new(),
        };

        let actual_width = metadata.line_width_pt(staff_height_ddist, thickness_spaces);
        assert!((actual_width - expected_width).abs() < 0.01);
    }
}
