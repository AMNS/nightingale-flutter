# Nightingale C++ Drawing Pipeline - Comprehensive Analysis

## Executive Summary

The Nightingale drawing pipeline is a recursive descent through the object hierarchy (PAGE → SYSTEM → STAFF → MEASURE → SYNC → NOTE) with context-based coordinate transformation. The key principle: **DDIST (1/16 point) data from the score model is converted to pixels at draw time** through the `d2p()` macro, which uses magnification-dependent rounding.

---

## Coordinate System

### Core Types

- **DDIST** (signed short): Data model coordinates. Range: ±2048 points, resolution: 1/16 point
- **Pixel space**: QuickDraw screen/print coordinates (int)
- **Points**: 1/72 inch = 1/16 DDIST (for print/export)
- **STDIST** (signed short): Staff-relative coordinates. Range: ±4096 staff lines, resolution: 1/8 staff line

### Transformation Macros (from defs.h line 572-579)

```c
d2p(d)    → D2PFunc(d)     // DDIST to pixels (with rounding)
d2px(d)   → D2PXFunc(d)    // DDIST to pixels (truncating)
p2d(p)    → P2DFunc(p)     // pixels to DDIST
d2pt(d)   → (((d)+8)>>4)   // DDIST to points (divide by 16 + round)
pt2d(p)   → ((p)<<4)       // points to DDIST (multiply by 16)
```

### D2PFunc Implementation (Magnify.cp line 49-66)

Converts DDIST to pixels considering magnification level:
- Even magnification (100%, 50%, etc.): simple shift by magShift
- Odd magnification (150%, 75%, etc.): multiply by 1.5 after shifting (uses magRound for proper rounding)

**Key insight**: Magnification affects the bit-shift amount (`magShift`), allowing seamless scaling without floating point.

---

## CONTEXT Structure (NObjTypes.h line 871-899)

The rendering context passed to all drawing functions. Tracks position, visibility, clef, key sig, time sig:

```c
typedef struct {
    Boolean     visible;           // (staffVisible && measureVisible)
    Boolean     staffVisible;      // staff is visible
    Boolean     measureVisible;    // measure is visible
    Boolean     inMeasure;         // True if currently in measure
    Rect        paper;             // Sheet rect in window coordinates
    short       sheetNum;          // PAGE number
    short       systemNum;         // SYSTEM number
    
    // SYSTEM-level positions (page-relative, DDIST)
    DDIST       systemTop;         
    DDIST       systemLeft;        
    DDIST       systemBottom;      
    
    // STAFF-level positions (page-relative, DDIST)
    DDIST       staffTop;          
    DDIST       staffLeft;         
    DDIST       staffRight;        
    DDIST       staffHeight;       
    DDIST       staffHalfHeight;   // staffHeight >> 1
    
    // STAFF layout
    SignedByte  staffLines;        // usually 5
    SignedByte  showLines;         // 0, 1, or SHOW_ALL_LINES
    Boolean     showLedgers;       
    short       fontSize;          // music font size in points
    
    // MEASURE-level positions (page-relative, DDIST)
    DDIST       measureTop;        
    DDIST       measureLeft;       
    
    // Current state (updated as we draw)
    SignedByte  clefType;          
    SignedByte  dynamicType;       
    WHOLE_KSINFO;                  // key signature
    SignedByte  timeSigType, numerator, denominator;
} CONTEXT, *PCONTEXT;
```

**Helper macro**: `LNSPACE(pCont)` = `pCont->staffHeight / (pCont->staffLines - 1)` = distance in DDIST between staff lines.

---

## Drawing Pipeline: Top Level

### DrawRange() — DrawHighLevel.cp:709

**Purpose**: Main entry point for drawing a range of objects [fromL, toL).

