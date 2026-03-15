# MusicXML Round-Trip Findings

**Date**: 2026-03-14
**ROADMAP Task**: Tier 1 #2 — MusicXML Round-Trip Stability

## Test Method

Added `ngl_musicxml_roundtrip_visual_test()` in `tests/musicxml_pipeline.rs` to test:

```
NGL → render PDF (original)
  ↓
NGL → export MusicXML → import → layout → render PDF (roundtrip)
  ↓
Compare original vs roundtrip PDFs
```

## Results Summary

| Fixture | Notes | Measures | PDF Size | Status |
|---------|-------|----------|----------|--------|
| tc_ich_bin_ja | 651 → 665 (+14) | 54 → 86 (+32) | 646K → 676K (+5%) | ⚠️ Note gain |
| tc_05 | 105 → 105 (stable) | 5 → 9 (+4) | 523K → 523K (0%) | ✓ Stable |
| 01_me_and_lucy | 3453 → 3453 (stable) | 102 → 160 (+58) | 1.2M → 1.1M (-8%) | ✓ Stable |

## Key Findings

### 1. Measure Count Inflation

**All fixtures show significant measure count increases** after roundtrip:
- tc_ich_bin_ja: +32 measures (60% increase)
- tc_05: +4 measures (80% increase)
- 01_me_and_lucy: +58 measures (57% increase)

**Likely cause**: `layout_score()` may be creating extra MEASURE objects when applied to imported MusicXML scores. The original NGL files have pre-existing measure layout from OG Nightingale.

**Impact**: Structural representation differs, but PDF sizes remain similar (within 10%), suggesting visual rendering is largely equivalent.

### 2. Note Count Stability

**2 out of 3 fixtures have perfect note count stability** (tc_05, 01_me_and_lucy).

**tc_ich_bin_ja shows +14 notes** (2% increase):
- Possible causes:
  - Whole-measure rests synthesized during export (not present in sparse NGL data)
  - Grace notes or tuplet notes counted differently
  - Import creating explicit rest notes for empty beats

**Requires investigation**: Compare `tc_ich_bin_ja_exported.musicxml` structure to identify source of extra notes.

### 3. PDF Size Parity

**PDF sizes are very similar** across all roundtrips:
- tc_05: Identical (0% change)
- tc_ich_bin_ja: +5% (likely from +14 notes)
- 01_me_and_lucy: -8% (possibly tighter spacing)

**Conclusion**: Visual rendering appears stable despite structural measure count differences.

## Recommendations

### High Priority

1. **Investigate measure count inflation**
   - Examine `layout_score()` behavior on imported MusicXML
   - Compare with NGL pipeline (which preserves OG layout)
   - May need separate layout logic for MusicXML import vs NGL

2. **Analyze tc_ich_bin_ja +14 notes**
   - Parse `tc_ich_bin_ja_exported.musicxml`
   - Identify which elements became extra notes on re-import
   - Fix export/import pipeline to preserve exact note counts

### Medium Priority

3. **Visual regression test for roundtrips**
   - Convert PDFs to PNGs (use existing `pdf_to_png()` from tests/common)
   - Generate diff images (use `compare_images_and_diff()`)
   - Automate visual comparison (similar to QA Compare workflow)

4. **Add round-trip tests for all 26 NGL fixtures**
   - Expand `ngl_musicxml_roundtrip_visual_test()` to cover all fixtures
   - Document any fixtures with >10% note count changes
   - Track visual diff percentages

### Low Priority

5. **Dorico/MuseScore interop testing**
   - Export NGL → MusicXML → import to Dorico → re-export → import to Nightingale
   - Test with MuseScore similarly
   - Document incompatibilities

## Test Output Location

PDFs written to: `test-output/musicxml_pipeline/ngl_roundtrip/`

Files for each fixture:
- `{name}_original.pdf` — original NGL rendering
- `{name}_exported.musicxml` — NGL → MusicXML export
- `{name}_roundtrip.pdf` — re-imported rendering

## Next Steps

1. Run visual diff analysis on the generated PDFs
2. Investigate measure count inflation root cause
3. Fix tc_ich_bin_ja note count discrepancy
4. Expand test to all NGL fixtures
5. Add automated visual regression checks
