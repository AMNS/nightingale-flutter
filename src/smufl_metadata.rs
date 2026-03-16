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

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// SMuFL font metadata (top-level structure)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmuflMetadata {
    #[serde(rename = "fontName")]
    pub font_name: String,
    #[serde(rename = "fontVersion")]
    pub font_version: f32,
    pub engraving_defaults: EngravingDefaults,
    #[serde(default)]
    pub glyph_advance_widths: HashMap<String, f32>,
    #[serde(default)]
    pub glyph_bboxes: HashMap<String, BBox>,
    #[serde(default)]
    pub glyphs_with_anchors: HashMap<String, HashMap<String, (f32, f32)>>,
}

/// Engraving defaults (line thicknesses, spacing, etc.)
///
/// All values are in staff spaces (floating point).
/// To convert to points: `value_pt = value_spaces * staff_space_pt`
/// where `staff_space_pt = staff_height_pt / 4.0` for a 5-line staff.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Deserialize)]
pub struct BBox {
    #[serde(rename = "bBoxNE")]
    pub ne: (f32, f32), // Northeast (top-right)
    #[serde(rename = "bBoxSW")]
    pub sw: (f32, f32), // Southwest (bottom-left)
}

/// Load SMuFL metadata from JSON file
///
/// Example usage:
/// ```ignore
/// let metadata = SmuflMetadata::load("assets/fonts/bravura_metadata.json")?;
/// ```
#[allow(dead_code)]
impl SmuflMetadata {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let json = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
        let metadata: SmuflMetadata =
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse JSON: {}", e))?;
        Ok(metadata)
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
    fn test_load_bravura_metadata() {
        let metadata =
            SmuflMetadata::load("assets/fonts/bravura_metadata.json").expect("Failed to load");
        assert_eq!(metadata.font_name, "Bravura");
        assert!((metadata.font_version - 1.392).abs() < 0.001); // Compare f32 with tolerance

        // Verify key engraving defaults
        let defaults = &metadata.engraving_defaults;
        assert_eq!(defaults.staff_line_thickness, 0.13);
        assert_eq!(defaults.leger_line_thickness, 0.16);
        assert_eq!(defaults.stem_thickness, 0.12);
        assert_eq!(defaults.beam_thickness, 0.5);
        assert_eq!(defaults.thin_barline_thickness, 0.16);
        assert_eq!(defaults.thick_barline_thickness, 0.5);
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
            font_version: 1.0,
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