**Flow**:
1. Call `SetMusicPort(doc)` to set font, size, build charRect cache, install magnification
2. Call `GetAllContexts(doc, context, fromL)` if first object is not PAGE (PAGE calls this internally)
3. Branch on doc state:
   - `doc->masterView` → `DrawMasterRange()`
   - `doc->showFormat` → `DrawFormatRange()`
   - else → `DrawScoreRange()` + `HiliteScoreRange()` for selection

### DrawScoreRange() — DrawHighLevel.cp:349

**Purpose**: Draw normal score in the specified range.

**Iteration**: Walks fromL→toL via `RightLINK(pL)`, dispatches on object type.

**Key Objects Drawn**:

| Type | Handler | Notes |
|------|---------|-------|
| PAGE | DrawPAGE() | Sets up paper rect, updates all contexts |
| SYSTEM | DrawSYSTEM() | Checks visibility, frames if debug |
| STAFF | DrawSTAFF() | Iterates substaves, updates context |
| CONNECT | DrawCONNECT() | Bracket/brace connector lines |
| MEASURE | DrawMEASURE() | Barlines, measure numbers |
| SYNC | DrawSYNC() | Notes & rests (if visible & drawAll) |
| GRSYNC | DrawGRSYNC() | Grace notes |
| BEAMSET | DrawBEAMSET() / DrawGRBEAMSET() | Beam lines |
| TUPLET | DrawTUPLET() | Tuplet brackets |
| SLUR | DrawSLUR() | Slur curves |
| DYNAMIC | DrawDYNAMIC() | Dynamics marks |
| TEMPO | DrawTEMPO() | Tempo marks |
| GRAPHIC | DrawGRAPHIC() | User graphics |
| etc. | ... | Clef, key sig, time sig, etc. |

**Critical Variable**: `drawAll` (bool)
- If TRUE: draw all objects in measure
- If FALSE: draw only "spanning" objects (those crossing measure boundaries)
- Set based on intersection of measure's measureBBox with paperUpdate rect

---

## Function-by-Function Deep Dive

### 1. DrawSTAFF() — DrawObject.cp:591

**Signature**:
```c
void DrawSTAFF(Document *doc, LINK pL, Rect *paper,
                CONTEXT context[], 
                short ground,      // TOPSYS_STAFF, BACKGROUND_STAFF, etc.
                short hilite       // Boolean: should selected state be hilited?
                )
```

**What It Does**:
- Iterates over substaves (FirstSubLINK → NextSTAFFL)
- For each substaff, updates context[staffn] with staff-specific values:
  - `pContext->staffTop = aStaff->staffTop + pContext->systemTop` (absolute position)
  - `pContext->staffLeft = aStaff->staffLeft + pContext->systemLeft`
  - `pContext->staffHeight`, `pContext->staffLines`, `pContext->fontSize`
  - Visibility flags
- Calls `Draw1Staff(doc, staffn, paper, pContext, ground)` to draw actual staff lines
- Optional: Draw part names (instrument names) via DrawPartName()
- Optional: Hilite if selected

**Drawing Calls**: None directly; delegates to Draw1Staff() and DrawPartName()

**Coordinate Transformation**:
- Input: aStaff→staffTop/Left (DDIST, relative to system)
- Processing: Add systemTop/Left to get absolute page-relative DDIST
- Output: Stored in context[staffn] for child objects to use

---

### 2. DrawCONNECT() — DrawObject.cp:670

**Signature**:
```c
void DrawCONNECT(Document *doc, LINK pL,
                CONTEXT context[],
                short ground       // TOPSYS_STAFF, BACKGROUND_STAFF, etc.
                )
```

**What It Does**:
- Iterates over subobjects (FirstSubLINK → NextCONNECTL)
- For each connection (bracket, brace, or line):
  - Determines staff range (staffAbove, staffBelow)
  - Gets context for top and bottom staves
  - Computes dTop (top staff top), dBottom (bottom staff bottom), xd (left edge)
  - Calls draw function based on connectType

**Drawing Calls**:

| Type | Call | Coordinates |
|------|------|-------------|
| CONNECTLINE | MoveTo(px, pyTop); LineTo(px, pyBot) | QuickDraw: px=paper.left+d2p(xd), pyTop/pyBot in pixels |
| | PS_ConLine(dTop, dBottom, xd) | PostScript: DDIST coords |
| CONNECTCURLY | DrawPict() or PS_MusChar() | Brace from resource or PostScript |
| CONNECTBRACKET | Sonata character via DrawMChar() | Bracket symbol |

**Key Code Pattern**:
```c
dLeft = pContext->staffLeft;
dTop = pContext->staffTop;              // top staff top (DDIST)
pContext = &context[stfB];
dBottom = pContext->staffTop + pContext->staffHeight;  // bottom staff bottom
xd = dLeft + aConnect->xd;

px = pContext->paper.left + d2p(xd);
pyTop = pContext->paper.top + d2p(dTop);
pyBot = pContext->paper.top + d2p(dBottom);
MoveTo(px, pyTop); 
LineTo(px, pyBot);
```

---

### 3. DrawMEASURE() — DrawObject.cp:2772

**Signature**:
```c
void DrawMEASURE(Document *doc, LINK pL, CONTEXT context[])
```

**What It Does**:
- Iterates over substaves (FirstSubLINK → NextMEASUREL)
- For each measure (one per staff):
  - Updates context[staff] with measure-level positions:
    - `pContext->measureTop = pContext->staffTop` (or adjusted if single line shown)
    - `pContext->measureLeft = pContext->staffLeft + LinkXD(pL)` (absolute position)
  - Copies clef, key sig, time sig, dynamic info into context
  - Calls `ShouldDrawBarline()` to decide if barline should draw
  - Draws barlines via DrawBarline() or DrawRptBar()
  - Draws measure number if applicable via DrawMeasNum()
  - Updates objRect and bBox for hit-testing

**Drawing Calls**:

| Function | Purpose |
|----------|---------|
| DrawBarline() | Single, double, final barline |
| DrawRptBar() | Repeat barlines |
| DrawMeasNum() | Measure number at top/bottom |

**Coordinate Logic**:
```c
dTop = pContext->measureTop = pContext->staffTop;
dLeft = pContext->measureLeft = pContext->staffLeft + LinkXD(pL);
pContext->inMeasure = True;

// For measure number positioning:
xdMN = dLeft + halfLn2d(doc->xMNOffset, ...);
ydMN = dTop + halfLn2d(yOffset, ...);  // above or below measure
```

**Important**: measureBBox is **absolute page-relative DDIST** rect, used for clipping/visibility checks.

---

### 4. DrawSYNC() — DrawNRGR.cp:1509

**Signature**:
```c
void DrawSYNC(Document *doc, LINK pL, CONTEXT context[])
```

**What It Does**:
- Iterates over subnotes (FirstSubLINK → NextNOTEL)
- For each note:
  - Gets `pContext = &context[NoteSTAFF(aNoteL)]`
  - Calls `MaySetPSMusSize(doc, pContext)` for PostScript font sizing
  - Branches: if rest → `DrawRest()`, else → `DrawNote()`
- If LinkSEL(pL) and selected: calls `CheckSYNC()` for hiliting
- Shows sync info if debugging

**Drawing Calls**: None directly; delegates to DrawNote() or DrawRest()

**Context Usage**: Accesses context[NoteSTAFF(aNoteL)] for each note's staff

---

### 5. DrawNote() — DrawNRGR.cp:662

**Signature**:
```c
void DrawNote(Document *doc,
              LINK pL,               // SYNC this note belongs to
              PCONTEXT pContext,     // Context for this note's staff
              LINK aNoteL,           // Subobject (note) to draw
              Boolean *drawn,        // False until a subobject has been drawn
              Boolean *recalc        // True if we need to recompute objRect
              )
```

**What It Does**: Draw a single note (notehead, stem, flags, accidental, ledger lines, dots).

