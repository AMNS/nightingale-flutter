# RPTEND & ENDING Rendering Implementation Plan

## Overview

This plan outlines the implementation of volta bracket (ENDING) and repeat barline (RPTEND) rendering in Nightingale Modernize. Test fixture: `17_capital_regiment_march.ngl` (contains DS, repeat dots, and coda markers).

**Status**: Research phase complete. Implementation ready to begin in next session.

---

## Phase 1: ARPTEND Unpacking (Blocking)

Currently stubbed in `src/ngl/unpack_stubs.rs`. RPTEND rendering cannot proceed without this.

### Task 1a: Implement `unpack_arptend_n105()`
**File**: `src/ngl/unpack_stubs.rs` (lines ~170-175)

**Data Structure** (6 bytes per staff):
```
Offset  Size  Field
------  ----  -------
0       1     subType (RPT_L, RPT_R, RPT_LR, etc.)
1       1     connAbove (link to staff above)
2       1     connStaff (first staff in bracket group)
3-4     2     ?unused?
5       1     ?unused?
```

**Inputs**:
- `bytes`: 6-byte slice from N105 heap file
- Current position in file

**Output**:
- `Result<Arptend, String>` where `Arptend` contains the unpacked fields

**Reference**:
- OG: `NObjTypes.h:142-147` (ARPTEND_5 struct)
- Analysis: `/RPTEND_AND_VOLTA_ANALYSIS.md`

### Task 1b: Wire ARPTEND unpacking into main heap unpacking
**File**: `src/ngl/unpack.rs`

**Where**: In `unpack_object()` function, when `obj_type == RPTEND`
- Current: Falls through to stub
- Change: Call `unpack_arptend_n105()` for each staff's ARPTEND subobject
- Store results in `InterpretedRptEnd` struct (see next task)

### Task 1c: Create `InterpretedRptEnd` struct
**File**: `src/ngl/interpret.rs`

```rust
pub struct InterpretedRptEnd {
    pub id: ObjRef,
    pub staff_id: ObjRef,
    pub meas_id: ObjRef,
    pub subtype: u8,  // RPT_L, RPT_R, RPT_LR
    pub conn_above: Option<ObjRef>,
    pub conn_staff: Option<ObjRef>,
    // ... other fields from OG RPTEND struct
}
```

**Reference**: RPTEND struct in NObjTypes.h:149-156

---

## Phase 2: ENDING Unpacking

Unlike RPTEND, ENDING objects don't have subobjects, so unpacking is simpler.

### Task 2a: Implement `unpack_ending_n105()`
**File**: `src/ngl/unpack_stubs.rs` (new function)

**Data** (N105 ENDING on-disk format, ~30 bytes):
```
OBJECTHEADER (4 bytes)
firstObjL     (4 bytes)  → first measure/sync in bracket
lastObjL      (4 bytes)  → last measure/sync in bracket
endNum        (1 byte)   → ending label code (1-31, maps to ending strings)
noLCutoff     (1 bit)    → suppress left vertical line?
noRCutoff     (1 bit)    → suppress right vertical line?
spareFlags    (6 bits)   → reserved
endxd         (2 bytes)  → x offset for right bracket
... (more fields)
```

**Reference**:
- OG: `NObjTypes.h:795-806` (ENDING struct)
- Analysis: `/RPTEND_AND_VOLTA_ANALYSIS.md`

### Task 2b: Create `InterpretedEnding` struct
**File**: `src/ngl/interpret.rs`

```rust
pub struct InterpretedEnding {
    pub id: ObjRef,
    pub first_obj_id: ObjRef,   // measure or sync
    pub last_obj_id: ObjRef,
    pub end_num: u8,             // 1-31 (maps to ending strings table)
    pub no_l_cutoff: bool,       // suppress left bracket line
    pub no_r_cutoff: bool,       // suppress right bracket line
    pub endxd: i16,              // x offset for right bracket
}
```

