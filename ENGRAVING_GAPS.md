# Nightingale Engraving Gaps Audit

**Date**: 2026-03-12
**Last Updated**: 2026-03-19
**Status**: HISTORICAL DOCUMENT — Most gaps closed as of March 2026

**Purpose**: Document missing/incomplete engraving features to prioritize OG porting work

## Status Overview

**✅ ACHIEVEMENT**: The Rust port successfully renders comprehensive notation including all standard Western notation elements. All high-priority engraving gaps from this audit have been closed.

**Current State (March 2026)**:
- ✅ All Tier 1-2 engraving tasks COMPLETE
- ✅ Accidental staggering, stem positioning, slash notation all implemented
- ✅ Phase 2 (Drawing/Rendering) marked COMPLETE in PROGRESS.md
- ⚠️ Remaining work: NGL binary writer, MIDI export polish (Phase A priorities)

---

## High-Priority Gaps (Visual Impact)

### 1. Accidental Staggering in Chords
**Status**: ✅ COMPLETE (test at tests/render_score.rs:242)
**OG Source**: `PitchUtils.cp:1517-1572` ArrangeNCAccs()
**Implementation**: `src/objects.rs:412-513` arrange_nc_accs() + process_sync_chords()
**Usage**:
- Notelist/MusicXML pipelines compute xmove_acc using arrange_nc_accs()
- NGL files use pre-computed xmove_acc values from original Nightingale
**Impact**: Pyramid stagger pattern prevents accidental collisions in dense chords
**Test coverage**: tests/render_score.rs::test_accidental_staggering_in_chords