**Coordinate Setup**:
```c
staffn = NoteSTAFF(aNoteL);
dLeft = pContext->measureLeft + LinkXD(pL);           // measure left (absolute DDIST)
dTop = pContext->measureTop;                           // staff top (absolute DDIST)
lnSpace = LNSPACE(pContext);                           // distance between lines (DDIST)
dhalfSp = lnSpace / 2;                                 // half-space (DDIST)

xd = NoteXLoc(pL, aNoteL, pContext->measureLeft, HeadWidth(lnSpace), &xdNorm);
aNote = GetPANOTE(aNoteL);
yd = dTop + aNote->yd;                                 // absolute Y position (DDIST)
```

**Subfunction Calls** (in order):

| Function | Purpose | Key Coordinates |
|----------|---------|-----------------|
| DrawAcc() | Accidental | xdNorm, yd (DDIST) |
| DrawNCLedgers() | Ledger lines | xd, dTop (DDIST) |
| DrawNotehead() | Notehead symbol | xhead, yhead (pixels after d2p) |
| [Stem] | Stem line | via Move/Line at xhead, yhead |
| [Flags] | Flag symbols | xhead, ypStem (pixels) |
| DrawAugDots() | Augmentation dots | xdNorm, yd (DDIST) |

**Screen Drawing (toScreen/toBitmapPrint/toPICT)** — lines 748-900:

```c
// Convert absolute DDIST to window pixels
yOffset = MusCharYOffset(doc->musFontInfoIndex, glyph, lnSpace);
if (yOffset) {
    xhead = pContext->paper.left + d2p(xd);
    xadjhead = pContext->paper.left + d2p(xd + yOffset);
} else
    xhead = xadjhead = pContext->paper.left + d2p(xd);

yOffset = MusCharYOffset(doc->musFontInfoIndex, glyph, lnSpace);
if (yOffset) {
    yp = pContext->paper.top + d2p(yd);
    yhead = yp + fudgeHeadY + d2p(breveFudgeHeadY);
    yp = pContext->paper.top + d2p(yd + yOffset);
    yadjhead = yp + fudgeHeadY + d2p(breveFudgeHeadY);
} else {
    yp = pContext->paper.top + d2p(yd);
    yhead = yadjhead = yp + fudgeHeadY + d2p(breveFudgeHeadY);
}

TextSize(useTxSize);
ForeColor(Voice2Color(doc, aNote->voice));
if (dim) PenPat(NGetQDGlobalsGray());

MoveTo(xadjhead, yadjhead);
DrawNotehead(doc, glyph, appearance, dim, dhalfSp);
```

**Stem Drawing** (if not whole/breve):
```c
if (MainNote(aNoteL)) {
    stemDown = (dStemLen > 0);
    stemSpace = (stemDown ? 0 : MusFontStemSpaceWidthPixels(...));
    MoveTo(xhead + stemSpace, yhead);
    Move(0, -1);
    Line(0, d2p(dStemLen) + (stemDown ? 0 : 1));  // ← pixel-space stem
}
```

**PostScript Drawing** (toPostScript) — uses DDIST throughout:
```c
PS_MusChar(doc, xd, yd, glyph, ...);      // ← DDIST, not pixels
```

**Visibility/Dimming Logic**:
```c
dim = (outputTo == toScreen && !LOOKING_AT(doc, aNote->voice));
ForeColor(Voice2Color(doc, aNote->voice));
if (dim) PenPat(NGetQDGlobalsGray());      // Gray out non-active voices
```

---

### 6. DrawAcc() — DrawNRGR.cp:314

**Signature**:
```c
void DrawAcc(Document *doc,
              PCONTEXT pContext,
              LINK theNoteL,              // Note to draw accidental for
              DDIST xdNorm, DDIST yd,     // Notehead position (excluding otherStemSide effect)
              Boolean dim,                // Dim if voice not active
              short sizePercent,          // 100 for normal, <100 for small notes
              Boolean chordNoteToL        // Note in chord downstemmed w/ notes to left?
              )
```

