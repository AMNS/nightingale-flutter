# Nightingale Modernization — Progress & Roadmap

**Last Updated**: March 26, 2026

---

## Current Focus

### Priority 1: Flutter Renderer Overhaul
**Status**: NOT STARTED
**Goal**: Replace the CommandRenderer→ScorePainter pipeline with BitmapRenderer-based
rendering for the main score view, matching the quality of the PDF output.

The current main score view uses CommandRenderer (records draw commands) → FFI bridge →
ScorePainter (replays on Flutter Canvas). This produces noticeably lower quality than
the PDF renderer or the BitmapRenderer used by the comparison screens. The comparison
screens already demonstrate that BitmapRenderer produces excellent output — the task is
to use that same approach for the primary score view.

### Priority 2: MusicXML Import Polish
**Status**: IN PROGRESS (parsing complete, rendering issues remain)
- [ ] Staff line continuity — empty staves not rendering continuation lines
- [ ] Round-trip fidelity — NGL→XML→import→render should be visually stable
- [ ] Validate against MuseScore / Dorico round-trip

### Priority 3: Score Formatting Engine
**Status**: NOT STARTED
**Goal**: Auto-layout from scratch (SFormat.cp / SFormatHighLevel.cp port)
- System breaks, page breaks, spacing optimization
- Currently we only preserve OG layout from NGL files

### Deferred
- **MIDI Export** — Basic infrastructure works (`src/midi/export.rs`). Tempo map,
  velocity dynamics, articulation mapping remain. Good enough for now.
- **Editing Operations** — Tool palette, basic note entry (Phase B)
- **Cross-Platform** — Linux/Windows builds (Phase C)

---

## Completed Phases

### Phase 1: Rust Data Model — COMPLETE
- Rust workspace with cargo, pre-commit hooks (fmt + clippy + tests)
- DDIST/STDIST/SHORTQD coordinate types, 25 object/subobject structs
- .ngl binary reader (N103 + N105 formats), heap interpreter, all 25 types
- Notelist (.nl) parser, V1/V2, 13 record types
- Musical context system, forward-traversal propagation
- Score accessors, document header parser, duration math

### Phase 2: Drawing / Rendering Layer — COMPLETE
- MusicRenderer trait: 32 methods mirroring PS_Stdio.cp primitives
- PdfRenderer (pdf-writer): PostScript operators → PDF content stream, embedded Bravura SMuFL font
- BitmapRenderer (tiny-skia + ab_glyph): pure-Rust bitmap rendering for visual regression
- CommandRenderer: records RenderCommand stream for structural testing + Flutter bridge
- render_score() main loop (DrawHighLevel.cp port) dispatching all 16 object types

#### Notation Rendering
- Staff lines, barlines (single/double/final/repeat/dotted), clefs (all 12 types + aliases)
- Key signatures (all 7 clef types, cancelling naturals), time signatures (numeric + common/cut)
- Noteheads (standard + custom: X, harmonic, square, diamond, slash), stems, flags, dots
- Accidentals (all 5 types + staggering), ledger lines, note collision avoidance
- Beams (slope computation, cross-staff, system-boundary break)
- Ties (cross-system partial ties), slurs (NGL tapered Bezier + Notelist, cross-system)
- Tuplet brackets/numbers, grace notes (70% size, stem slash, beamed)
- Dynamics (hairpin + text), tempo marks (verbal + metronome glyph), volta brackets
- Articulations/ornaments (all 22 MODNR types), arpeggio signs, GRDraw lines
- Part names, measure numbers, page numbers, header/footer text
- Rehearsal mark enclosures, chord symbol normalization
- Ottava (8va/8vb/15ma, dashed brackets)
- Sonata→SMuFL character mapping (90+ characters)
- Cross-staff notation, cross-system slurs, multi-page pagination

#### Layout & Spacing
- Multi-system layout (CreateSystem/NewSysNums port)
- OG Gourlay spacing pipeline (SymWidthRight/Left, FIdealSpace, Respace1Bar)
- Multiple voices per staff, preamble layout, continuation preamble
- Anacrusis measure width, SMuFL-based line widths

### Phase 3: Engraving Engine — COMPLETE
- Beam.cp → beam.rs (GetBeamEndYStems, cross-staff beams)
- Objects.cp → objects.rs (NormalStemUpDown, ArrangeChordNotes, ArrangeNCAccs)
- SpaceTime.cp → space_time.rs (complete Gourlay pipeline)
- Slurs, tuplets, dynamics, all object types ported

### Phase 4: Flutter Shell — IN PROGRESS
- flutter_rust_bridge setup (v2.11.1, flat DTO bridge)
- FlutterRenderer backend (command-based → CustomPaint, 32 command types)
- Score view widget with multi-page rendering, zoom, file browser
- QA Compare screen (before/after visual diffs)
- OG Comparison screen (modern vs OG reference PDFs)
- **TODO**: Renderer overhaul (Priority 1 above)

### Phase 5: MusicXML — MOSTLY COMPLETE

#### Export (NGL → MusicXML) — DONE
- Notes, rests, chords, multi-voice, multi-part, clefs, key sigs, time sigs
- Beams, ties, slurs, dynamics, tuplets, grace notes, tempo marks
- Volta endings, repeat barlines, ottava, articulations, ornaments
- Part groups (brackets/braces), all 26 NGL fixtures pass DTD validation

#### Import (MusicXML → InterpretedScore) — IN PROGRESS
- Full parsing: notes, rests, chords, accidentals, ties, slurs, dynamics
- Tuplets, grace notes, articulations, ornaments, tempo marks
- Volta endings, repeat barlines, ottava, part groups
- Title/composer credits, guitar clef transposition, lyrics
- System/page layout integrated, beaming fixed
- **Remaining**: staff line continuity, round-trip fidelity

### NGL Binary Writer — COMPLETE
- Full N105 format writer: `src/ngl/writer.rs` + `src/ngl/pack_*.rs`
- All 24 object types with LINK backpatching
- 26/26 fixtures pass round-trip validation (read → write → read)
- Pixel-perfect roundtrip rendering verified

---

## Stats

| Metric | Value |
|--------|-------|
| Rust source files | 56 (.rs files under src/) |
| Rust source lines | ~51,900 |
| Rust test lines | ~7,800 (under tests/) |
| Test count | 408 (399 passed + 9 ignored) |
| Test fixture files | 26 .ngl + 41 .nl + 55 .musicxml |
| Insta snapshots | 87 |

---

## Key Documents

- **CLAUDE.md** — Project guide (architecture, conventions, porting patterns)
- **PROGRESS.md** — This file (progress tracker + roadmap)
- **TESTING.md** — Test infrastructure and strategy
- **README.md** — Project overview
