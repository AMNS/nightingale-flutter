# RPTEND and Volta Bracket (ENDING) Analysis

## Overview

Nightingale distinguishes between two related but separate object types:
1. **RPTEND** - Repeat End barlines (left/right/both repeat dots and barlines)
2. **ENDING** - Volta brackets / alternate endings (1st, 2nd endings, etc.)

The terms "volta bracket" and "ending" are used interchangeably; the technical object type is ENDING.

---

## 1. RPTEND: Repeat End Barlines

### Definition

RPTEND objects represent barline symbols with repeat dots (also called "repeat signs"). 
They mark repeat points in a score, similar to "D.C." (Da Capo) or "D.S." (Dal Segno) but 
using visual barline notation with dots.

### Data Structure

#### RPTEND (main object)
**File**: `/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/icebox/Nightingale/src/Precomps/NObjTypes.h:149-156`

```c
typedef struct {
    OBJECTHEADER                    /* xd, yd, selected, visible, etc. */
    LINK     firstObj;              /* Beginning of ending or NILINK */
    LINK     startRpt;              /* Repeat start point or NILINK */
    LINK     endRpt;                /* Repeat end point or NILINK */
    SignedByte subType;             /* Code from enum below */
    Byte     count;                 /* Number of times to repeat */
} RPTEND, *PRPTEND;
```

#### ARPTEND (subobject, per staff)
**File**: `NObjTypes.h:142-147`

```c
typedef struct {
    SUBOBJHEADER                    /* subType unused, staffn, selected */
    Byte     connAbove;             /* True if connected above */
    Byte     filler;                /* (unused) */
    SignedByte connStaff;           /* staff to connect to; valid if connAbove True */
} ARPTEND, *PARPTEND;
```

### Subtypes (repeat dot patterns)

**File**: `NObjTypes.h:158-166`

```c
enum {
    RPT_DC=1,           /* D.C. marking */
    RPT_DS,             /* D.S. (Dal Segno) marking */
    RPT_SEGNO1,         /* Segno #1 */
    RPT_SEGNO2,         /* Segno #2 */
    RPT_L,              /* Left repeat dots only (barline :|) */
    RPT_R,              /* Right repeat dots only (barline |:) */
    RPT_LR              /* Both left and right repeat dots (barline :|:) */
};
```

Note: The last three codes (RPT_L, RPT_R, RPT_LR) match the barline enum 
codes (BAR_RPT_L, BAR_RPT_R, BAR_RPT_LR) in MEASURE objects to maintain consistency.

### Data Domain

- **L domain** (Logical): `firstObj`, `startRpt`, `endRpt`, `count`
- **G domain** (Graphical): `xd` (from OBJECTHEADER), subobject `connAbove`, `connStaff`
- **P domain** (Performance): (unused for RPTEND)

---

## 2. ENDING: Volta Brackets (Alternate Endings)

### Definition

ENDING objects represent "volta" brackets or "alternate ending" brackets. These are the 
bracket symbols that appear above measures to indicate which ending a performer should 
take (1st ending, 2nd ending, etc.). Example: "1." above measures, "2." above different measures.

### Data Structure

#### ENDING (main object)
**File**: `NObjTypes.h:795-806`

```c
typedef struct {
    OBJECTHEADER                    /* xd, yd, selected, visible, etc. */
    EXTOBJHEADER                    /* staffn, selected, visible (extended) */
    LINK     firstObjL;             /* Object left end of ending is attached to */
    LINK     lastObjL;              /* Object right end of ending is attached to or NILINK */
    Byte     noLCutoff;             /* True to suppress cutoff at left end of Ending */
    Byte     noRCutoff;             /* True to suppress cutoff at right end of Ending */
    Byte     endNum;                /* 0=no ending number/label, else code for the ending label */
    DDIST    endxd;                 /* Position offset from lastObjL */
} ENDING, *PENDING;
```

No subobjects (unlike RPTEND which has ARPTEND subobjects).

### Data Fields

- **firstObjL**: The object on the left where the bracket attachment point begins
  - Typically a MEASURE or SYNC
  
- **lastObjL**: The object on the right where the bracket ends, or NILINK if bracket extends to end of system
  
- **noLCutoff**: Boolean flag; if True, suppress the vertical line on the left side of the bracket
  
- **noRCutoff**: Boolean flag; if True, suppress the vertical line on the right side of the bracket
  
- **endNum**: Ending number code (0=no label, 1=first code, etc.)
  - Value 1-31 indexes into endingString table
  - MAX_ENDING_STRLEN = 16 chars per label
  - MAX_ENDING_STRINGS = 31 labels
  
- **endxd**: Horizontal offset from lastObjL for right bracket position

- **xd, yd**: Position (from OBJECTHEADER) relative to the system
  
- **staffn**: Staff number this ending bracket is attached to (from EXTOBJHEADER)

### Data Domain

- **L domain** (Logical): `firstObjL`, `lastObjL`, `endNum`
- **G domain** (Graphical): `xd`, `yd`, `endxd`, `noLCutoff`, `noRCutoff`
- **P domain** (Performance): (unused for ENDING)

