# Nightingale Drawing Pipeline Documentation Index

This index points to comprehensive documentation of the Nightingale C++ drawing pipeline, created for porting to Rust + Flutter.

## Files in This Documentation Set

### 1. DRAWING_PIPELINE_SUMMARY.txt
**Read this first.** Executive summary of the drawing pipeline architecture.
- Core concept (recursive descent with context transformation)
- Coordinate system (DDIST vs. pixels)
- CONTEXT structure
- Function hierarchy
- Critical drawing patterns
- Porting priorities (5 tiers)

**Best for**: Getting oriented quickly, understanding the big picture

### 2. DRAWING_PIPELINE_ANALYSIS.md
**Comprehensive technical reference.** Deep dive into each drawing function with code snippets.

Covers:
- Coordinate system and transformation macros
- CONTEXT structure definition and usage
- DrawRange() - entry point
- DrawScoreRange() - main dispatcher
- DrawSTAFF() - staff drawing
- DrawCONNECT() - bracket/brace drawing
- DrawMEASURE() - barline and measure number drawing
- DrawSYNC() - note/rest synchronization
- DrawNote() - individual note drawing (with detailed coordinate patterns)
- DrawAcc() - accidental drawing
- DrawNCLedgers() - ledger line drawing
- DrawAugDots() - augmentation dot drawing
- Function call hierarchy
- QuickDraw vs. PostScript calls
- Key insights for porting

**Best for**: Implementation, reference during porting, understanding coordinate transformations

### 3. DRAWING_FUNCTIONS_REFERENCE.txt
**Quick lookup table.** All functions listed with file paths, line numbers, and key patterns.

Contains:
- Coordinate transformation function locations
- High-level drawing function list
- Object-level drawing function list
- Note-level drawing function list
- Key code patterns with line numbers
- Data structure definitions
- Object type dispatch table
- Drawing output modes
- Visibility optimization details
- Context update sequence
- Important macros

**Best for**: Finding specific function locations, copy-paste code patterns

## Quick Navigation

### By Task

**Understanding the overall flow:**
1. Read DRAWING_PIPELINE_SUMMARY.txt sections "CORE CONCEPT" and "DRAWING FUNCTIONS HIERARCHY"
2. Review DRAWING_PIPELINE_ANALYSIS.md section "Function Call Hierarchy"

**Porting DrawNote():**
1. Read DRAWING_PIPELINE_ANALYSIS.md section on DrawNote() (extensive)
2. Check DRAWING_FUNCTIONS_REFERENCE.txt for exact line numbers
3. Reference the Screen Drawing and PostScript Drawing subsections for the two code paths

**Understanding coordinates:**
1. Read DRAWING_PIPELINE_SUMMARY.txt section "COORDINATE SYSTEM"
2. Read DRAWING_PIPELINE_ANALYSIS.md section on coordinate transformation
3. Review Magnify.cp D2PFunc() implementation (referenced in both docs)

**Porting DrawSTAFF():**
1. Read DRAWING_PIPELINE_ANALYSIS.md section on DrawSTAFF()
2. Understand context update pattern (line 614-623 in DrawObject.cp)
3. See DRAWING_PIPELINE_SUMMARY.txt pattern #1

**Finding a specific function:**
1. Use DRAWING_FUNCTIONS_REFERENCE.txt to locate file path and line number
2. Cross-reference DRAWING_PIPELINE_ANALYSIS.md for detailed description

### By Source File

**DrawHighLevel.cp:**
- DRAWING_PIPELINE_SUMMARY.txt: "DRAWING FUNCTIONS HIERARCHY"
- DRAWING_PIPELINE_ANALYSIS.md: "Drawing Pipeline: Top Level" section
- DRAWING_FUNCTIONS_REFERENCE.txt: "HIGH-LEVEL DRAWING FUNCTIONS"

**DrawObject.cp:**
- DRAWING_PIPELINE_ANALYSIS.md: DrawSTAFF, DrawCONNECT, DrawMEASURE sections
- DRAWING_FUNCTIONS_REFERENCE.txt: "OBJECT-LEVEL DRAWING FUNCTIONS"

**DrawNRGR.cp:**
- DRAWING_PIPELINE_ANALYSIS.md: DrawSYNC, DrawNote, DrawAcc, DrawNCLedgers, DrawAugDots sections
- DRAWING_FUNCTIONS_REFERENCE.txt: "NOTE-LEVEL DRAWING FUNCTIONS"

