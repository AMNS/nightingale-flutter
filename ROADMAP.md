# Nightingale Modernization — Roadmap

**Project Phase**: Phases 1-4 complete, Phase 5 (MusicXML) 95% complete

---

## 🎯 Strategic Priorities

### Tier 1: Critical
**Goal**: Production-ready notation engine with full MusicXML interop

1. ✅ **Accidental Staggering** — COMPLETE
   - Implemented: `src/objects.rs:412-513` arrange_nc_accs() (port of PitchUtils.cp:1517-1572)
   - Used by: Notelist/MusicXML pipelines via process_sync_chords()
   - NGL files: Use pre-computed xmove_acc values from original Nightingale
   - Test: tests/render_score.rs::test_accidental_staggering_in_chords

2. ✅ **MusicXML Round-Trip Stability** — COMPLETE
   - Comprehensive test on all 26 NGL fixtures (tests/musicxml_pipeline.rs:577)
   - Results: 18/26 perfect note count stability, 8/26 +1 to +14 notes (rest synthesis)
   - Visual diffs: 3.1% to 26.8% (expected due to layout_score() system breaks)
   - Measure inflation resolved: Closing barlines added per system (expected behavior)
   - Analysis documented: doc/MUSICXML_ROUNDTRIP_FINDINGS.md

### Tier 2: High Priority
**Goal**: Engraving quality polish

3. ✅ **Stem X-Position for Seconds** — COMPLETE
   - Implemented: src/draw/draw_nrgr.rs:305-316 (stem uses xd_norm, not shifted note_x)
   - Port of DrawUtils.cp NoteXLoc() + PS_Stdio.cp PS_NoteStem()
   - Test: tests/render_score.rs::test_stem_x_between_second_note_columns

4. ✅ **AccXOffset Refinement** — COMPLETE
   - Implemented: src/utility.rs:169-174 acc_x_offset() + src/draw/draw_nrgr.rs:232-259
   - Core positioning logic matches OG DrawNRGR.cp:340-348
   - MusCharXOffset/MusCharYOffset (Sonata-specific) not needed for SMuFL fonts
   - Courtesy accidentals (parentheses) deferred (no fixture coverage)

5. **Slash Notation** — Percussion/drum scores
   - Investigate OG tremolo stem rendering
   - Add slash notehead support

6. **Staff Visibility Model** — Empty staff continuation
   - Understand OG staff visibility logic
   - Render continuation staves for resting parts

### Tier 3: Medium Priority
**Goal**: Interactive editing & playback

7. **MIDI Export** — Playback via Flutter
   - Port NightingaleMIDI.cp duration/pitch/velocity logic
   - Flutter audio synthesis integration

8. **SMuFL Metadata** — Use Bravura's engraving defaults
   - Load SMuFL JSON metadata (anchors, defaults)
   - Apply optical spacing corrections

9. **Editing Operations** — Tool palette, insert/delete
   - Port basic editing from CFilesEditor/
   - Flutter tool palette UI

10. **NGL Binary Writer** — Save edited scores (skeleton complete, implementation pending)
    - ✅ File structure and API designed (src/ngl/writer.rs)
    - ✅ OG source analyzed (FileSave.cp, HeapFileIO.cp, EndianUtils.cp)
    - TODO: LINK conversion, endian handling, object/subobject packing
    - Support N105 format (N106 future)

### Tier 4: Future
**Goal**: Full-featured notation app

11. **Score Formatting Engine** (SFormat.cp)
    - Auto layout from scratch (vs. preserving OG layout)
    - System breaks, page breaks, spacing optimization

12. **Cross-Platform Flutter** — Linux/Windows builds
    - Test on Linux, Windows
    - Package for distribution

13. **RPTEND Subtypes** (DC/DS/SEGNO) — Optional
    - No fixtures use these
    - OG also logs errors for these types

---

## ✅ Completed Milestones

- **Phase 1**: Rust Data Model ✓
- **Phase 2**: Drawing/Rendering Layer ✓
- **Phase 3**: Engraving Engine ✓
- **Phase 4**: Flutter Shell ✓ (editing deferred)
- **Phase 5**: MusicXML Import/Export (95% - polish ongoing)

---

## 📊 Current Metrics

| Metric | Value |
|--------|-------|
| Rust LOC | 34,700 (src/) + 8,900 (tests/) |
| Test count | 363 (358 passed, 5 ignored) |
| Test fixtures | 26 NGL + 41 Notelist + 18 MusicXML |
| PDF outputs | 678 (all pages, all fixtures) |
| Notation types | All standard Western notation + grace notes, tuplets, dynamics, articulations |
| Layout features | Multi-system, multi-page, Gourlay spacing, cross-staff beams |
| MusicXML validation | 18/18 xmlsamples pass DTD validation |

---

## 🔧 Development Workflow

### Daily Development Loop
1. Pick task from roadmap
2. Read OG C source (if porting)
3. Port to Rust with reference comments
4. Write/update tests
5. Run QA compare workflow:
   ```bash
   ./scripts/qa-compare-smart.sh  # Generate before/after PDFs
   cd nightingale && flutter run  # Visual review
   ```
6. Iterate until rendering correct
7. Commit with `cargo fmt` + `cargo clippy` pre-commit checks

### Visual QA Process
- **Before making changes**: Commit current state
- **After code changes**: Run qa-compare-smart.sh
- **Review in Flutter**: QA Compare screen shows only changed fixtures
- **Approve or fix**: Iterate until visual output matches expectations

### Testing Strategy
- **Unit tests**: Algorithm correctness (duration, pitch, spacing)
- **Integration tests**: Full score rendering (ngl_all.rs, notelist_all.rs, musicxml_pipeline.rs)
- **Snapshot regression**: Command stream hashes (Insta)
- **Visual regression**: PDF output (manual review via Flutter)

---

## 📚 Key Documents

- **CLAUDE.md** — Project guide for AI assistants (architecture, conventions, porting patterns)
- **PROGRESS.md** — Phase-by-phase progress tracker
- **ENGRAVING_GAPS.md** — Documented missing/incomplete features
- **TESTING.md** — Test infrastructure, fixture management
- **ROADMAP.md** — This file (strategic priorities)

---

## 🎵 The Mission

Port the Nightingale music notation engine from legacy C/Carbon/QuickDraw to a modern Rust core with Flutter UI, **preserving the engraving algorithms** that are the project's real value. Cut cruft ruthlessly. Prioritize visual correctness and MusicXML interop.

**Guiding Principle**: Faithful translation of OG algorithms, ruthless deletion of platform-specific code.
