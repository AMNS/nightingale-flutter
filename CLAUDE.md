# Nightingale Modernization — Claude Code Project Guide

## Mission

Port the Nightingale music notation engine from legacy C/Carbon/QuickDraw to a modern
Rust core with a Flutter UI, preserving the engraving algorithms that are the project's
real value. The owner (Geoff Chirgwin) is ruthless about cutting cruft — when in doubt,
delete it.

## Repository

- Upstream (OG C++ source): https://github.com/AMNS/Nightingale (branch: `develop`)
- This repo (Rust/Flutter port): https://github.com/AMNS/nightingale-flutter (branch: `main`)
- License: MPL-2.0

## Codebase Stats (develop branch)

- 195K lines across 277 source files (C++/C)
- 91.8% C++, 8.2% C
- ~650 QuickDraw drawing calls
- ~1,118 Carbon dialog API calls
- ~309 Carbon control API calls  
- ~414 Carbon window API calls

## Source Directory Map

```
src/
├── CFilesBoth/       68K lines — CORE: shared code (data model, drawing, file I/O,
│                      beams, layout, engraving). THIS IS WHAT WE'RE PORTING.
├── CFilesEditor/     76K lines — Editor operations (editing, MIDI I/O, dialogs,
│                      insert/delete, undo, reformatting). PARTIALLY PORT.
├── Utilities/        28K lines — Utility functions. PORT SELECTIVELY.
├── Precomps/          8K lines — Header files / type definitions. PORT FIRST.
├── FilesSearch/       5K lines — Score search. PORT LATER.
├── MIDI/              3K lines — MIDI (CoreMIDI already). PORT.
├── Headers/           3K lines — App-level headers. REFERENCE ONLY.
├── CFilesCarbon/      92 lines — Carbon stubs. DELETE.
├── CarbonPrinting/   1.3K lines — Carbon printing. DELETE (use PDF).
├── CarbonNavServices/ 735 lines — Carbon file dialogs. DELETE.
├── CarbonTEField/     728 lines — Carbon text fields. DELETE.
├── CustomLDEF/        165 lines — Custom list definition. DELETE.
├── CustomMDEF/       1.2K lines — Custom menu definition. DELETE.
```

Other directories:
```
doc/                   Tech notes, format docs (NgaleFileFormatStatus.txt is key)
MiscellaneousCode/     Experimental/unused code (NTypesNEW.h has proposed N106 types)
Resources/             Icons, bitmaps, .rsrc files
English.lproj/         Localization
```

## Architecture Decisions (FINAL — do not re-debate)

1. **Rust core** — all data model, engraving, layout, file I/O
2. **Flutter UI** via `flutter_rust_bridge` — Rust is authoritative, Flutter just renders
3. **No Carbon/QuickDraw** — zero legacy Mac APIs in the new code
4. **MusicXML import/export** — first-class citizen (interop with Dorico, MuseScore, etc.)
5. **SMuFL fonts** — replace Sonata dependency with the modern standard
6. **PDF output** — OS handles printing
7. **.ngl file format** — read support mandatory (N103/N105/N106), write support desired

## What to KEEP (translate faithfully)

- Score data model: the object hierarchy (HEADER, SYNC/NOTE, MEASURE, STAFF, etc.)
- Engraving/layout engine: beam angles, spacing, note placement, slur curves, etc.
- The ANOTE struct and all its L/G/P domain annotations (see NObjTypes.h)
- MIDI export
- The palette-based editing paradigm

## What to CUT (delete aggressively)

- All Carbon/QuickDraw UI code
- Mac Sound Manager / OMS / FreeMIDI playback
- QuickTime integration  
- Platform-specific printing
- Don's experimental features (anything in MiscellaneousCode/ or marked experimental)
- Resource fork dependencies (.rsrc files)
- Xcode project files (NightingalePPC.xcodeproj, NightingaleIntel.xcodeproj)

## What to ADD

- MusicXML import/export
- SMuFL font support
- Retina/HiDPI display
- Cross-platform (macOS, Linux, Windows via Flutter)

## Key Data Types (from Precomps/)

The core coordinate system uses DDIST (1/16 point resolution):
```
DDIST    = short    (range ±2048 points, resolution 1/16 point)
STDIST   = short    (range ±4096 staffLines, resolution 1/8 staffLine)
SHORTQD  = SignedByte (range ±32 staffLines, resolution 1/4 staffLine)
STD_LINEHT = 8     (STDIST units per staff interline space)
```

Object types use OBJECTHEADER (linked list with position, selection, visibility) and
SUBOBJHEADER (staff number, subtype, selection). The ANOTE struct is the most complex,
with ~30 fields spanning Logical, Graphical, and Performance domains.

## File Format Versions