---

## 3. Rendering Functions

### RPTEND Rendering

#### Main Entry Point: DrawRPTEND()
**File**: `/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/icebox/Nightingale/src/CFilesBoth/DrawObject.cp:1330-1381`

```c
void DrawRPTEND(Document *doc, LINK pL, CONTEXT context[])
{
    /* For each ARPTEND subobject on each staff:
       1. Get draw info (position, barline widths for subType)
       2. Decide whether to draw full barline or dots-only
       3. Call DrawRptBar() with appropriate parameters
    */
}
```

**Logic flow**:
1. Iterate through all ARPTEND subobjects
2. Check visibility
3. Call `GetRptEndDrawInfo()` to compute position and bounds
4. Call `ShouldREDrawBarline()` to determine if barline + dots or dots-only
5. Call `DrawRptBar()` to render

#### Helper: GetRptEndDrawInfo()
**File**: `/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/icebox/Nightingale/src/Utilities/DrawUtils.cp:529-586`

Computes barline position and dimensions based on subType:

```c
switch (p->subType) {
    case RPT_L:   lWidth = 2; rWidth = 4;  /* |: barline */
    case RPT_R:   lWidth = 4; rWidth = 2;  /* :| barline */
    case RPT_LR:  lWidth = 4; rWidth = 4;  /* :|: barline */
}
```

- `lWidth`, `rWidth`: Pixel offsets for left/right dots relative to barline
- Bounds rectangle `rSub` encodes staff range to draw across (uses `connStaff`)

#### Helper: ShouldREDrawBarline()
**File**: `DrawUtils.cp:1967-2004`

Determines whether to draw the full barline or dots-only based on staff grouping:

```c
if (theRptEnd->connStaff!=0 || !theRptEnd->connAbove)
    /* Top staff of group or standalone: draw full barline */
    return True;
else
    /* Subordinate staff: draw dots-only unless all staves above are invisible */
    return False;
```

#### Core Drawing: DrawRptBar()
**File**: `DrawUtils.cp:592-734`

Renders the repeat barline and dots:

**Parameters**:
- `doc`: Document
- `pL`: RPTEND object link
- `staff`: Staff number to draw on
- `connStaff`: Staff to connect barline down to (0 = single staff)
- `context[]`: Context array
- `dLeft`: Horizontal position (DDIST)
- `subType`: RPT_L, RPT_R, or RPT_LR
- `mode`: MEDraw (main editing), SDDraw, SWDraw (other modes)
- `dotsOnly`: Boolean; if True, skip barline, draw dots only

**Rendering logic**:

1. **Barline positioning**:
   - If `connStaff == 0`: draw from `dTop` to `dTop + staffHeight`
   - Else: draw from `dTop` (top staff) to `dBottom` (connStaff's bottom)

2. **Repeat dots**:
   - Get repeat dot glyph from music font
   - Compute offset using `MusCharXOffset()`, `MusCharYOffset()`
   - Position depends on `subType`:
     - RPT_L: dots on right side of barline
     - RPT_R: dots on left side of barline
     - RPT_LR: dots on both sides

3. **Output modes**:
   - QuickDraw (screen/bitmap): MoveTo/LineTo for barlines, DrawChar for dots
   - PostScript: Call `PS_Repeat()` with computed parameters

---

### ENDING Rendering (Volta Brackets)

#### Main Entry Point: DrawENDING()
**File**: `DrawObject.cp:1389-1497`

Draws the volta bracket (1st/2nd/etc. ending bracket) above a staff.

**Logic**:
1. Get ENDING object `p`
2. Compute positions:
   - `xd`: Left position (from `firstObjL`)
   - `endxd`: Right position (from `lastObjL` + `endxd` offset)
   - `yd`: Vertical position above staff
3. Draw bracket:
   - Left vertical line (unless `noLCutoff`)
   - Horizontal line above staff
   - Right vertical line (unless `noRCutoff`)
4. Draw ending label (1st, 2nd, etc.) if `endNum != 0`

**Screen rendering** (QuickDraw):
```c
/* Left cutoff */
if (!p->noLCutoff) MOVE_AND_LINE(papLeft+xp, papTop+yp+risePxl, papLeft+xp, papTop+yp);

/* Horizontal line */
MOVE_AND_LINE(papLeft+xp, papTop+yp, papLeft+endxp, papTop+yp);

/* Right cutoff */
if (!p->noRCutoff) MOVE_AND_LINE(papLeft+endxp, papTop+yp, papLeft+endxp, papTop+yp+risePxl);

/* Label (if endNum != 0) */
if (endNum!=0 && endNum<maxEndingNum) {
    /* Draw text at (xdNum, ydNum) using sans-serif font */
}
```

**PostScript rendering**:
```c
endThick = ENDING_THICK(lnSpace);  /* Line thickness */
if (!p->noLCutoff) PS_Line(xd, yd+rise, xd, yd, endThick);
PS_Line(xd, yd, endxd, yd, endThick);
if (!p->noRCutoff) PS_Line(endxd, yd, endxd, yd+rise, endThick);
/* Label via PS_FontString if endNum != 0 */
```

---

## 4. Key Positioning and Layout Rules

### RPTEND Positioning

1. **Horizontal (xd)**: At the barline position in the measure
2. **Vertical (yd)**: At staff top (computed in CONTEXT)
3. **Vertical extent**:
   - Single staff: from staff top to bottom
   - Connected group: from top staff to bottom of connStaff
4. **Dot positioning**: 
   - Offset by `MusCharXOffset()` and `MusCharYOffset()` from music font metrics
   - Y-position: staff bottom minus space for centered dots

### ENDING Positioning

1. **Horizontal**:
   - Left (xd): Position relative to firstObjL (measure/sync start)
   - Right (endxd): Position relative to lastObjL
   - If lastObjL is NILINK, extends to system right edge
   
2. **Vertical**:
   - yd: Above staff, typically 2-3 line spaces above top staff line
   - rise: Vertical cutoff length (typically 1 line space)
   - ydNum: Label position, typically 2 line spaces above top of bracket

3. **Line thickness**:
   - ENDING_THICK(lnSpace): proportional to line space
   - ENDING_CUTOFFLEN(lnSpace): proportional to line space

4. **Label**:
   - Font: Sans-serif on screen, Times on PostScript
   - Size: derived from line space (fontSize = d2pt(2*lnSpace - 1))
   - String: indexed from endingString table via endNum code

---

## 5. Constants and Macros

**From NLimits.h**:
```c
#define MAX_ENDING_STRLEN 16      /* Maximum length of any ending label */
#define MAX_ENDING_STRINGS 31     /* Maximum number of ending labels */
```

**From DrawUtils.cp and context**:
```c
#define LNSPACE(pContext)         pContext->instrSpaceBelow  /* Line space in DDIST */
#define ENDING_CUTOFFLEN(lnSpace) (lnSpace)                  /* Vertical cutoff length */
#define ENDING_THICK(lnSpace)     ???                        /* Line thickness (not explicitly defined) */
```

---

## 6. File Locations and Line Numbers

### Header Definitions
- `NObjTypes.h`: Type definitions, enums
  - RPTEND: lines 140-166
  - ENDING: lines 795-806
  - ARPTEND: lines 142-147

- `NObjTypesN105.h`: N105 format versions
  - RPTEND_5: lines 113-120
  - ARPTEND_5: lines 107-111
  - ENDING_5: lines 547-556

- `NBasicTypes.h`: Object type enum
  - RPTENDtype: line 102

- `Draw.h`: Function declarations
  - DrawRPTEND: line 41
  - DrawENDING: (not shown, but in DrawObject.cp)

- `DrawUtils.h`: Helper declarations
  - DrawRptBar: line 20
  - GetRptEndDrawInfo: line 21
  - ShouldREDrawBarline: line 54

### Implementation
- `DrawObject.cp`:
  - DrawRPTEND: lines 1330-1381
  - DrawENDING: lines 1389-1497

- `DrawUtils.cp`:
  - GetRptEndDrawInfo: lines 529-586
  - DrawRptBar: lines 592-734
  - ShouldREDrawBarline: lines 1967-2004

### Memory Macros
- `MemMacros.h`:
  - RptEndSTAFF, RptEndSEL, RptType: lines 96-147
  - GetPRPTEND, GetPARPTEND, NextRPTENDL: lines 229-300

---

## 7. Special Notes

### Staff Grouping and Barline Connections

Both RPTEND (via ARPTEND) and MEASURE (via AMEASURE) support "barline grouping" where
a single visual barline connects multiple staves:

- **connAbove**: Boolean; if True, this staff's barline is part of a group and connects above
- **connStaff**: Staff number to connect to (only valid if !connAbove or connStaff > 0)

**Logic**: 
- If a staff is the "top" of a group (connStaff != 0 or !connAbove), the barline is drawn across
  all staves in that group
- If a staff is "subordinate" (connAbove && connStaff == 0), only dots are drawn unless all
  staves above it in the group are invisible

### N105 Struct Alignment

The N105 file format uses `#pragma options align=mac68k`, which pads all structs to 2-byte
boundaries. This affects binary file I/O but not the runtime C struct layout in memory.

### Glyph Metrics

Both RPTEND and ENDING positioning relies heavily on music font metrics:
- `MusCharXOffset()`: Horizontal offset from glyph origin
- `MusCharYOffset()`: Vertical offset from glyph origin
- `MusFontHasRepeatDots()`: Whether font has native repeat dot glyph or we must use augmentation dots

---

## Summary

- **RPTEND**: Barlines with repeat dots (:|, |:, :|:) + optional D.C./D.S./Segno markers
  - Has subobjects (ARPTEND) per staff
  - Supports multi-staff barline grouping
  - Rendered via DrawRptBar with dot glyphs from music font

- **ENDING**: Volta brackets for alternate endings (1st, 2nd, etc.)
  - No subobjects; single bracket per object
  - Attached to measures via firstObjL/lastObjL links
  - Rendered as bracket lines + optional text label
  - Supports optional cutoff suppression on left/right

Both are graphical elements that enhance musical structure without affecting playback semantics.
