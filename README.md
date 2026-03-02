# Nightingale Flutter

**Nightingale reborn in the 21st century: a faithful cross-platform port of the classic Mac music notation app.**

Nightingale was a professional music notation editor for the Mac, developed from 1988 through
the mid-2020s, largely by Don Byrd from the software's inception until not long before his death.
This project attempts to the full application — its engraving engine, data model, and
UI — from ~195K lines of C/QuickDraw/Carbon to a modern Rust core with a Flutter UI. Much of the engraving
engine and data model are ported and functioning.

- Upstream (origin C source): https://github.com/AMNS/Nightingale
- License: MPL-2.0

---

## Architecture

```
nightingale-modernize/
├── src/                    # Rust core library (nightingale_core crate)
│   ├── ngl/                # .ngl binary file reader (N103/N105 formats)
│   ├── notelist/           # Notelist text format parser + score converter
│   ├── draw/               # Drawing modules (mirrors OG C source layout)
│   │   ├── draw_high_level.rs   # render_score() main loop  <- DrawHighLevel.cp
│   │   ├── draw_object.rs       # staff/clef/keysig/timesig <- DrawObject.cp
│   │   ├── draw_nrgr.rs         # notes/rests/beams         <- DrawNRGR.cp
│   │   ├── draw_beam.rs         # beam sets                 <- DrawBeam.cp
│   │   ├── draw_tuplet.rs       # tuplet brackets           <- Tuplet.cp
│   │   └── draw_utils.rs        # glyph mapping, KS offsets <- DrawUtils.cp
│   ├── render/             # Rendering backends
│   │   ├── pdf_renderer.rs      # PDF output via pdf-writer
│   │   ├── bitmap_renderer.rs   # PNG output via tiny-skia (for tests)
│   │   └── command_renderer.rs  # Command stream (for Flutter bridge)
│   ├── beam.rs             # Beam slope computation        <- Beam.cp
│   ├── objects.rs          # Stem direction, chord layout  <- Objects.cp
│   ├── space_time.rs       # Gourlay spacing engine        <- SpaceTime.cp
│   ├── pitch_utils.rs      # Pitch/staff position math     <- PitchUtils.cp
│   ├── utility.rs          # CalcYStem, head widths, etc.  <- Utility.cp
│   └── ...
├── tests/                  # Integration tests
│   ├── fixtures/           # Real .ngl score files (17 fixtures)
│   ├── notelist_examples/  # Notelist text scores (41 fixtures)
│   ├── golden_bitmaps/     # Pixel-exact reference renders (491 PNGs)
│   └── og-reference/       # OG Nightingale PDF renders for visual comparison
├── nightingale/            # Flutter app (file browser + score viewer)
│   └── rust/               # Rust FFI bridge (flutter_rust_bridge)
└── Nightingale/            # OG C++ source (git submodule / local checkout)
    └── src/CFilesBoth/     # Core engraving algorithms being ported
```

### Two rendering pipelines

The port intentionally supports two input formats in parallel:

| Format | Description | Entry point |
|--------|-------------|-------------|
| `.ngl` | Original Nightingale binary files (N103/N105) | `ngl::read_score()` |
| Notelist | Nightingale text export format | `notelist::parse()` + `to_score()` |

Both pipelines produce an `InterpretedScore` struct that feeds the same rendering engine.

### Three rendering backends

All implement the `MusicRenderer` trait:

| Backend | Usage |
|---------|-------|
| `PdfRenderer` | PDF output (document export, printing) |
| `BitmapRenderer` | PNG output (visual regression tests in `cargo test`) |
| `CommandRenderer` + Flutter canvas | Live rendering in the Flutter UI |

---

## Development Setup

### Prerequisites

- Rust stable (1.75+): https://rustup.rs
- Flutter 3.x: https://flutter.dev/docs/get-started/install (for the UI app only)
- `flutter_rust_bridge_codegen` (if modifying the FFI bridge): `cargo install flutter_rust_bridge_codegen`

### Build & test

```sh
# Run all tests (fast — ~30s, skips slow bitmap regression)
cargo test

# Run with visual regression tests (slow — ~3min, generates/compares 491 PNGs)
cargo test --features visual-regression

# Run a specific test
cargo test test_ngl_capital_regiment_march

# Regenerate golden bitmaps after intentional rendering changes
REGENERATE_REFS=1 cargo test --features visual-regression test_all_ngl_bitmap_regression
REGENERATE_REFS=1 cargo test --features visual-regression test_all_notelists_bitmap_regression
```

### Running the Flutter app

The Flutter app (`nightingale/`) provides a live score viewer with a file browser sidebar.
It discovers `.ngl` and `.nl` fixture files relative to the working directory and renders
them on-demand via the Rust FFI bridge.

```sh
cd nightingale
flutter pub get
flutter run -d macos
```

The app requires the Rust bridge to be built first (done automatically by `flutter run`
via the `flutter_rust_bridge` build script). If you modify the Rust FFI API in
`nightingale/rust/src/api/`, regenerate the Dart bindings:

```sh
cd nightingale
flutter_rust_bridge_codegen generate
```

### OG comparison renders

The `tests/og_comparison.rs` suite renders our output alongside the original Nightingale
PDF output for side-by-side comparison. Images go to `test-output/og-comparison/`:

```sh
cargo test compare_og_references
# Then view PNG pairs in test-output/og-comparison/:
#   {fixture}_og_page1.png   — original Nightingale output
#   {fixture}_ours_page1.png — our render
```

The golden bitmap diff workflow produces visual diffs across all 491 golden bitmaps:

```sh
# Run visual regression and produce diff images for changed bitmaps
cargo test --features visual-regression

# Diff images go to test-output/golden-diff/:
#   {name}_old.png   — committed golden
#   {name}_new.png   — current render
#   {name}_diff.png  — pixel diff (matching=dimmed, changed=red)
```

### Pre-commit hooks

The repo has a pre-commit hook that runs `cargo fmt`, `cargo clippy`, `cargo test`,
and the golden bitmap diff check before each commit. Run manually:

```sh
.git/hooks/pre-commit
```

---

## Key Concepts

### Coordinate system

Distances in the engraving engine use `DDIST` (1/16 point resolution):

```
DDIST    = i16    (±2048 points @ 1/16pt resolution)
STDIST   = i16    (staff interline units @ 1/8 staffLine)
STD_LINEHT = 8   (STDIST units per staff interline space)
```

### Object model

Scores are a linked list of typed objects (`ObjData` enum), each with zero or more
subobjects. The main types mirror the OG `NObjTypes.h`:

- `SYNC` / note: notes and rests, grouped into chords by voice
- `MEASURE`: barline + measure metadata
- `STAFF` / `SYSTEM`: layout containers
- `CLEF`, `KEYSIG`, `TIMESIG`: preamble objects
- `SLUR`, `BEAM`, `TUPLET`, `DYNAMIC`, `TEMPO`, `GRAPHIC`: attached markings

### Porting convention

Every ported function includes a comment citing the OG source file and line number:

```rust
// OG: DrawObject.cp:1490 DrawEnclosure()
fn draw_enclosure(renderer: &mut dyn MusicRenderer, ...) { ... }
```

When in doubt about the intended behavior, read the OG source at
`Nightingale/src/CFilesBoth/` (local checkout required — see CLAUDE.md).

---

## Testing Strategy

| Test suite | File | What it covers |
|------------|------|----------------|
| Unit tests | `src/**/*.rs` | Individual functions (spacing math, glyph mapping, etc.) |
| NGL integration | `tests/ngl_all.rs` | Read + render all 17 .ngl fixtures |
| Notelist integration | `tests/notelist_all.rs` | Parse + render all 41 .nl fixtures |
| Visual regression | (both, `--features visual-regression`) | Pixel-diff against 491 golden PNGs |
| OG comparison | `tests/og_comparison.rs` | Side-by-side vs original Nightingale output |
| Cross-validation | `tests/cross_validate.rs` | Consistency checks across formats |

Golden bitmaps live in `tests/golden_bitmaps/`. Update them intentionally with
`REGENERATE_REFS=1`; the diff check in the pre-commit hook will catch accidental changes.

---

## Current Status

Phase 2 (drawing/rendering) is well underway. See `PROGRESS.md` for the full roadmap
and feature checklist. The high-level picture:

- **Working well**: staff/barline/clef/keysig/timesig layout, notes/rests/accidentals,
  beams, stems, slurs (single-system), tuplets, dynamics, tempo, lyrics, ties,
  grace notes, articulations, repeats, ottava, chord symbols, key cancellation,
  multi-voice, multi-page, page numbers
- **In progress / known gaps**: cross-system slurs, pagination, common/cut time glyphs,
  RPTEND symbols (segno/coda/D.C./D.S.), alias clefs, header/footer text, arpeggio signs
- **Not started**: MusicXML import/export, editor operations, MIDI playback

---

## UI / Editing Roadmap

The current Flutter app is a **read-only viewer**. The editing layer is the long-term goal.
The OG Nightingale editor (in `CFilesEditor/`, ~76K lines) covers:

- **Tool palette**: select, insert (note/rest/grace note), duration, pitch, articulation,
  dynamics, text, slur, hairpin tools — all driven by a palette-based interaction model
- **Note input**: step-time and real-time (MIDI) entry, auto-advancing cursor, chord building
- **Selection & editing**: click-select, drag-select, pitch adjust, duration change,
  copy/paste, delete, transpose, insert/delete measures
- **Reformatting**: re-lay out after edits (RespaceMeasure, ReformatSystem),
  system/page break control
- **MIDI I/O**: CoreMIDI playback (replace with cross-platform MIDI library), step-time input
- **Undo/redo**: OG uses a command-based undo stack (Undo.cp)

The porting strategy for the editor layer:

1. **Flutter tool palette** — replicate the OG palette UI in Flutter, wired to Rust
   mutation commands (not yet designed)
2. **Rust mutation API** — add `score.insert_note()`, `score.delete_measure()`, etc.
   to the Rust core, with the OG editor functions as the reference implementation
3. **Reformatting** — port `RespaceMeasure` / `ReformatSystem` (SpaceHighLevel.cp,
   SFormat.cp) to trigger after any mutation
4. **MIDI playback** — replace CoreMIDI with a cross-platform MIDI library
   (e.g. `midir` crate) once the viewer phase is stable

MusicXML import/export is tracked separately (Phase 5) and is independent of the
editor work.
