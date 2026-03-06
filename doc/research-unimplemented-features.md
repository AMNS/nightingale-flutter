# Unimplemented Core Notation Features Audit

*Generated 2026-03-05 from codebase analysis*

## Overall Assessment

Rendering completeness: ~85% of core notation elements

## Tier 1 — High Impact Gaps

### 1. Cross-System Slurs
- **Status**: Detection code exists, rendering does not span systems
- **Impact**: 750+ slurs in fixtures marked `cross_system` but not rendered across boundaries
- **Files**: `src/draw/draw_object.rs` (slur rendering), `tests/ngl_all.rs` (diagnostic test)
- **Effort**: Medium — need to split slur curves at system boundaries

### 2. Pagination / Multi-Page Rendering
- **Status**: Only single-page rendering implemented
- **Impact**: Multi-page scores truncate at page 1
- **Files**: `src/draw/draw_high_level.rs`
- **Effort**: Medium-High — need page break logic and system allocation per page

### 3. Ottava (8va/8vb)
- **Status**: Parsed from NGL, rendering untested with real data
- **Impact**: Bracket/line drawing likely incomplete
- **Files**: `src/draw/draw_object.rs` (GRAPHIC object handling)
- **Effort**: Low-Medium — infrastructure exists, needs testing and refinement

### 4. Common/Cut Time Glyphs
- **Status**: Time signatures render as numbers only
- **Impact**: Common time (C) and cut time (₵) display as "4/4" and "2/2"
- **Files**: `src/draw/draw_object.rs` (time signature rendering)
- **Effort**: Low — just need glyph lookup for SMuFL common/cut time characters

## Tier 2 — Medium Impact

### 5. Header/Footer Text
- **Status**: Not implemented
- **Impact**: Score title, composer name, page numbers missing
- **Files**: Would need new drawing code in `draw_high_level.rs`
- **Effort**: Medium — need to extract from score metadata and render text

### 6. Tempo Markings
- **Status**: Parsed from NGL but not rendered
- **Impact**: No metronome marks or tempo text
- **Files**: Would be part of GRAPHIC object rendering
- **Effort**: Medium — need text + glyph (quarter note = 120) rendering

### 7. Rehearsal Marks
- **Status**: Not implemented
- **Impact**: No boxed/circled letter/number markers
- **Files**: Would be part of GRAPHIC object rendering
- **Effort**: Low-Medium

### 8. RPTEND Variants (D.C., D.S., Segno, Coda)
- **Status**: Repeat barlines (RptL/RptR) implemented; text/glyph variants not
- **Impact**: D.C. al Fine, D.S. al Coda, Segno/Coda symbols missing
- **Files**: `src/draw/draw_object.rs` (draw_rptend function)
- **Effort**: Low-Medium — need text rendering + segno/coda glyphs

## Tier 3 — Lower Priority

### 9. Chord Symbols
- **Status**: Not implemented
- **Impact**: Jazz/pop chord names (Am7, Cmaj9, etc.) missing
- **Effort**: Medium — text rendering with optional superscript/subscript

### 10. Hairpins (Crescendo/Diminuendo)
- **Status**: GRAPHIC objects parsed but hairpin wedges not rendered
- **Impact**: No crescendo/diminuendo wedge lines
- **Files**: `src/draw/draw_object.rs`
- **Effort**: Low — two diverging/converging lines

### 11. Arpeggio Lines
- **Status**: Not implemented
- **Impact**: Wavy vertical lines before chords missing
- **Effort**: Low — wavy line glyph repeated vertically

### 12. Pedal Marks
- **Status**: Not implemented
- **Impact**: Ped./★ marks and bracket lines missing
- **Effort**: Low-Medium

## What IS Implemented (Working)

- Staff lines, barlines, system brackets/braces
- Note heads (all durations), stems, flags
- Beams (regular + grace notes, primary/secondary/tertiary)
- Slurs (within single system)
- Ties
- Key signatures, time signatures (numeric), clefs
- Accidentals (sharp, flat, natural, double-sharp, double-flat)
- Dynamics text
- Ledger lines
- Tuplet brackets and numbers
- Grace notes (heads, stems, flags, beams, slashes)
- Dots (augmentation)
- Rests (all durations)
- Connector lines (between staves)
- Text strings (lyrics, expression marks)
