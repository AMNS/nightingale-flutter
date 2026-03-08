# Nightingale Modernization — Progress Tracker

## Active Work: Phase 5 (MusicXML import/export) + Phase 2 polish

## Phase 0: Source Archaeology — COMPLETE
- [x] Classify core source files by role (DATA_MODEL / ENGRAVING / UI / PLATFORM)
- [x] Build dependency graph (DEPENDENCY_CHAIN.csv, DEPENDENCY_DIAGRAM.md)
- [x] Produce porting roadmap (superseded by this file)

## Phase 1: Rust Data Model — COMPLETE
- [x] Rust workspace with cargo, pre-commit hooks (fmt + clippy + tests)
- [x] DDIST/STDIST/SHORTQD coordinate types, 25 object/subobject structs
- [x] .ngl binary reader (N103 + N105 formats), heap interpreter, all 25 types
- [x] Notelist (.nl) parser, V1/V2, 13 record types
- [x] Musical context system, forward-traversal propagation
- [x] Score accessors, document header parser, duration math

## Phase 2: Drawing / Rendering Layer — COMPLETE (polish ongoing)

All OG drawing functions ported. Remaining work is engraving polish only.

### Rendering Architecture
- [x] MusicRenderer trait: 32 methods mirroring PS_Stdio.cp primitives
- [x] PdfRenderer (pdf-writer): PostScript operators → PDF content stream, embedded Bravura SMuFL font
- [x] BitmapRenderer (tiny-skia + ab_glyph): pure-Rust bitmap rendering for test-loop visual regression
- [x] CommandRenderer: records RenderCommand stream for structural testing + Flutter bridge
- [x] render_score() main loop (DrawHighLevel.cp port) dispatching all 16 object types

### Notation Rendering (all complete)
- [x] Staff lines, barlines (single/double/final/repeat/dotted), clefs (all 12 types + alias clefs)
- [x] Key signatures (all 7 clef types, cancelling naturals), time signatures (numeric + common/cut)
- [x] Noteheads (standard + custom: X, harmonic, square, diamond, slash), stems, flags, dots
- [x] Accidentals (all 5 types + staggering), ledger lines, note collision avoidance (seconds in chords)
- [x] Beams (slope computation, cross-staff, system-boundary break, stem gap fix)
- [x] Ties (cross-system partial ties), slurs (NGL tapered Bezier + Notelist endpoint matching, cross-system)
- [x] Tuplet brackets/numbers, grace notes (70% size, stem slash, beamed)
- [x] Dynamics (hairpin + text), tempo marks (verbal + metronome glyph), volta brackets
- [x] Articulations/ornaments (all 22 MODNR types), arpeggio signs, PSMEAS, GRDraw lines
- [x] Part names, measure numbers, page numbers, header/footer text
- [x] Rehearsal mark enclosures, chord symbol normalization
- [x] Ottava (8va/8vb/15ma, dashed brackets — no fixture coverage)
- [x] Sonata→SMuFL character mapping (90+ characters)
- [x] Cross-staff notation, cross-system slurs, multi-page pagination

### Layout & Spacing
- [x] Multi-system layout (CreateSystem/NewSysNums port)
- [x] OG Gourlay spacing pipeline (SymWidthRight/Left, FIdealSpace, ConsiderITWidths, Respace1Bar)
- [x] Multiple voices per staff (VoiceRole, auto detection, stem direction, rest offset)
- [x] Preamble layout, continuation preamble, clef changes, mid-score time sig changes
- [x] Anacrusis measure width, line widths from lnSpace (staff/ledger/stem/barline)

### Visual Regression Testing
- [x] 678 golden bitmaps across 85 fixtures (26 NGL + 41 Notelist + 18 MusicXML), all pages
- [x] Insta snapshot regression, command-stream hash regression
- [x] Golden diff tool (scripts/visual-review.sh)

### Deferred
- [ ] SMuFL metadata loading (anchors, engraving defaults)
- [ ] .ngl binary writer
- [~] RPTEND subtypes RPT_DC/DS/SEGNO — OG also logs errors for these; no fixtures use them