---

## Phase 3: Rendering

All rendering code targets PDF output via `MusicRenderer` trait.

### Task 3a: Implement `draw_rptend()`
**File**: `src/draw/draw_object.rs` (new function, ~100 lines)

**Entry point**: Called from `render_score()` when obj_type == RPTEND

**Logic**:
1. Get RPTEND data: subtype, conn_staff, firstObj, lastObj
2. For each staff in the system:
   - Decide: should we draw full barline or dots-only? (Use `should_re_draw_barline()`)
   - Get horizontal position from first measure/sync
   - Get vertical extents from staff metrics
   - Call `draw_rpt_bar()` to render dots + barline

**Reference**:
- OG: `DrawObject.cp:1330` (DrawRPTEND)
- Helper: `GetRptEndDrawInfo()` (DrawUtils.cp:529)
- Helper: `ShouldREDrawBarline()` (DrawUtils.cp:1967)
- Core render: `DrawRptBar()` (DrawUtils.cp:592)

### Task 3b: Implement `draw_rpt_bar()` helper
**File**: `src/draw/draw_utils.rs` (new function, ~80 lines)

**Purpose**: Draw repeat dots + barline for a single staff

**Signature**:
```rust
fn draw_rpt_bar(
    renderer: &mut dyn MusicRenderer,
    subtype: u8,      // RPT_L, RPT_R, RPT_LR
    x_pos: f64,       // horizontal position
    top_y: f64,       // top of staff
    bottom_y: f64,    // bottom of staff
    dot_spacing: f64, // vertical spacing between dots
) -> Result<(), String>
```

**Algorithm**:
1. Draw repeat dots using SMuFL glyph (codepoint `U+E040` for dots, or augmentation dot fallback)
2. Draw barline(s) based on subtype:
   - RPT_L: dots on right
   - RPT_R: barline on left + dots on right
   - RPT_LR: barline both sides + dots in middle

**Reference**: OG `DrawUtils.cp:592` (DrawRptBar)

### Task 3c: Implement `should_re_draw_barline()`
**File**: `src/draw/draw_utils.rs` (new function, ~30 lines)

**Purpose**: Determine if we should draw full barline or just dots for this staff

**Logic**:
- If staff has `connStaff` link → part of a multi-staff group
  - Draw full barline only on leftmost/rightmost staff in group
  - Draw dots-only on middle staves
- Else: standalone staff → draw full barline

**Reference**: OG `DrawUtils.cp:1967` (ShouldREDrawBarline)

### Task 3d: Implement `draw_ending()`
**File**: `src/draw/draw_object.rs` (new function, ~60 lines)

**Entry point**: Called from `render_score()` when obj_type == ENDING

**Logic**:
1. Get ENDING data: firstObj, lastObj, endNum, no_l_cutoff, no_r_cutoff
2. Find x positions of first and last measures in bracket
3. Compute bracket height (1-1.5 spaces above staff)
4. Draw bracket:
   - Top horizontal line
   - Left vertical line (unless `no_l_cutoff`)
   - Right vertical line (unless `no_r_cutoff`)
5. Draw ending label text above bracket (look up endNum in table: "1.", "2.", etc.)

**Reference**: OG `DrawObject.cp:1389` (DrawENDING)

### Task 3e: Implement SMuFL glyph helpers
**File**: `src/music_font.rs` (extend existing)

**Add**:
- `RPTDOTS_GLYPH`: SMuFL repeat dots (U+E040)
- `get_rpt_dot_offset()`: Y offset for dots within staff space
- Fallback: use augmentation dot glyph if SMuFL unavailable

**Reference**: OG `MusCharXOffset()`, `MusCharYOffset()` in DrawUtils.cp

---

## Phase 4: Integration & Testing

### Task 4a: Add test harness
**File**: `tests/ngl_all.rs` (add new test)

