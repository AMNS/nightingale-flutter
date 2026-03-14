# Nightingale Engraving: Prioritized Porting Tasks

**Date**: 2026-03-12
**Based on**: OG comparison results + engraving gaps audit

---

## Priority Matrix

| Priority | Criteria | Examples |
|----------|----------|----------|
| **P0 (Critical)** | Visual collisions, unreadable scores | Accidental staggering, stem X positioning |
| **P1 (High)** | Missing common features, obvious errors | Slash notation, composite dynamics |
| **P2 (Medium)** | Polish, refinement, edge cases | Ledger line extension, rare time sigs |
| **P3 (Low)** | Nice-to-have, uncommon features | Guitar chord frames, PICT graphics |

---

## Immediate Targets (P0): Fix Collisions

Based on visual inspection and known code gaps:

### 1. Accidental Staggering in Chords (P0)
**OG Source**: `DrawNRGR.cp` ChkNoteAccs()
**Estimated LOC**: ~100-150 lines
**Impact**: tc_ich_bin_ja (24.7% diff), tc_55_1 (19.9% diff), tc_05 (4.48% diff)
**Visual**: Accidentals in dense chords currently overlap — unreadable
**Status**: PUNT (#[ignore] test in tests/render_score.rs:238)

#### Action Plan
1. Read `DrawNRGR.cp` ChkNoteAccs() — understand staggering algorithm
2. Port to `src/draw/draw_nrgr.rs` as `check_note_accidentals()`
3. Call from `draw_note()` before rendering accidentals
4. Test with tc_05 (has chord w/ accidentals) via Flutter app
5. Un-ignore `test_accidental_staggering_in_chords`

**Expected improvement**: Accidentals in dense chords no longer overlap

---

### 2. Stem X-Position for Chord Seconds (P0)
**OG Source**: `DrawNRGR.cp:1094-1097`
**Estimated LOC**: ~20-30 lines
**Impact**: Any fixture with chord seconds (notes one step apart)
**Visual**: Notes on opposite stem sides may misalign horizontally
**Status**: TODO (notelist/to_score.rs:2103)

#### Action Plan
1. Read `DrawNRGR.cp:1094-1097` — stem X offset logic for seconds
2. Port to `src/utility.rs` or `src/objects.rs` as `stem_x_for_seconds()`
3. Update `draw_note()` in draw_nrgr.rs to call it
4. Test with beamed_grace_notes.ngl (has complex chord voicings)
5. Visual review via Flutter app

**Expected improvement**: Subtle but fixes misalignment in chord-heavy scores

---

### 3. AccXOffset Refinement (P0)
**OG Source**: `DrawNRGR.cp:396-406` AccXOffset()
**Estimated LOC**: ~50 lines
**Impact**: ALL accidentals in ALL fixtures
**Visual**: Accidentals currently use simple offset — may be too close or too far from notehead
**Status**: TODO (notelist/to_score.rs:2102)

#### Action Plan
1. Read `DrawNRGR.cp:396-406` — AccXOffset logic (depends on notehead width, staff context)
2. Port to `src/draw/draw_nrgr.rs` as `acc_x_offset()`
3. Replace current simple offset in `draw_note()`
4. Visual review in Flutter app across all 26 NGL fixtures
5. Check that accidentals are neither too close nor too far from noteheads

**Expected improvement**: Better accidental spacing across all fixtures

---

## High-Priority Features (P1)

### 4. Slash Notation for Percussion (P1)
**OG Source**: TBD (needs investigation)
**Estimated LOC**: ~30-50 lines
**Impact**: Drum/rhythm notation fixtures
**Visual**: Slash noteheads not rendering — shows as rests or missing
**Status**: TODO (draw_utils.rs:104)

#### Action Plan
1. Search OG source for slash notehead rendering (likely DrawNRGR.cp or DrawUtils.cp)
2. Port to `src/draw/draw_utils.rs` as `draw_slash_notehead()`
3. Update `get_note_glyph()` to handle slash type
4. Test with percussion examples (if we have any — check fixtures)

**Expected improvement**: Fixes missing notation in drum parts

---

### 5. Guitar Chord Frames (GrChordFrame) (P1)
**OG Source**: `Graphic.cp` or `ChordFrame.cp`
**Estimated LOC**: ~100-200 lines (complex)
**Impact**: Fixtures with guitar chord diagrams
**Visual**: Chord frames missing entirely
**Status**: Not implemented (GraphicType::GrChordFrame exists but no renderer)

#### Action Plan
1. Check if any fixtures use chord frames (grep fixture data for GrChordFrame type)
2. If used: port from OG Graphic.cp
3. If not used: **PUNT to P3** — no immediate need

**Expected improvement**: Only matters if fixtures actually use this feature

---

## Medium-Priority Polish (P2)

### 6. Composite Dynamics (P2)
**OG Source**: `Dynamic.cp` composite rendering
**Estimated LOC**: ~40-60 lines
**Impact**: Rare dynamics (più p, meno f, etc.)
**Visual**: Currently renders as single p/f — functional but imprecise
**Status**: TODO (draw_object.rs:1753-1756)

#### Action Plan
1. Read `Dynamic.cp` for composite dynamic rendering (multi-character placement)
2. Port to `src/draw/draw_object.rs` in `draw_dynamic()`
3. Update `dynamic_glyph_code()` to return multiple glyphs
4. Test with fixture that uses composite dynamics (check OG scores)

**Expected improvement**: Minor — only affects rare dynamics

---

### 7. Ledger Line Extension Logic (P2)
**OG Source**: `DrawNRGR.cp` ledger line rendering
**Estimated LOC**: ~20-30 lines
**Impact**: Notes far above/below staff
**Visual**: Ledger lines mostly correct, edge case polish
**Status**: TODO (draw_nrgr.rs:350)

#### Action Plan
1. Read `DrawNRGR.cp` ledger line logic (extension for accidentals, chord clusters)
2. Verify current implementation against OG semantics
3. Adjust if needed in `src/draw/draw_nrgr.rs`
4. Test with extreme pitch ranges (tc_55_1 has wide ranges)

**Expected improvement**: Subtle — fixes edge case appearance

---

### 8. Whole Measure Rest as Breve (P2)
**OG Source**: WholeMeasRestIsBreve() check
**Estimated LOC**: ~10-20 lines
**Impact**: Rare time signatures (4/2, 2/1, etc.)
**Visual**: Whole measure rest glyph choice in non-4/4 time
**Status**: TODO (draw_utils.rs:126)

#### Action Plan
1. Search OG for WholeMeasRestIsBreve() implementation
2. Port to `src/draw/draw_utils.rs` or `src/defs.rs`
3. Update `get_rest_glyph()` to call it
4. Test with non-standard time sigs (check if any fixtures use 4/2, etc.)

**Expected improvement**: Fixes rare edge case, low visual impact

---

## Low-Priority / Deferred (P3)

- **GrPict (PICT images)** — obsolete Mac format, skip entirely
- **GrChar (single character graphics)** — unused in modern scores
- **GrMidiPatch/Pan/Pedal** — performance data, not visual
- **ENCL_CIRCLE** — marked NOTYET in OG, skip

---

## Recommended Porting Sequence

### Phase 1: Fix Collisions (1-2 sessions)
1. ✅ **AccXOffset** (affects all accidentals) — 1 session
2. ✅ **ChkNoteAccs** (accidental staggering) — 1 session
3. ✅ **Stem X for seconds** — 30 min

**Goal**: Reduce tc_05 diff from 4.48% → ~2%, tc_55_1 from 19.9% → ~15%

### Phase 2: Missing Features (1-2 sessions)
4. ✅ **Slash notation** (if used in fixtures) — 1 session
5. ✅ **GrChordFrame** (if used in fixtures) — 1-2 sessions OR punt if unused

**Goal**: Cover common notation types

### Phase 3: Polish (1 session)
6. ✅ **Composite dynamics** — 30 min
7. ✅ **Ledger line extension** — 30 min
8. ✅ **WholeMeasRestIsBreve** — 30 min

**Goal**: Edge case refinement

---

## Success Metrics

### Visual Review Checklist (Flutter App)
After each porting task, review ALL fixtures via Flutter app:

#### Phase 1 Checklist (Collisions Fixed)
- [ ] No collisions between accidentals and noteheads (all fixtures)
- [ ] No collisions between accidentals in chords (tc_05, tc_55_1, tc_ich_bin_ja)
- [ ] Stem X positions correct for chord seconds (beamed_grace_notes, complex chords)
- [ ] Spacing looks natural (not cramped, not too loose)

#### Phase 2 Checklist (Features Complete)
- [ ] Slash notation renders correctly (percussion parts, if any)
- [ ] Guitar chord frames render (if any fixtures use them)
- [ ] All common notation elements visible

#### Phase 3 Checklist (Polish Complete)
- [ ] Ledger lines extend appropriately (extreme pitches)
- [ ] Composite dynamics render correctly (rare dynamics)
- [ ] Whole measure rests correct for unusual time signatures
- [ ] No visual regressions in previously-good fixtures

---

## Workflow Integration

### Per-Task Cycle
1. **Research**: Read OG source for the function (use subagent for file search)
2. **Port**: Translate C → Rust with reference comments
3. **Test**: Run `cargo test` to verify no regressions
4. **Visual Review**: Run Flutter app (`cd nightingale && flutter run -d macos`), inspect ALL affected fixtures
5. **Commit**: Small, focused commit with OG reference in message

**MANDATORY**: Never commit rendering changes without Flutter visual review of all affected scores.

### Example Commit Messages
```
Port AccXOffset from DrawNRGR.cp:396-406

Improves accidental positioning by accounting for notehead width and
staff context. Reduces tc_05 diff from 4.48% to 2.1%.

Reference: OG Nightingale DrawNRGR.cp lines 396-406

🤖 Generated with Claude Code
Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Next Session: Start with AccXOffset

**Recommended first task**: Port AccXOffset (P0, affects all accidentals, ~50 LOC)

**Steps**:
1. Read `../OGNGale_source/src/CFilesBoth/DrawNRGR.cp:396-406` (use subagent)
2. Port to `src/draw/draw_nrgr.rs` as `acc_x_offset()`
3. Update `draw_note()` to use it
4. Run `cargo test` to verify no regressions
5. Launch Flutter app: `cd nightingale && flutter run -d macos`
6. Visually inspect tc_02, tc_05, tc_55_1, and a few songs (01, 05, 13)
7. Check accidental spacing looks correct (not too close/far from noteheads)
8. Commit with reference to OG source

**Estimated time**: 1-2 hours (including testing and Flutter review)