## Phase 3: Engraving Engine — MOSTLY COMPLETE
- [x] Beam.cp → beam.rs (GetBeamEndYStems, FixSyncInBeamset, cross-staff beams)
- [x] Objects.cp → objects.rs (NormalStemUpDown, ArrangeChordNotes, ArrangeNCAccs, SetupKeySig)
- [x] SpaceTime.cp → space_time.rs (complete Gourlay pipeline)
- [x] Slurs.cp → draw_object.rs (NGL + Notelist, single-system + cross-system)
- [x] Tuplet.cp → draw_tuplet.rs (DrawTUPLET/DrawPSTupletBracket)
- [x] DrawObject.cp → draw_object.rs (all object types: OTTAVA/DYNAMIC/GRAPHIC/TEMPO/ENDING/etc.)
- [ ] SFormat.cp / SFormatHighLevel.cp → format module (pagination, system layout from scratch)

## Phase 4: Flutter Shell — IN PROGRESS
- [x] flutter_rust_bridge setup (v2.11.1, flat DTO bridge)
- [x] FlutterRenderer backend (command-based → CustomPaint, 32 command types)
- [x] Score view widget with multi-page rendering, zoom, file browser
- [ ] Tool palette, basic editing

## Phase 5: MusicXML — IN PROGRESS

### Export (NGL → MusicXML) — DONE
- [x] Notes, rests, chords, multi-voice, multi-part, clefs, key sigs, time sigs
- [x] Beams, ties, slurs, dynamics (text + hairpin wedges)
- [x] Tuplets, grace notes, tempo marks, volta endings, repeat barlines, ottava
- [x] Articulations, ornaments, technical marks
- [x] Part groups (brackets/braces from Connect objects)
- [x] All 26 NGL fixtures pass MusicXML DTD validation

### Import (MusicXML → InterpretedScore) — IN PROGRESS
- [x] Notes, rests, chords, multi-voice, multi-part, clefs, key sigs, time sigs
- [x] Accidentals, dots, ties, slurs, dynamics
- [x] Tuplets (time-modification → Tuplet + ANoteTuple objects)
- [x] Grace notes (→ GrSync + AGrNote objects with full pitch positioning)
- [x] Articulations and ornaments (AModNr sub-objects, 14 mod_code types)
- [x] Tempo marks (Tempo objects + score.tempo_strings)
- [x] Volta endings (Ending objects), repeat barlines (RptEnd objects)
- [x] Ottava (Ottava objects with oct_sign_type mapping)
- [x] Part groups (Connect objects from `<part-group>` elements)
- [x] Title/composer credits (page-relative GrString GRAPHICs from `<movement-title>`/`<creator>`)
- [x] Lyrics (GrLyric GRAPHICs from `<lyric>/<syllabic>/<text>`)
- [ ] **Beams — need major rework** (rendering as thick black rectangles, wrong slopes/groupings)
- [ ] **Measures/barlines — broken** (missing barlines, uneven spacing)
- [ ] Round-trip tests: NGL→MusicXML→import→render, canonical stability

### Validation
- [x] MusicXML golden bitmap regression (56 goldens across 18 xmlsamples)
- [ ] Validate against MuseScore / Dorico round-trip

## Phase 6: Sound Playback / MIDI — NOT STARTED
- [ ] MIDI export (port NightingaleMIDI.cp logic — duration/pitch/velocity/channel)
- [ ] Real-time playback via Flutter (Rust emits events, Flutter drives synth)
- [ ] Metronome / click track
- [ ] Tempo map from TEMPO objects
- [ ] Playback cursor: highlight currently playing note in UI

## Stats
| Metric | Value |
|--------|-------|
| Rust source files | 49 (.rs files under src/) |
| Rust source lines | ~34,700 |
| Rust test lines | ~8,900 (under tests/) |
| Test count | 363 (358 passed + 5 ignored) |
| Test fixture files | 26 .ngl + 41 .nl + 18 .musicxml |
| Insta snapshots | 87 |
| Bitmap goldens | 678 (26 NGL + 41 Notelist + 18 MusicXML, all pages) |
| Module subdirs | 6 (draw, musicxml, ngl, notelist, render + top-level) |