```rust
#[test]
fn test_17_capital_regiment_march_rptend() {
    let path = "tests/fixtures/17_capital_regiment_march.ngl";
    let ngl = NglFile::read_from_file(path).expect("read");
    let score = interpret_heap(&ngl).expect("interpret");

    // Verify we found RPTEND and ENDING objects
    let rptends = score.find_by_type(RPTEND);
    let endings = score.find_by_type(ENDING);

    assert!(!rptends.is_empty(), "Should find RPTEND objects");
    assert!(!endings.is_empty(), "Should find ENDING objects");

    // Render and verify no panics
    let commands = render_score(&score, &config).expect("render");
    assert!(!commands.is_empty(), "Should produce render commands");
}
```

### Task 4b: Generate golden bitmap
**File**: `tests/golden_bitmaps/17_capital_regiment_march_page1.png`

```bash
REGENERATE_REFS=1 cargo test test_17_capital_regiment_march_rptend --features visual-regression
```

### Task 4c: Visual review against OG
**File**: `tests/fixtures/17_capital_regiment_march_reference.pdf`

Compare:
- Dot positions (should be exactly 1/8 space from staff lines)
- Barline weight and alignment
- Volta bracket height and text positioning
- Coda/Segno marker placement (if present)

---

## Implementation Order

1. **Start here**: Task 1a (ARPTEND unpacking) — blocks everything else
2. Task 1b (wire unpacking)
3. Task 1c (create InterpretedRptEnd)
4. Task 2a (ENDING unpacking)
5. Task 2b (InterpretedEnding struct)
6. Task 3a (draw_rptend entry point)
7. Task 3b (draw_rpt_bar helper)
8. Task 3c (should_re_draw_barline helper)
9. Task 3e (glyph helpers)
10. Task 3d (draw_ending)
11. Task 4a (test harness)
12. Task 4b (golden bitmap)
13. Task 4c (visual review)

---

## Key Files & Line References

| Purpose | File | Location |
|---------|------|----------|
| RPTEND struct | `Nightingale/src/Precomps/NObjTypes.h` | 149-156 |
| ARPTEND struct | `Nightingale/src/Precomps/NObjTypes.h` | 142-147 |
| ENDING struct | `Nightingale/src/Precomps/NObjTypes.h` | 795-806 |
| DrawRPTEND | `Nightingale/src/CFilesBoth/DrawObject.cp` | 1330-1381 |
| DrawENDING | `Nightingale/src/CFilesBoth/DrawObject.cp` | 1389-1497 |
| GetRptEndDrawInfo | `Nightingale/src/CFilesBoth/DrawUtils.cp` | 529-586 |
| DrawRptBar | `Nightingale/src/CFilesBoth/DrawUtils.cp` | 592-734 |
| ShouldREDrawBarline | `Nightingale/src/CFilesBoth/DrawUtils.cp` | 1967-2004 |
| Repeat subtypes | `Nightingale/src/Precomps/NObjTypes.h` | 158-166 |

---

## Estimated Effort

- **Unpacking (Phase 1-2)**: 2-3 hours (bitfield parsing, error handling)
- **Rendering (Phase 3)**: 4-5 hours (positioning logic, multi-staff grouping)
- **Testing & Polish (Phase 4)**: 1-2 hours

**Total**: ~8-10 hours of focused work

---

## Notes

- **SMuFL glyph**: Repeat dots are `U+E040` (rptDots) in SMuFL, but not all fonts have them. Fallback to augmentation dot or simple circle.
- **Mac68k alignment**: When unpacking bitfields from N105, watch for padding rules (see CLAUDE.md).
- **Coordinate system**: DDIST (1/16 point), must convert to render coordinates (inches or PDF points).
- **Staff grouping**: Multi-staff barlines (connStaff) are the trickiest part—test carefully with Capital Regiment March which likely has piano staff grouping.

---

**Next Session**: Start with Task 1a (ARPTEND unpacking). This is the critical path blocker.