**What It Does**: Draw accidental symbol (and optional courtesy parentheses).

**Coordinate Computation**:
```c
theNote = GetPANOTE(theNoteL);
if (theNote->accident == 0) return;     // No accidental

accGlyph = MapMusChar(doc->musFontInfoIndex, SonataAcc[theNote->accident]);
lnSpace = LNSPACE(pContext);
d8thSp = lnSpace / 8;

// Adjust for chord context
if (chordNoteToL) 
    xdNorm -= SizePercentSCALE(HeadWidth(LNSPACE(pContext)));

// Compute accidental X offset
xmoveAcc = (theNote->accident == AC_DBLFLAT ? theNote->xmoveAcc + 2 : theNote->xmoveAcc);
accXOffset = SizePercentSCALE(AccXDOffset(xmoveAcc, pContext));
xdAcc = xdNorm - accXOffset;

// If courtesy accidental, add parentheses
if (theNote->courtesyAcc) {
    scalePercent = (config.courtesyAccPSize * sizePercent) / 100;
    xdRParen = xdAcc + MusCharXOffset(...);
    deltaXR = (scalePercent * config.courtesyAccRXD) / 100;
    xdAcc = xdRParen - deltaXR * d8thSp;
    deltaXL = (scalePercent * config.courtesyAccLXD) / 100;
    xdLParen = xdAcc - deltaXL * d8thSp;
    deltaY = (scalePercent * config.courtesyAccYD) / 100;
    ydParens = yd + deltaY * d8thSp + MusCharYOffset(...);
}

// Apply font-specific offsets
xdAcc += SizePercentSCALE(MusCharXOffset(doc->musFontInfoIndex, accGlyph, lnSpace));
yd += SizePercentSCALE(MusCharYOffset(doc->musFontInfoIndex, accGlyph, lnSpace));
```

**Drawing** (QuickDraw):
```c
xp = pContext->paper.left + d2p(xdAcc);
yp = pContext->paper.top + d2p(yd);
MoveTo(xp, yp);
DrawMChar(doc, accGlyph, NORMAL_VIS, dim);

if (theNote->courtesyAcc) {
    yp = pContext->paper.top + d2p(ydParens);
    xp = pContext->paper.left + d2p(xdLParen);
    MoveTo(xp, yp);
    DrawMChar(doc, lParenGlyph, NORMAL_VIS, dim);
    xp = pContext->paper.left + d2p(xdRParen);
    MoveTo(xp, yp);
    DrawMChar(doc, rParenGlyph, NORMAL_VIS, dim);
}
```

**Drawing** (PostScript):
```c
PS_MusChar(doc, xdAcc, yd, accGlyph, True, sizePercent);
if (theNote->courtesyAcc) {
    PS_MusChar(doc, xdLParen, ydParens, lParenGlyph, True, scalePercent);
    PS_MusChar(doc, xdRParen, ydParens, rParenGlyph, True, scalePercent);
}
```

---

### 7. DrawNCLedgers() — DrawNRGR.cp:435

**Signature**:
```c
static void DrawNCLedgers(
    LINK syncL,
    PCONTEXT pContext,
    LINK aNoteL,
    DDIST xd,              // X position of notehead (DDIST)
    DDIST dTop,            // Staff top (DDIST)
    short ledgerSizePct    // 100 for normal size
    )
```

**What It Does**: Draw ledger lines above/below staff as needed.

**Logic**:
```c
if (!pContext->showLedgers) return;

stemDown = (NoteYSTEM(aNoteL) > NoteYD(aNoteL));

GetNCLedgerInfo(syncL, aNoteL, pContext, &hiyqpit, &lowyqpit, &hiyqpitSus, &lowyqpitSus);

if (NoteINCHORD(aNoteL)) {
    DrawNoteLedgers(xd, hiyqpit, hiyqpitSus, stemDown, dTop, pContext, ledgerSizePct);
    DrawNoteLedgers(xd, lowyqpit, lowyqpitSus, stemDown, dTop, pContext, ledgerSizePct);
} else {
    yqpit = (NoteYD(aNoteL) < 0 ? hiyqpit : lowyqpit);
    DrawNoteLedgers(xd, yqpit, 0, stemDown, dTop, pContext, ledgerSizePct);
}
```

