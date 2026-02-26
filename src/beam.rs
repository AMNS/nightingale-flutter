//! Beam algorithms — port of Beam.cp.
//!
//! Beam slope calculation and stem adjustment for beamed note groups.
//! Used by the score builder (notelist/to_score.rs and future NGL pipeline).
//!
//! Reference: Nightingale/src/CFilesBoth/Beam.cp

use crate::basic_types::Ddist;

/// Information about a note in a beam group, used for slope calculation.
#[derive(Debug, Clone)]
pub struct BeamNoteInfo {
    pub sync_xd: Ddist,
    pub note_yd: Ddist,
    pub note_ystem: Ddist,
    /// Index or link to the sync object (caller-defined semantics).
    pub sync_id: u16,
}

/// Compute adjusted beam slope and return new ystem values for each note.
///
/// Port of GetBeamEndYStems (Beam.cp:181-235) + FixSyncInBeamset (Beam.cp:272-299).
///
/// Algorithm:
/// 1. Get CalcYStem for first and last notes (their current ystem values)
/// 2. Compute natural slope = lastYstem - firstYstem
/// 3. Apply `rel_beam_slope` percentage to reduce slope
/// 4. Recompute first/last ystem based on the "base" note (the one whose
///    CalcYStem is the extreme — highest for stems up, lowest for stems down)
/// 5. Linearly interpolate all intermediate stems
///
/// Returns a Vec of new ystem values, one per input note, or None if the beam
/// is degenerate (fewer than 2 notes or zero horizontal span).
pub fn compute_beam_slope(infos: &[BeamNoteInfo], rel_beam_slope: i16) -> Option<Vec<Ddist>> {
    if infos.len() < 2 || rel_beam_slope <= 0 {
        return None;
    }

    // Determine stem direction from first note
    let stem_down = infos[0].note_ystem > infos[0].note_yd;

    // Natural CalcYStem endpoints (already computed)
    let first_ystem = infos[0].note_ystem;
    let last_ystem = infos[infos.len() - 1].note_ystem;

    // endDiff: vertical difference between endpoints
    // (Beam.cp:208: fEndDiff = (double)(firstystem1 - lastystem1))
    let end_diff = first_ystem - last_ystem;

    // Apply reduced slope (Beam.cp:214: fSlope = fEndDiff * relBeamSlope / 100)
    let slope = end_diff as f32 * rel_beam_slope as f32 / 100.0;

    // Find the "base" note — the one whose CalcYStem is the extreme.
    // For stems up: base = note with smallest (highest) ystem
    // For stems down: base = note with largest (lowest) ystem
    // (Beam.cp:191-196)
    let base_idx = if stem_down {
        infos
            .iter()
            .enumerate()
            .max_by_key(|(_, info)| info.note_ystem)
            .map(|(i, _)| i)
            .unwrap_or(0)
    } else {
        infos
            .iter()
            .enumerate()
            .min_by_key(|(_, info)| info.note_ystem)
            .map(|(i, _)| i)
            .unwrap_or(0)
    };

    let base_ystem = infos[base_idx].note_ystem;

    // Compute horizontal positions
    let first_xd = infos[0].sync_xd;
    let last_xd = infos[infos.len() - 1].sync_xd;
    let beam_length = (last_xd - first_xd) as f32;

    if beam_length.abs() < 1.0 {
        return None; // Degenerate beam
    }

    // Compute offset from base note to first note
    // (Beam.cp:220-224)
    let base_xd = infos[base_idx].sync_xd;
    let base_to_first = (first_xd - base_xd) as f32;
    let base_frac = base_to_first / beam_length;
    let base_offset = slope * base_frac;

    // First and last ystem based on base note position + slope
    // (Beam.cp:226-227)
    let new_first_ystem = base_ystem as f32 + base_offset;
    let new_last_ystem = new_first_ystem - slope;

    // Interpolate all stems linearly along the beam line
    let mut result = Vec::with_capacity(infos.len());
    for info in infos {
        let t = if beam_length.abs() > 0.0 {
            (info.sync_xd - first_xd) as f32 / beam_length
        } else {
            0.0
        };
        let interpolated = new_first_ystem + t * (new_last_ystem - new_first_ystem);
        result.push(interpolated.round() as Ddist);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_beam_slope_flat() {
        // All notes at the same height → slope should be zero, all ystem identical
        let infos = vec![
            BeamNoteInfo {
                sync_xd: 100,
                note_yd: 32,
                note_ystem: 10,
                sync_id: 1,
            },
            BeamNoteInfo {
                sync_xd: 200,
                note_yd: 32,
                note_ystem: 10,
                sync_id: 2,
            },
            BeamNoteInfo {
                sync_xd: 300,
                note_yd: 32,
                note_ystem: 10,
                sync_id: 3,
            },
        ];
        let result = compute_beam_slope(&infos, 33).unwrap();
        assert_eq!(result.len(), 3);
        // All should be 10 (no slope)
        assert_eq!(result[0], 10);
        assert_eq!(result[1], 10);
        assert_eq!(result[2], 10);
    }

    #[test]
    fn test_compute_beam_slope_ascending() {
        // Ascending notes (ystem decreasing) — stems up
        let infos = vec![
            BeamNoteInfo {
                sync_xd: 100,
                note_yd: 40,
                note_ystem: 10,
                sync_id: 1,
            },
            BeamNoteInfo {
                sync_xd: 300,
                note_yd: 20,
                note_ystem: -10,
                sync_id: 2,
            },
        ];
        let result = compute_beam_slope(&infos, 33).unwrap();
        assert_eq!(result.len(), 2);
        // Slope reduced to 33%, so endpoints should be closer together
        let diff = (result[0] - result[1]).abs();
        assert!(diff < 20, "slope should be reduced from natural 20");
    }

    #[test]
    fn test_compute_beam_slope_degenerate() {
        // Single note → None
        let infos = vec![BeamNoteInfo {
            sync_xd: 100,
            note_yd: 32,
            note_ystem: 10,
            sync_id: 1,
        }];
        assert!(compute_beam_slope(&infos, 33).is_none());
    }
}