- N103: Legacy (files from ~2002). Smaller DOCUMENTHDR.
- N105: Current (Nightingale 5.6). Larger header with expanded font table.
- N106: Proposed future format (see NgaleFileFormatStatus.txt and NTypesNEW.h)

Previous reverse engineering produced a working NGL→MusicXML converter in Python
(see conversation history). The `4B 40` note record marker pattern and 30-byte records
were identified. String pool uses `02 <length> <string bytes>` format.

## N105 Struct Alignment (mac68k pragma)

**CRITICAL**: The N105 header (`NObjTypesN105.h` line 6) uses `#pragma options align=mac68k`.
Under mac68k alignment, **every struct is padded to a 2-byte boundary**, including single-byte
structs like `KSITEM_5` (which becomes 2 bytes on disk despite having only 1 byte of data).
This means manual byte-offset calculations from the C struct definitions will be WRONG unless
you account for this padding.

### KSITEM_5 is 2 bytes, not 1

```c
typedef struct { char letcode:7; Boolean sharp:1; } KSITEM_5;
// sizeof(KSITEM_5) = 2 (1 byte data + 1 byte mac68k padding)
```

Therefore `WHOLE_KSINFO_5` = 7 x 2 (KSItem array) + 1 (nKSItems) = **15 bytes**.
(The NBasicTypesN105.h comment at line 37 confirms: "the macro takes 15 bytes".)

### ASTAFF_5 on-disk layout (50 bytes)

```
Offset  Size  Field
------  ----  -----------------
0       2     next (LINK)
2       1     staffn
3       1     selected:1+visible:1+fillerStf:6
4       2     staffTop (DDIST)
6       2     staffLeft (DDIST)
8       2     staffRight (DDIST)
10      2     staffHeight (DDIST)
12      1     staffLines
13      1     [PADDING — align fontSize]
14      2     fontSize (short)
16      2     flagLeading (DDIST)
18      2     minStemFree (DDIST)
20      2     ledgerWidth (DDIST)
22      2     noteHeadWidth (DDIST)
24      2     fracBeamWidth (DDIST)
26      2     spaceBelow (DDIST)
28      1     clefType
29      1     dynamicType
30      14    KSItem[0..6] (7 x 2 bytes: data byte + mac68k pad byte)
44      1     nKSItems
45      1     timeSigType
46      1     numerator
47      1     denominator
48      1     filler:3+showLedgers:1+showLines:4
49      1     [PADDING — struct aligned to 2-byte boundary]
              TOTAL: 50 bytes
```

### General rule for N105 subobject unpackers

When computing byte offsets for any N105 struct, always verify against the file's
`heap.obj_size` value. If manual offset calculation disagrees with obj_size, there is
padding. Use raw hex dumps of known-good data to locate fields empirically.

## Progress & Roadmap

See `PROGRESS.md` for phase plan, current status, and next steps.

## Task Execution Pattern

Each session should:
1. Read this CLAUDE.md
2. Check `PROGRESS.md` for current state
3. Pick the next incomplete task
4. Do the work, commit, update PROGRESS.md
5. Keep commits small and focused
6. Run `cargo fmt` and `cargo clippy` before committing

## Autonomous Work Cycle

When working on rendering/engraving tasks, operate in autonomous micro-cycles:

1. **Research** — Read the OG Nightingale C++ source for the relevant function
   (use subagents with haiku/sonnet for grep/read tasks to conserve credits).
2. **Port** — Faithfully translate the C++ logic to Rust with reference comments.
3. **Render** — Run the test that produces PDF, convert to PNG, visually inspect.
4. **Self-check** — Look for collisions, misalignments, incorrect positioning.
   If issues found, iterate (go to step 1 for the next discrete issue).
5. **Checkpoint** — After fixing a batch of discrete issues, present a visual
   review to the user before committing. Ask for human feedback on bigger-picture
   direction/decisions, not micro-level code choices.

**Minimize prompts, maximize self-feedback.** The user prefers fewer back-and-forth
exchanges with more work done per cycle.

## Visual Review & Bitmap Regression

### Golden bitmaps

Every NGL and Notelist fixture has a golden bitmap in `tests/golden_bitmaps/`:
- NGL: `{fixture_name}_page1.png`
- Notelist: `nl_{fixture_name}_page1.png`

Bitmap regression tests run as part of `cargo test`. On mismatch the test panics
with the pixel diff percentage and a path to the diff image.

### Reviewing visual changes

```sh
# Quick visual diff of all golden bitmaps vs git HEAD:
./scripts/visual-review.sh

# Or run the Rust test directly:
cargo test --test golden_diff -- --nocapture

# Diff images go to test-output/golden-diff/
#   {name}_old.png   — committed version
#   {name}_new.png   — current version
#   {name}_diff.png  — visual diff (matching=dimmed, changed=red)
```

### Updating goldens after intentional rendering changes