**Magnify.cp:**
- DRAWING_PIPELINE_ANALYSIS.md: "Coordinate System" section
- DRAWING_PIPELINE_SUMMARY.txt: "COORDINATE SYSTEM" section

**Precomps headers (NObjTypes.h, defs.h, NBasicTypes.h):**
- DRAWING_PIPELINE_ANALYSIS.md: "CONTEXT Structure" section
- DRAWING_FUNCTIONS_REFERENCE.txt: "DATA STRUCTURES"

## Key Concepts Reference

### DDIST (Double Distance)
- Signed short, range ±2048 points, resolution 1/16 point
- Used in data model and CONTEXT positions
- Converted to pixels at draw time via d2p() macro

### CONTEXT Structure
- Passed to ALL drawing functions
- Tracks position, visibility, layout, and state
- Values are ACCUMULATED as tree is descended (must be mutable reference in Rust)
- Critical helper: LNSPACE(pCont) = staffHeight / (staffLines - 1)

### d2p() Macro
- Converts DDIST to pixels
- Depends on magnification level (nonlinear for odd magnifications)
- Uses bit-shifting and optional 1.5x multiply

### Two Drawing Paths
- Screen/Print: QuickDraw calls with pixel coordinates (d2p() conversion)
- PostScript: PS_* calls with DDIST coordinates (no conversion)

### Object Hierarchy
- PAGE → SYSTEM → STAFF → MEASURE → SYNC → NOTE
- Linked-list traversal (RightLINK), not tree traversal

### Drawing Dispatch
- DrawRange() is entry point
- DrawScoreRange() walks object list and calls appropriate Draw* function for each type
- Each Draw* function updates CONTEXT and may call sub-drawing functions

## Porting Strategy

See DRAWING_PIPELINE_SUMMARY.txt section "PORTING PRIORITY" for the 5-tier approach:

1. **TIER 1**: DDIST, CONTEXT, d2p()
2. **TIER 2**: DrawNote() and subfunctions
3. **TIER 3**: DrawMEASURE(), DrawSTAFF(), DrawCONNECT()
4. **TIER 4**: DrawScoreRange() dispatcher
5. **TIER 5**: Grace notes, beams, slurs, tuplets, etc.

## Critical Implementation Notes

### Magnification
- Not a simple linear scale
- Even magnifications: simple bit-shift
- Odd magnifications: bit-shift then multiply by 1.5
- Must match C++ implementation exactly to avoid stem/beat misalignment

### Rendering Abstraction
- Cannot use QuickDraw directly in Rust
- Two options: command list or trait-based backend
- See DRAWING_PIPELINE_SUMMARY.txt "RENDERING ABSTRACTION REQUIRED" section

### Context Statefulness
- CONTEXT is updated as we descend (staffTop = aStaff->staffTop + pContext->systemTop)
- Must use mutable references in Rust
- Each level accumulates positions from parent level

### Coordinate Transformation Timing
- Happens at DRAW TIME, not layout time
- Data model uses DDIST
- Screen drawing converts to pixels via d2p()
- PostScript uses DDIST directly

### Voice Coloring and Dimming
- Color depends on note's voice
- Dimming depends on whether voice is "active" (LOOKING_AT macro)
- Must be preserved in rendering abstraction

## File Locations (Absolute Paths)

### C++ Source Files
```
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/CFilesBoth/DrawHighLevel.cp
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/CFilesBoth/DrawObject.cp
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/CFilesBoth/DrawNRGR.cp
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/CFilesBoth/Magnify.cp
```

### Header Files
```
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/Precomps/NObjTypes.h
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/Precomps/defs.h
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/Precomps/NBasicTypes.h
/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/src/Precomps/Context.h
```

## Code Snippet Locations

All code snippets in the documentation include line numbers and file names. Use these to navigate to the original C++ source for detailed context.

Examples:
- "DrawSTAFF() — DrawObject.cp:591" means start at line 591 of DrawObject.cp
- "D2PFunc implementation (Magnify.cp line 49-66)" means lines 49 through 66 of Magnify.cp
- "CONTEXT structure (NObjTypes.h line 871-899)" means lines 871 through 899 of NObjTypes.h

---

**Documentation created**: February 23, 2026
**For project**: Nightingale Modernization (Rust + Flutter port)
**Source**: Exhaustive analysis of C++ drawing pipeline (195K lines across 277 files)