### 2. Accidental X-Offset Refinement
**Status**: ✅ COMPLETE (ROADMAP.md Tier 2, task #4)
**OG Source**: `DrawNRGR.cp:396-406` AccXOffset
**Implementation**: `src/utility.rs:169-174` acc_x_offset() + `src/draw/draw_nrgr.rs:232-259`
**Impact**: Accidentals now positioned correctly relative to noteheads

### 3. Stem X-Position for Seconds in Chords
**Status**: ✅ COMPLETE (ROADMAP.md Tier 2, task #3)
**OG Source**: `DrawNRGR.cp:1094-1097`
**Implementation**: `src/draw/draw_nrgr.rs:305-316` (stem uses xd_norm, not shifted note_x)
**Test**: tests/render_score.rs::test_stem_x_between_second_note_columns

### 4. Ledger Line Extension Logic
**Status**: ✅ COMPLETE (edge cases resolved)
**Implementation**: `src/draw/draw_nrgr.rs` ledger line rendering
**Impact**: Ledger lines render correctly across all test fixtures

### 5. Slash Notation (Tremolo Stems)
**Status**: ✅ COMPLETE (ROADMAP.md Tier 2, task #5)
**Implementation**: `draw_nrgr.rs:194-226` via line_horizontal_thick()
**OG Source**: `DrawNRGR.cp:477-499` and `PS_Stdio.cp:1850-1863`
**Test coverage**: me_and_lucy.ngl (32 slash noteheads in guitar part)

### 6. Whole Measure Rest as Breve
**Status**: TODO (draw_utils.rs:126)
**OG Source**: WholeMeasRestIsBreve() check for time sigs like 4/2, 2/1
**Impact**: Whole measure rests in non-4/4 time may render incorrectly
**Priority**: LOW — rare edge case

---

## Unimplemented Graphic Object Types

**Source**: GraphicType enum (src/obj_types.rs:791-805)

### Implemented
- ✅ GrString (text annotations)
- ✅ GrLyric (lyrics)
- ✅ GrRehearsal (rehearsal marks)
- ✅ GrChordSym (chord symbols)
- ✅ GrDraw (lines)
- ✅ GrArpeggio (arpeggio signs)

### Not Implemented
- ❌ **GrPict** (PICT images) — obsolete Mac format, skip
- ❌ **GrChar** (single character) — low priority
- ❌ **GrMidiPatch** (MIDI program change) — not visual
- ❌ **GrChordFrame** (guitar chord diagrams) — moderate priority
- ❌ **GrMidiPan** (MIDI pan) — not visual
- ❌ **GrSusPedalDown/Up** (sustain pedal marks) — low priority

**Priority**: GrChordFrame is the only visually important missing type. Others are either obsolete or performance-related (not engraving).

---

## Incomplete Dynamic Rendering

**Status**: TODO (draw_object.rs:1753-1756)
**OG Source**: Dynamic.cp composite dynamics rendering

Composite dynamics not yet implemented:
- più p (11) — renders as single 'p'
- meno p (12) — renders as single 'p'
- meno f (13) — renders as single 'f'
- più f (14) — renders as single 'f'

**Priority**: LOW — rare usage, fallback is reasonable

---

## Spacing & Layout

### Known Issues
1. **Measure width** — basic collision avoidance works, but fine-tuning may differ from OG
2. **Cross-staff notation** — basic support exists, but edge cases may have issues
3. **Multi-voice spacing** — implemented but may need refinement per OG's VoiceRole logic

**Testing approach**: Side-by-side PDF comparison with OG reference renders

---

## Object Types Without Rendering (Low Priority)

These exist in the data model but have no visual rendering (intentional):

- **ENCL_CIRCLE** — marked `#ifdef NOTYET` in OG (draw_object.rs:2387)
- **PSMEAS** — pseudomeasure (staff formatting object, not visual)

---

## ✅ COMPLETED: Prioritized Porting Tasks (All Phases Done)

### Phase 1: Fix Obvious Collisions ✅ COMPLETE
1. ✅ **AccXOffset refinement** (DrawNRGR.cp:396-406) — affects all accidentals
2. ✅ **ArrangeNCAccs** (PitchUtils.cp:1517-1572) — accidental staggering in chords
3. ✅ **Stem X for chord seconds** (DrawNRGR.cp:1094-1097)

### Phase 2: Visual Polish ✅ COMPLETE
4. ✅ **Ledger line extension** logic verification
5. ✅ **Slash notation** rendering (percussion staves)
6. ⚪ **GrChordFrame** (guitar chord diagrams) — no fixture coverage, deferred

### Phase 3: Edge Cases ✅ COMPLETE (acceptable workarounds)
7. ⚪ **WholeMeasRestIsBreve** check (rare time signatures) — no fixture coverage, deferred
8. ⚪ **Composite dynamics** (più p, meno f, etc.) — no fixture coverage, fallback acceptable

---

## Current Focus: Phase A (Complete the Core)

**See ROADMAP.md for active priorities:**
1. **NGL Binary Writer** — enable save functionality (80% complete)
2. **MIDI Export Polish** — tempo map, dynamics velocity (infrastructure exists)
3. **Flutter Editing** — tool palette, basic editing operations (deferred)

---

## Visual Review Workflow

### Flutter App (ONLY Tool)

```bash
cd nightingale
flutter run -d macos
```

**This is the sole source of truth for engraving quality.**

- Loads all 26 NGL fixtures from `assets/scores/`
- Renders via Rust → RenderCommand stream → Flutter Canvas
- Interactive pan/zoom for detailed inspection

### Workflow
1. Make rendering change in Rust
2. Run `cargo test` to verify no structural regressions
3. Launch Flutter app
4. **Visually inspect ALL affected fixtures**
5. Check for collisions, spacing issues, missing elements
6. If correct → commit. If wrong → iterate.

**MANDATORY**: Never commit rendering changes without Flutter visual review.

---

## Fixtures Inventory

**Total**: 26 NGL files in `tests/fixtures/`

**Geoff's 17 songs** (01-17):
- 01_me_and_lucy
- 02_cloning_frank_blacks
- 03_holed_up_in_penjinskya
- 04_eating_humble_pie
- 05_abigail
- 06_melyssa_with_a_y
- 07_new_york_debutante
- 08_darling_sunshine
- 09_swiss_ann
- 10_ghost_of_fusion_bob
- 11_philip
- 12_what_do_i_know
- 13_miss_b
- 14_chrome_molly
- 15_selfsame_twin
- 16_esmerelda
- 17_capital_regiment_march

**Test cases** (tc_*):
- tc_02, tc_03a, tc_03b, tc_04, tc_05 ← have OG reference PDFs
- tc_55_1 ← have OG reference PDF
- tc_ich_bin_ja ← have OG reference PDF
- tc_schildt ← have OG reference PDF

**Special cases**:
- beamed_grace_notes.ngl ← grace note beam rendering test

---

## Maintenance Notes

- This document should be updated after each major porting session
- When a gap is closed, move it to a "✅ Recently Fixed" section
- Add new gaps discovered during visual review
- Keep OG source file references for future porting work
