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

## Phase Plan

### Phase 0: Source Archaeology (DO THIS FIRST)
Classify every .cp/.h file as: DATA_MODEL, ENGRAVING, UI, PLATFORM, or EXPERIMENTAL.
Build a dependency graph. Output CSV + Mermaid diagram. This determines everything else.

### Phase 1: Rust Data Model
Port Precomps/ type definitions to Rust structs. Implement .ngl file reader that can
round-trip files. Use the existing Python NGL→MusicXML converter as a reference
implementation for validation.

### Phase 2: Engraving Engine
Port CFilesBoth/ engraving algorithms one subsystem at a time:
- Beam calculation (Beam.cp, GRBeam.cp)
- Spacing (SpaceTime.cp, SpaceHighLevel.cp)
- Note/rest layout (DrawNRGR.cp minus QuickDraw calls)
- Slurs (Slurs.cp)
- Tuplets (Tuplet.cp)
- Score formatting (SFormat.cp, SFormatHighLevel.cp)
Each subsystem gets its own module with tests.

### Phase 3: MusicXML
Implement MusicXML 4.0 import/export using the Rust data model.

### Phase 4: Rendering Abstraction
Create a platform-agnostic rendering trait that outputs "draw glyph X at position Y,Z"
instructions. This is what Flutter will consume.

### Phase 5: Flutter Shell
Build the UI. The Rust core provides layout-computed positions; Flutter draws them.

## Task Execution Pattern

Each session should:
1. Read this CLAUDE.md
2. Check PROGRESS.md for current state
3. Pick the next incomplete task
4. Do the work, commit, update PROGRESS.md
5. Keep commits small and focused

## Conventions

- Rust code: `snake_case`, modules mirror the original C++ file organization where sensible
- Tests: every ported function gets at least one test with data from a real .ngl file
- Comments: when porting, include a reference to the original C++ file and line number
- Commits: `[phase-N] descriptive message` format