```sh
# Regenerate NGL goldens:
REGENERATE_REFS=1 cargo test test_all_ngl_bitmap_regression

# Regenerate Notelist goldens:
REGENERATE_REFS=1 cargo test test_all_notelists_bitmap_regression

# Then review the diffs before committing:
./scripts/visual-review.sh
```

### Shared test utilities

`tests/common/mod.rs` provides `pdf_to_png()` and `compare_images_and_diff()`
used by ngl_all.rs, notelist_all.rs, and golden_diff.rs. Do not duplicate these.

## Porting Drawing Code

OG Nightingale has two separate rendering pipelines:
- **Screen**: QuickDraw-based (DrawHighLevel.cp, DrawObject.cp, DrawNRGR.cp)
- **Print/PostScript**: PS_Stdio.cp primitives (moveto, lineto, curveto, stroke, fill)

Our Rust port targets the **PostScript/print pipeline** via the MusicRenderer trait,
which maps to PDF output. When porting drawing functions, prefer the PS_Stdio.cp
code path over QuickDraw code paths. Keep the two pipelines conceptually separate
during porting; consolidation can happen later once porting is more complete.

Each discrete drawing function should be ported as its own unit with tests:
- Port one function at a time (e.g., AccXOffset, DrawLedgerLines, CalcYStem)
- Include OG source file + line number in comments
- Test with real Notelist data rendered to PDF
- Create `#[ignore]` tests for known-punted items so they show up in test output

## OG Nightingale Source Location

The authoritative C++ source is the **local clone** at:
`./Nightingale/src/` (relative to this repo root, i.e. `nightingale-modernize/Nightingale/src/`)

**Always use this local copy** — not any parent-directory or absolute path.

Key directories: `CFilesBoth/`, `CFilesBothEd/`, `Utilities/`, `Precomps/`

Line endings have been converted to Unix LF. Use Read/Grep tools directly —
no need for `tr` or other preprocessing. Always reference this when porting.

## Test Data Strategy

Primary test data sources (in priority order):
1. **Notelist files** (`tests/notelist_examples/`) — simplest, most portable
2. **NGL fixture files** (`tests/fixtures/`) — real Nightingale documents
3. **VexFlow examples** — consider translating simple ones to Notelist format
4. **MusicXML** — future import path; see icebox code and seiso.com converters
   (nl2xml: https://www.seiso.com/nl2xml/, xml2nl: https://www.seiso.com/xml2nl/)

## Subagent Usage

Use the right model for the job:
- **haiku**: File search, grep, simple code reads, quick lookups
- **sonnet**: Code analysis, moderate complexity research
- **opus**: Complex porting decisions, architecture, nuanced code translation

Launch multiple subagents in parallel when tasks are independent.

**IMPORTANT:** Always instruct subagents to use only the Grep and Read tools for
searching/reading files — never Bash with grep, tr, cat, awk, etc. Bash commands
that aren't in the auto-approved list will produce blocking permission prompts that
require manual user approval, which defeats the purpose of autonomous subagents.

**NO PYTHON:** Do not use Python scripts for data analysis, hex dumps, or computation.
Use Rust tests and approved Bash commands (cargo, xxd, etc.) instead. If a new Bash
command is needed, ask the user to add it to the allowed list rather than using an
unapproved command that will block on permissions.

## Module Map (mirrors OG C source files)

Shared algorithm modules (used by both NGL and Notelist pipelines):
```
src/
├── pitch_utils.rs     <- PitchUtils.cp  (pitch→staff position)
├── utility.rs         <- Utility.cp     (calc_ystem, nflags, std2d, head_width)
├── music_font.rs      <- MusicFont.cp   (font metrics)
├── objects.rs         <- Objects.cp     (stem direction, chord processing, key sig)
├── beam.rs            <- Beam.cp        (beam slope computation)
├── space_time.rs      <- SpaceTime.cp   (duration-proportional spacing)
```

Drawing modules (split from score_renderer.rs):
```
src/draw/
├── draw_high_level.rs <- DrawHighLevel.cp  (render_score main loop)
├── draw_object.rs     <- DrawObject.cp     (staff, measure, connect, clef, keysig, timesig, ties)
├── draw_nrgr.rs       <- DrawNRGR.cp       (sync/notes/rests, ledger lines)
├── draw_utils.rs      <- DrawUtils.cp      (glyph mapping, KS Y offsets)
├── draw_beam.rs       <- DrawBeam.cp       (beam sets)
├── draw_tuplet.rs     <- Tuplet.cp         (tuplet brackets/numbers)
├── helpers.rs         (d2r_sum, count_staves, TieEndpoint)
└── score_renderer.rs  (backward-compat re-export shim)
```

## Conventions

- Rust code: `snake_case`, modules mirror the original C file organization
- Tests: every ported function gets at least one test with data from a real .ngl file
- Comments: when porting, include a reference to the original C file and line number
- Commits: `[phase-N] descriptive message` format
