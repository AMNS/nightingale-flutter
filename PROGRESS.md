# Nightingale Modernization — Progress Tracker

## Current Phase: 2 (Drawing / Rendering Layer)

## Phase 0: Source Archaeology — COMPLETE
- [x] Classify core source files by role (DATA_MODEL / ENGRAVING / UI / PLATFORM)
- [x] Build dependency graph (DEPENDENCY_CHAIN.csv, DEPENDENCY_DIAGRAM.md)
- [x] Produce porting roadmap (PORTING_ROADMAP.txt)

## Phase 1: Rust Data Model — COMPLETE

### Foundation (commit 7da7522)
- [x] Rust workspace with cargo, pre-commit hooks (fmt + clippy + tests)
- [x] DDIST/STDIST/SHORTQD coordinate types (basic_types.rs)
- [x] Constants/limits (limits.rs), enums/macros (defs.rs)
- [x] 25 object/subobject struct definitions (obj_types.rs)
- [x] Document/score header types (doc_types.rs)
- [x] .ngl binary reader, N103 format (ngl/reader.rs)
- [x] Heap interpreter, variable-stride, all 25 types (ngl/interpret.rs)

### Infrastructure (commit cb8d791)
- [x] Variable-stride object heap decoding (critical bug fix)
- [x] Full interpret_heap() with subobject unpacking (AStaff, AMeasure, AClef, AKeySig, ATimeSig)
- [x] Notelist (.nl) parser, V1/V2, 13 record types (notelist/parser.rs)
- [x] Musical context system, forward-traversal propagation (context.rs)

### Accessors & Math (commit 81c75fe)
- [x] Cross-validation: NGL interpreter across all 16 fixtures
- [x] Score accessors: head(), tail(), num_staves(), score_list(), syncs(), measure_objects()
- [x] Document header parser (ngl/doc_header.rs)
- [x] Duration math: code_to_l_dur, measure_dur, beat_l_dur, etc. (duration.rs)

## Phase 2: Drawing / Rendering Layer — IN PROGRESS

### Rendering Architecture (commits f1ff974, 42aa184)
- [x] MusicRenderer trait: 32 methods mirroring PS_Stdio.cp's 27 primitives + state mgmt
- [x] RenderCommand enum: serializable commands for Flutter bridge and test recording
- [x] CommandRenderer: records commands for structural testing
- [x] PdfRenderer (pdf-writer): PS_Stdio.cp PostScript operators mapped to PDF content stream
- [x] Embedded Bravura SMuFL font rendering in PDF output
- [x] score_renderer.rs: staff lines, barlines, clefs, time sigs, noteheads, stems, accidentals, ledger lines, beams (flat), flags
- [x] to_score.rs: Notelist->InterpretedScore converter (measure spacing, chords, voice filtering, beam grouping)
- [x] Preamble layout from CreateSystem (Score.cp:1785-1814), Ross-convention spacing
- [x] Stem direction: NormalStemUpDown + CalcYStem for single notes and chords
- [x] Ledger lines from NoteLedgers (DrawUtils.cp)
- [x] Stem/beam X from OG HeadWidth (defs.h:355)
- [x] Invisible initial measure (no spurious barline before anacrusis)
- [x] 20 PDF primitive smoke tests + 2 HBD_33 pipeline tests + 4 punted roadmap tests

