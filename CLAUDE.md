# Nightingale Modernization — Claude Code Project Guide

## Mission

Port the Nightingale music notation engine from legacy C++/Carbon/QuickDraw to a modern
Rust core with a Flutter UI, preserving the engraving algorithms that are the project's
real value. The owner (Geoff Chirgwin) is ruthless about cutting cruft — when in doubt,
delete it.

## Repository

- Upstream: https://github.com/AMNS/Nightingale (branch: `develop`)
- Fork/working branch: `modernize` (create if it doesn't exist)
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

## Progress & Roadmap

See `PROGRESS.md` for phase plan, current status, and next steps.
See `PORTING_ROADMAP.txt` for the detailed layer-by-layer porting roadmap.

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
`Nightingale/src/` (relative to this repo root)

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

## Conventions

- Rust code: `snake_case`, modules mirror the original C++ file organization where sensible
- Tests: every ported function gets at least one test with data from a real .ngl file
- Comments: when porting, include a reference to the original C++ file and line number
- Commits: `[phase-N] descriptive message` format
