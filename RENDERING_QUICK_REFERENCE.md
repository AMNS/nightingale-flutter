# Nightingale Rendering Layer — Quick Reference

## File Locations
- **PS_Stdio.cp** (2,388 lines) — PostScript abstraction layer [PRIMARY]
- **DrawHighLevel.cp** (735 lines) — High-level drawing orchestration  
- **DrawObject.cp** (3,160 lines) — Object-specific drawing
- **DrawNRGR.cp** (2,252 lines) — Note/rest/grace note drawing
- **DrawUtils.cp** (2,626 lines) — Drawing utilities & font functions

## Coordinate Types
- **DDIST**: signed short, resolution 1/16 point
  - Range: ±2048 points (±28 inches)
  - Conversion: `d2pt(d) = (d+8)>>4`, `pt2d(p) = p<<16`
- **STDIST**: signed short, resolution 1/8 staff line
- **Point**: QuickDraw coordinates (72 DPI screen)

## PS_Stdio.cp Public API (27 functions)

### Line Drawing (6 functions)
```c
PS_Line(x0, y0, x1, y1, width)          /* General line, perpendicular thickness */
PS_LineVT(x0, y0, x1, y1, width)        /* Vertical thickening (beams) */
PS_LineHT(x0, y0, x1, y1, width)        /* Horizontal thickening */
PS_HDashedLine(x0, y, x1, width, dashLen)
PS_VDashedLine(x, y0, y1, width, dashLen)
PS_FrameRect(DRect*, width)
```

### Staff & Bars (6 functions)
```c
PS_StaffLine(height, x0, x1)                    /* Single line */
PS_Staff(height, x0, x1, nLines, *dy)           /* N-line staff */
PS_BarLine(top, bot, x, type)                   /* BAR_SINGLE/DOUBLE/FINALDBL */
PS_ConLine(top, bot, x)                         /* System connector line */
PS_LedgerLine(height, x0, dx)
PS_Repeat(doc, top, bot, botNorm, x, type, sizePercent, dotsOnly)
```

### Musical Elements (5 functions)
```c
PS_Beam(x0, y0, x1, y1, thick, upOrDown0, upOrDown1)
PS_Bracket(doc, x, yTop, yBot)          /* System bracket */
PS_Brace(doc, x, yTop, yBot)            /* System brace */
PS_Slur(p0x, p0y, c1x, c1y, c2x, c2y, p3x, p3y, dashed)
PS_NoteStem(doc, x, y, xNorm, sym, stemLen, stemShorten, beamed, headVisible, sizePercent)
```

### Characters & Strings (4 functions)
```c
PS_MusChar(doc, x, y, sym, visible, sizePercent)
PS_MusString(doc, x, y, str, sizePercent)
PS_FontString(doc, x, y, str, fontName, fontSize, fontStyle)
PS_MusColon(doc, x, y, sizePercent, lnSpace, italic)
```

### Configuration (4 functions)
```c
PS_SetLineWidth(width)
PS_SetWidths(staff, ledger, stem, bar)
PS_MusSize(doc, ptSize)
PS_PageSize(x, y)
```

### Page Management (2 functions)
```c
PS_NewPage(doc, page, n)
PS_EndPage()
```

## QuickDraw Direct Calls (4 most common)

```c
MoveTo(x, y)            /* Move pen to Point(x,y) in QuickDraw coordinates */
LineTo(x, y)            /* Draw line to Point(x,y) */
DrawChar(glyph)         /* Draw character at current pen position */
PaintRect(&rect)        /* Fill rectangle with current pattern */
```

Note: These appear primarily in DrawNRGR.cp for notehead/stem drawing and rest bars.

## DrawUtils.cp Utility Functions

```c
DrawMChar(doc, mchar, shape, dim)           /* Music font character */
DrawMString(doc, mstr, shape, dim)          /* Music font string (Pascal) */
DrawMColon(doc, italic, dim, lnSpace)       /* Colon from two dots */
DrawPaddedChar(ch)                          /* Character with padding (bug workaround) */

/* GetXxxxDrawInfo functions (20+ total) */
GetClefDrawInfo(doc, pL, aClefL, context, sizePercent, *glyph, *xd, *yd, *xdOct, *ydOct)
GetNoteDrawInfo(...)                        /* Similar pattern for all notation elements */
```

## Music Character Constants (from defs.h)

### Clefs
- `MCH_trebleclef` = '&'
- `MCH_cclef` = 'B'
- `MCH_bassclef` = '?'
- `MCH_percclef` = '/'

### Accidentals
- `MCH_sharp` = '#'
- `MCH_flat` = 'b'
- `MCH_natural` = 'n'