### Recently Completed
- [x] **Barline fix**: barlines at end of system only, not start of next (system-boundary xd fix)
- [x] **Continuation preamble**: narrower preamble for systems 2+ (no time sig space), clef_xd + 2.5*dLineSp
- [x] **Beam system-boundary break**: beams no longer span across system breaks
- [x] **Multi-system layout**: port of CreateSystem/NewSysNums — measures grouped into N systems (default 4/system), each with SYSTEM→STAFF→CONNECT→CLEF→[content], stacked vertically via inter_system spacing. Time sig only on system 1. Renderer required zero changes.
- [x] **Multiple voices per staff**: VoiceRole enum (Single/Upper/Lower), auto voice role detection, UPPER stems-up, LOWER stems-down, shorter 2v stems (stemLen2v=12), multi-voice rest offset
- [x] **Visual regression test framework**: insta snapshot-based, HBD_33 blessed snapshot with command counts, staff/barline/beam geometry, glyph distribution
- [x] **Beam slope**: port GetBeamEndYStems (Beam.cp:181) + FixSyncInBeamset (Beam.cp:272), 33% slope reduction
- [x] **Beam group stem unification**: port NormalStemUpDown (Objects.cp:1594) for beam groups — voice-role-aware
- [x] **Renderer stem direction fix**: beam renderer now uses per-note ystem vs yd (matching OG) instead of heuristic
- [x] **OG source line endings**: converted all 276 .cp/.h files to Unix LF — no more `tr` preprocessing

### Recently Completed (this session)
- [x] **Tied notes**: visual ties between notes across beats/measures + cross-system partial ties
- [x] **Notelist stem_info parser fix**: accept all 6 flag characters (ties, slurs, tuplets were silently dropped)

### Next: Engraving & Layout (priority order)
- [ ] **Grace notes**: small grace notes before principal notes (DrawGRSync port)
- [ ] **Duration-proportional spacing**: port SymWidthRight/CalcSpaceNeeded (SpaceTime.cp)
- [ ] **Notehead collision avoidance**: seconds in chords (otherStemSide placement), multi-voice X offsets
- [ ] **Accidental staggering**: port ChkNoteAccs (DrawNRGR.cp)
- [ ] **Rest rendering**: show rests at beat positions without notes (multi-voice rest offset)
- [ ] **Ledger line weight**: config.ledgerLW (13% of lnSpace, PS_Stdio.cp:2211)
- [ ] **Final barline**: double barline at end of piece
- [ ] **Anacrusis measure width**: narrower to reflect partial duration
- [ ] **Mid-score time signature changes**: render TimeSig objects within measures (e.g. 3/4 -> 5/4 at m.9 in HBD_33)
- [ ] **Score markings**: fermata, other articulations (DrawMODNR port)
- [ ] **Textual elements**: dynamics, tempo markings, rehearsal marks

### Deferred
- [ ] Port MapMusChar() (Sonata->SMuFL glyph mapping)
- [ ] SMuFL metadata loading (anchors, engraving defaults)
- [ ] .ngl binary writer
- [ ] N105 format test fixtures

## Phase 3: Engraving Engine — PARTIALLY IN PROGRESS
- [x] Port Beam.cp GetBeamEndYStems/FixSyncInBeamset -> beam slope in to_score.rs
- [x] Port Objects.cp NormalStemUpDown -> beam group stem unification
- [ ] Port SpaceTime.cp / SpaceHighLevel.cp -> spacing module
- [ ] Port Slurs.cp -> slur module
- [ ] Port Tuplet.cp -> tuplet module
- [ ] Port SFormat.cp / SFormatHighLevel.cp -> format module

## Phase 4: Flutter Shell — NOT STARTED
- [ ] flutter_rust_bridge setup
- [ ] FlutterRenderer backend (command-based -> CustomPaint)
- [ ] Score view widget, tool palette, basic editing

## Phase 5: MusicXML — NOT STARTED (de-prioritized)
- [ ] MusicXML 4.0 export/import
- [ ] Validate against MuseScore / Dorico round-trip

## Stats
| Metric | Value |
|--------|-------|
| Rust source lines | ~20,500 |
| Rust test lines | ~3,000 |
| Test count | 157 (104 unit + 6 integration + 43 cross-validate/render + 8 doctest) |
| Test fixture files | 16 .ngl + 15 .nl |
| Commits | 7 |
| Modules | 12 (basic_types, limits, defs, obj_types, doc_types, ngl, notelist, context, duration, render, draw, lib) |