**Delegates to**: DrawNoteLedgers() (which draws actual lines via LineTo/PS_Line)

---

### 8. DrawAugDots() — DrawNRGR.cp:251

**Signature**:
```c
static void DrawAugDots(Document *doc,
                        LINK theNoteL,           // Note/rest to draw dots for
                        DDIST xdNorm, DDIST yd,  // Notehead position
                        PCONTEXT pContext,
                        Boolean chordNoteToR     // Chord note upstemmed w/ notes to right?
                        )
```

**What It Does**: Draw augmentation dots (rhythmic dots).

**Coordinate Computation**:
```c
theNote = GetPANOTE(theNoteL);
if (theNote->ndots == 0 || theNote->yMoveDots == 0) return;

dhalfSp = LNSPACE(pContext) / 2;
doNoteheadGraphs = (doc->graphMode == GRAPHMODE_NHGRAPHS);
ndWidth = AugDotXDOffset(theNoteL, pContext, chordNoteToR, doNoteheadGraphs, ...);
xdDots = xdNorm + ndWidth;
ydDots = yd + (theNote->yMoveDots - 2) * dhalfSp;

ndots = theNote->ndots;
dim = (outputTo == toScreen && !LOOKING_AT(doc, theNote->voice));

glyph = MapMusChar(doc->musFontInfoIndex, MCH_dot);
xdDots += MusCharXOffset(doc->musFontInfoIndex, glyph, dhalfSp * 2);
ydDots += MusCharYOffset(doc->musFontInfoIndex, glyph, dhalfSp * 2);
```

**Drawing** (QuickDraw — loops):
```c
while (ndots > 0) {
    xdDots += 2 * dhalfSp;                          // Space between dots
    xpDots = pContext->paper.left + d2p(xdDots);
    yp = pContext->paper.top + d2p(ydDots);
    MoveTo(xpDots, yp);
    DrawMChar(doc, glyph, NORMAL_VIS, dim);
    ndots--;
}
```

**Drawing** (PostScript — loops):
```c
if (doc->srastral == 7) ydDots += pt2d(1) / 8;  // Empirical correction
while (ndots > 0) {
    xdDots += 2 * dhalfSp;
    PS_MusChar(doc, xdDots, ydDots, glyph, True, 100);
    ndots--;
}
```

---

## Key Insights for Porting

### 1. **Two Parallel Drawing Paths**: Screen vs. PostScript
   - Screen: QuickDraw (`MoveTo`, `LineTo`, `DrawChar`), uses **pixel** coordinates
   - PostScript: PDF/EPS output, uses **DDIST** coordinates
   - Must preserve this in rendering abstraction

### 2. **Context Propagation**
   - Context is built once per page (via `GetAllContexts()` and recursive `ContextXXX()` calls)
   - Updated incrementally as we traverse objects (e.g., measureLeft set at each MEASURE)
   - **Stateful**: changes accumulate as we descend the tree
   - For Rust: make Context a mutable reference parameter

### 3. **Coordinate Transformations Happen at Draw Time**
   - Data model stores DDIST (resolution: 1/16 point)
   - At screen draw: `pixel = paper.left + d2p(ddist)` — depends on magnification
   - At PS draw: use DDIST directly
   - **Critical**: d2p() is nonlinear with magnification (odd mags multiply by 1.5)

### 4. **Visibility/Clipping Optimization**
   - Uses `SectRect()` to check if object bBox intersects updateRect
   - If no intersection: skips drawing (unless outputTo != toScreen)
   - `drawAll` boolean reduces work for measure-spanning objects