### Noteheads
- `MCH_breveNoteHead` = 0xDD
- `MCH_wholeNoteHead` = 'w'
- `MCH_halfNoteHead` = 'd'
- `MCH_quarterNoteHead` = 'n'

### Time Signatures
- `MCH_common` = 'c' (common time)
- `MCH_cut` = 'C' (cut time)

## Font Metrics Functions

```c
MapMusChar(musFontInfoIndex, logicalCode) → byte
    /* Sonata char code → physical glyph */

MusCharXOffset(musFontInfoIndex, glyph, lnSpace) → DDIST
    /* Horizontal positioning offset */

MusCharYOffset(musFontInfoIndex, glyph, lnSpace) → DDIST
    /* Vertical positioning offset */
```

## High-Level Drawing Functions (DrawHighLevel.cp)

```c
DrawRange(doc, fromL, toL, paper, updateRect)
    /* Main entry point for drawing object ranges */
    /* Calls SetMusicPort, then DrawScoreRange/DrawFormatRange/DrawMasterRange */

DrawScoreRange(doc, fromL, toL, context[], paper, updateRect)
    /* Draw normal score view */

DrawFormatRange(doc, fromL, toL, context[], paper, updateRect)
    /* Draw page layout mode */

DrawMasterRange(doc, fromL, toL, context[], paper, updateRect)
    /* Draw master page */

SetMusicPort(doc)
    /* Set font, size, build glyph cache before drawing */
```

## Object Drawing Functions (DrawObject.cp - sampler)

```c
DrawPAGE(doc, pL, paper, context[])        /* Page background, headers, footers */
DrawSYSTEM(doc, pL, paper, context[])      /* System with staves */
DrawSTAFF(doc, pL, paper, context[], ground, bMaster)
DrawMEASURE(doc, pL, context[])            /* Measure boundaries, barlines */
DrawCLEF(doc, pL, context[])               /* Clef symbol */
DrawKEYSIG(doc, pL, context[])             /* Key signature */
DrawTIMESIG(doc, pL, context[])            /* Time signature */
DrawSYNC(doc, pL, context[])               /* Note/rest chord */
DrawGRSYNC(doc, pL, context[])             /* Grace note chord */
DrawBEAMSET(doc, pL, context[])            /* Beam group */
DrawSLUR(doc, pL, context[])               /* Slur or tie */
DrawTUPLET(doc, pL, context[])             /* Tuplet (e.g., triplet) bracket */
```

## Coordinate Transformation Pattern

```c
/* Object has DDIST position relative to staff/measure */
DDIST xd = pL->xd;
DDIST yd = pL->yd;

/* Add staff/measure origin offset */
xd += pContext->staffLeft;
yd += pContext->staffTop;

/* Convert to QuickDraw points and adjust by paper position */
short xp = pContext->paper.left + d2p(xd);
short yp = pContext->paper.top + d2p(yd);

/* Position and draw */
MoveTo(xp, yp);
DrawChar(glyph);
```

## Key Data Structure: CONTEXT

```c
typedef struct {
    DDIST staffTop, staffLeft;          /* Staff origin in DDIST */
    DDIST measureTop, measureLeft;      /* Measure origin in DDIST */
    DDIST staffHeight;                  /* Staff height in DDIST */
    DDIST firstMeasTop;                 /* First measure in system */
    Rect paper;                         /* Paper bounding box (QuickDraw points) */
    /* ... ~30 more fields for clef, key sig, time sig, stem direction, etc. ... */
} CONTEXT;
```

## Porting Checklist

- [ ] **PS_Stdio.cp functions**: Convert PostScript emit calls to Rust struct building
- [ ] **Coordinate system**: Keep DDIST as primary, convert to display units in renderer
- [ ] **QuickDraw calls**: Replace MoveTo/LineTo/DrawChar with abstract render commands
- [ ] **Font system**: Port MapMusChar(), MusCharXOffset/YOffset() with SMuFL data
- [ ] **High-level orchestration**: Port DrawRange/DrawScoreRange logic to Rust
- [ ] **Object-specific drawing**: Port DrawSYNC, DrawCLEF, DrawBEAMSET, etc.
- [ ] **Flutter renderer**: Implement RenderCommand consumer in Dart/Flutter

## Testing Strategy

1. **PostScript validation**: Generate PostScript from ported PS_* functions, compare with original
2. **Coordinate accuracy**: Verify DDIST→display conversion matches original
3. **Glyph coverage**: Ensure all MCH_* characters map to SMuFL equivalents
4. **Visual regression**: Render known scores, compare with original Nightingale output

---

**Last updated:** February 23, 2026