### 5. **Selection Hiliting is Deferred**
   - DrawScoreRange() draws everything opaque
   - HiliteScoreRange() redraws selected objects with hiliting (CheckXXX functions)
   - Allows clean separation of structure from selection state

### 6. **Color and Dimming**
   - Voice-based coloring: `ForeColor(Voice2Color(doc, aNote->voice))`
   - Dimming for inactive voices: `PenPat(NGetQDGlobalsGray())`
   - These are QuickDraw calls — Rust must provide rendering-agnostic equivalents

### 7. **Glyph Positioning is Font-Aware**
   - `MusCharXOffset()` / `MusCharYOffset()` provide per-font fine-tuning
   - `SizePercentSCALE()` handles small notes and courtesy accidentals
   - These must be ported for each font being used (SMuFL)

### 8. **Chords Need Special Handling**
   - Notes in chords may share stems or have special positioning
   - `ChordNoteToLeft()` / `ChordNoteToRight()` determine stem side
   - Accidental and dot positioning depend on chord context

---

## Function Call Hierarchy

```
DrawRange()
├─ DrawScoreRange()
│  ├─ DrawPAGE()
│  ├─ DrawSYSTEM()
│  ├─ DrawSTAFF()
│  │  ├─ Draw1Staff()
│  │  └─ DrawPartName()
│  ├─ DrawCONNECT()
│  ├─ DrawMEASURE()
│  │  ├─ DrawBarline()
│  │  └─ DrawMeasNum()
│  ├─ DrawSYNC()
│  │  ├─ DrawNote()
│  │  │  ├─ DrawAcc()
│  │  │  ├─ DrawNCLedgers()
│  │  │  │  └─ DrawNoteLedgers()
│  │  │  ├─ DrawNotehead()
│  │  │  └─ DrawAugDots()
│  │  └─ DrawRest()
│  ├─ DrawBEAMSET()
│  ├─ DrawTUPLET()
│  ├─ DrawSLUR()
│  └─ ... (other object types)
└─ HiliteScoreRange()
   └─ CheckXXX() (selection hiliting)
```

---

## QuickDraw Calls by Category

### Coordinate Movement
- `MoveTo(x, y)` — move pen to (x, y)
- `Move(dx, dy)` — move pen relative
- `LineTo(x, y)` — draw line from current to (x, y)
- `Line(dx, dy)` — draw line relative

### Text/Glyph
- `DrawMChar(doc, glyph, appearance, dim)` — draw music character
- `TextSize(size)` — set font size in points
- `TextFont(fontNum)` — set font

### Color/Style
- `ForeColor(color)` — set pen color
- `PenPat(pattern)` — set pen pattern (for dimming: gray pattern)

### Geometry
- `FrameRect(r)` — frame a rectangle (debug)

---

## PostScript Calls (PS_*.h)

- `PS_MusChar(doc, xd, yd, glyph, ..., sizePercent)` — draw music character (DDIST coords)
- `PS_ConLine(dTop, dBottom, xd)` — draw connector line
- `PS_Line(x1, y1, x2, y2)` — draw line (DDIST)
- `PS_Staff(...)` — draw staff lines

These take DDIST directly, no d2p() conversion needed.

---

## Porting Strategy

### Phase 1: Data Structures
Port CONTEXT, DDIST, PCONTEXT structures to Rust. Keep DDIST as i16.

### Phase 2: Coordinate Transforms
Implement d2p(), d2px(), p2d() with magnification support.

### Phase 3: Rendering Trait
Define abstract trait:
```rust
trait RenderingBackend {
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn draw_glyph(&mut self, glyph: u8, appearance: u8, dim: bool);
    fn set_color(&mut self, color: RGB);
    fn set_line_width(&mut self, width: f32);
}
```

### Phase 4: Drawing Functions
Port DrawNote, DrawAcc, DrawSTAFF, etc. to Rust, using rendering trait instead of QuickDraw.

### Phase 5: Integration
Feed Flutter a list of drawing commands (glyph, position, color, size).

