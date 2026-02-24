# Nightingale Music Notation Rendering Rules - Complete Extraction

## Document Reference Information
**Source Repository**: /Users/chirgwin/Nightingale-Phoenix/nightingale-modernize/Nightingale/
**Key Drawing Files**:
- DrawNRGR.cp (lines 1-2200) — Main note/rest rendering
- PS_Stdio.cp (lines 1650-1735) — PostScript stem rendering
- DrawObject.cp — General drawing infrastructure  
- Utilities/Utility.cp (lines 58-210) — Stem calculation algorithms
- Precomps/style.h — Styling constants
- Precomps/defs.h — Core macros and definitions

---

## PART 1: COORDINATE SYSTEMS AND UNITS

### Core Units
All coordinates use **DDIST** (1/16 point resolution, 16 DDIST = 1 point):
```c
DDIST    = short     /* range ±2048 points, resolution 1/16 point */
STDIST   = short     /* range ±4096 staffLines, 1/8 line resolution */
SHORTQD  = signed_byte /* range ±32 staffLines, 1/4 line resolution */
STD_LINEHT = 8       /* STDIST units per staff interline space */
```

### Coordinate Conversions
From `defs.h` line 572-579:
```c
#define d2p(d)      D2PFunc(d)           /* DDIST to pixels */
#define d2px(d)     D2PXFunc(d)          /* DDIST to pixels (truncated) */
#define d2pt(d)     (((d)+8)>>4)         /* DDIST to points */
#define pt2d(p)     ((p)<<4)             /* points to DDIST */
#define p2pt(p)     d2pt(p2d(p))         /* pixels to points */
#define pt2p(p)     d2p(pt2d(p))         /* points to pixels */
```

### Staff Line Space Calculations
From `defs.h` line 452:
```c
#define LNSPACE(pCont) ((pCont)->staffHeight/((pCont)->staffLines-1))
/* Converts LNSPACE (line space in DDIST) to half-spaces */
#define halfLn2std(halfLn) ((STD_LINEHT/2)*(halfLn))
#define std2halfLn(std)    ((std)/(STD_LINEHT/2))
```

---

## PART 2: NOTEHEAD POSITIONING AND DIMENSIONS

### Notehead Width Calculation
From `style.h` line 130:
```c
#define HeadWidth(lnSp) (9*(lnSp)*4/32)
/* Expands to: HeadWidth = 1.125 * lnSpace */
```

**Units**: DDIST (staff interline space units)
**Formula**: Width = 9/8 of the staff interline space
**Example**: For a 32-point staff (lnSpace=32 DDIST), HeadWidth = 36 DDIST (2.25 points)

### Wide Notehead Adjustment
From `style.h` line 38:
```c
#define WIDENOTEHEAD_PCT(whCode, width) \
    (whCode==2? 160*(width)/100 : \
    (whCode==1? 135*(width)/100 : (width)))
```
- `whCode=0`: normal width
- `whCode=1`: whole notes (135% of normal)
- `whCode=2`: breves (160% of normal)

### Breve-Specific Y-Offset
From `defs.h` line 212:
```c
#define BREVEYOFFSET 2  /* Correction for Sonata error in breve origin (half-spaces) */
```
Applied in `DrawNRGR.cp` line 719:
```c
if (noteType==BREVE_L_DUR) breveFudgeHeadY = dhalfSp*BREVEYOFFSET;
else                       breveFudgeHeadY = (DDIST)0;
```

### Notehead X Offset (for font metrics)
From `DrawNRGR.cp` lines 767-774:
```c
yOffset = MusCharXOffset(doc->musFontInfoIndex, glyph, lnSpace);
if (yOffset) {
    xhead = pContext->paper.left + d2p(xd);
    xadjhead = pContext->paper.left + d2p(xd+yOffset);
} else {
    xhead = xadjhead = pContext->paper.left + d2p(xd);
}
```
The `MusCharXOffset()` function returns per-glyph X-positioning offsets from the music font.

### Notehead Y Offset (for font metrics)
From `DrawNRGR.cp` lines 774-784:
```c
yOffset = MusCharYOffset(doc->musFontInfoIndex, glyph, lnSpace);
if (yOffset) {
    yp = pContext->paper.top + d2p(yd);
    yhead = yp + fudgeHeadY + d2p(breveFudgeHeadY);
    yp = pContext->paper.top + d2p(yd+yOffset);
    yadjhead = yp + fudgeHeadY + d2p(breveFudgeHeadY);
} else {
    yp = pContext->paper.top + d2p(yd);
    yhead = yadjhead = yp + fudgeHeadY + d2p(breveFudgeHeadY);
}
```

---

## PART 3: STEM POSITIONING AND ATTACHMENT

### Stem Direction Determination
From `defs.h` line 413:
```c
#define DOWNSTEM(yd, staffHeight) ( ((yd) > (staffHeight)/2)? False : True )
```
Rule: **Stem goes DOWN if notehead Y-position > middle of staff**

In `DrawNRGR.cp` lines 237, 451, 828, 1053:
```c
stemDown = (NoteYSTEM(mainNoteL) > NoteYD(mainNoteL));
/* OR equivalently */
stemDown = (dStemLen > 0);
```
Direction is determined by sign of stem length: `dStemLen = aNote->ystem - aNote->yd`

### Stem Endpoint Calculation (CalcYStem)
From `Utilities/Utility.cp` lines 58-93:

**Function Signature**:
```c
DDIST CalcYStem(
    Document *doc,
    DDIST   yhead,           /* position of note head */
    short   nFlags,          /* number of flags or beams */
    Boolean stemDown,        /* True if stem is down, else stem is up */
    DDIST   staffHeight,     /* staff height */
    short   staffLines,      /* number of staff lines */
    short   qtrSp,           /* desired stem length in quarter-spaces */
    Boolean noExtend         /* True = don't extend stem to midline */
)
```

**Algorithm**:

1. **Adjust for flags** (lines 76-81):
```c
if (MusFontHas16thFlag(doc->musFontInfoIndex)) {
    if (nFlags>2) qtrSp += 4*(nFlags-2);  /* +1 space per extra flag after 1st 2 */
} else {
    if (nFlags>1) qtrSp += 4*(nFlags-1);  /* +1 space per flag after 1st */
}
```

2. **Convert quarter-spaces to DDIST** (line 82):
```c
dLen = qtrSp * staffHeight / (4 * (staffLines-1));
```

3. **Calculate stem endpoint** (line 83):
```c
ystem = (stemDown ? yhead+dLen : yhead-dLen);
```

4. **Extend to staff midline if needed** (lines 85-91):
```c
if (!noExtend) {
    midline = staffHeight / 2;
    if (ABS(yhead-midline) > dLen &&                    /* Would reach toward midline? */
        ABS(ystem-midline) < ABS(yhead-midline)) {      /* ...lengthening? */
        ystem = midline;                                 /* Yes, snap to midline */
    }
}
```

### Stem Length Configuration Constants
From `Initialize.cp` lines 919-929:

**Normal Stem Lengths** (quarter-spaces):
```c
#define STEMLEN_NORMAL_MIN 1
#define STEMLEN_NORMAL_DFLT 14      /* Single voice stem length */

#define STEMLEN_2V_MIN 1
#define STEMLEN_2V_DFLT 12          /* 2-voice (upper/lower) stem length */

#define STEMLEN_OUTSIDE_MIN 1
#define STEMLEN_OUTSIDE_DFLT 12     /* Outside staff stem length */

#define STEMLEN_GRACE_MIN 1
#define STEMLEN_GRACE_DFLT 10       /* Grace note stem length */
```

### Stem Length Selection (QSTEMLEN Macro)
From `defs.h` line 417:
```c
#define QSTEMLEN(singleV, shrt) \
    ((singleV)? config.stemLenNormal : \
     ((shrt)? config.stemLenOutside : config.stemLen2v))
```

**Selection Rules**:
- `singleV=True`: Use `config.stemLenNormal` (default 14)
- `singleV=False` AND `shrt=False`: Use `config.stemLen2v` (default 12)
- `singleV=False` AND `shrt=True`: Use `config.stemLenOutside` (default 12)

### Shortened Stem Detection
From `Utilities/Utility.cp` lines 135-150:

**Function**: `ShortenStem(LINK aNoteL, CONTEXT context, Boolean stemDown)`

**Rule**: Stem is shortened if:
1. Note is ABOVE staff and stem is UP:
   ```c
   if (halfLn < (0+STRICT_SHORTSTEM) && !stemDown) return True;
   ```
2. Note is BELOW staff and stem is DOWN:
   ```c
   if (halfLn > (2*(staffLines-1)-STRICT_SHORTSTEM) && stemDown) return True;
   ```

Where `halfLn` = half-line position from staff top, and `STRICT_SHORTSTEM=0` from `style.h`.

### Stem Width (Line Width)
From `Initialize.cp` line 952:
```c
#define STEMLW_DFLT 8    /* Default stem line width (% of line space) */
```

Applied in `PS_Stdio.cp` line 2044:
```c
stemLW = (config.stemLW * lineSpLong) / 100L;
```

### Upstem Spacing (Stem-Head Separation for Upstemmed Notes)
From `DrawNRGR.cp` lines 843-848:
```c
if (stemDown)
    stemSpace = 0;
else {
    stemSpace = MusFontStemSpaceWidthPixels(doc, doc->musFontInfoIndex, lnSpace);
    Move(stemSpace, 0);  /* Shift stem right for upstem notes */
}
```
The `MusFontStemSpaceWidthPixels()` function returns the music font's recommended space between notehead and upstem.

---

## PART 4: STEM SHORTENING FOR SPECIAL NOTEHEADS

From `style.h` lines 33-34:
```c
#define STEMSHORTEN_NOHEAD 6        /* Shorten by 6 eighth-spaces if no head */
#define STEMSHORTEN_XSHAPEHEAD 3    /* Shorten by 3 eighth-spaces if "X" head */
```

Applied in `DrawNRGR.cp` line 727:
```c
dStemShorten = (dhalfSp*stemShorten)/4;
```

**Units**: 8th-spaces (quarter-spaces / 2)
**Conversion**: `stemShorten` (in 8th-spaces) → DDIST via `(dhalfSp * stemShorten) / 4`

---

## PART 5: FLAG POSITIONING

### Flag Attachment Point
From `DrawNRGR.cp` lines 854-865 (Sonata font):
```c
ypStem = yhead+d2p(dStemLen);
if (doc->musicFontNum==sonataFontNum) {
    MoveTo(xhead, ypStem);  /* Position x at head, y at stem end */
    octaveLength = d2p(7*SizePercentSCALE(dhalfSp));
    Move(0, (stemDown? -octaveLength : octaveLength));  /* Adjust for flag origin */
```

**Sonata-specific rule**: Sonata flag glyphs assume a 7-space stem, so adjust by ±7 half-spaces.

### Flag Vertical Spacing (Leading)
From `style.h` line 68:
```c
#define FlagLeading(lnSp) (3*(lnSp)*4/16)
/* = (12 * lnSpace) / 16 = 0.75 * lnSpace */
```

**Units**: DDIST
**Meaning**: Vertical distance between consecutive flags

### Extension Flag Leading (Font-Specific)
From `DrawNRGR.cp` lines 927-932:
```c
if (stemDown) {
    flagGlyph = MapMusChar(doc->musFontInfoIndex, MCH_extendFlagDown);
    flagLeading = -d2p(DownstemExtFlagLeading(doc->musFontInfoIndex, lnSpace));
} else {
    flagGlyph = MapMusChar(doc->musFontInfoIndex, MCH_extendFlagUp);
    flagLeading = d2p(UpstemExtFlagLeading(doc->musFontInfoIndex, lnSpace));
}
```

Extension flag leading is retrieved from the music font, typically stored as % of line space.

### 8th Flag Leading (Font-Specific)
From `DrawNRGR.cp` lines 946-953:
```c
if (stemDown) {
    flagGlyph = MapMusChar(doc->musFontInfoIndex, MCH_eighthFlagDown);
    flagLeading = -d2p(Downstem8thFlagLeading(doc->musFontInfoIndex, lnSpace));
} else {
    flagGlyph = MapMusChar(doc->musFontInfoIndex, MCH_eighthFlagUp);
    flagLeading = d2p(Upstem8thFlagLeading(doc->musFontInfoIndex, lnSpace));
}
```

### Flag Glyph Offsets
From `DrawNRGR.cp` lines 905-908:
```c
xoff = MusCharXOffset(doc->musFontInfoIndex, flagGlyph, lnSpace);
yoff = SizePercentSCALE(MusCharYOffset(doc->musFontInfoIndex, flagGlyph, lnSpace));
if (xoff || yoff)
    Move(d2p(xoff), d2p(yoff));
```

Flag glyphs may have per-font X and Y offsets from their origin.

### Flag Count Determination
From `defs.h` line 409:
```c
#define NFLAGS(l_dur) ( ((l_dur)>QTR_L_DUR)? (l_dur)-QTR_L_DUR : 0 )
```

**Rule**:
- Quarter note or longer: 0 flags
- Eighth note: 1 flag (flagCount=1)
- 16th note: 2 flags
- 32nd note: 3 flags
- etc.

---

## PART 6: AUGMENTATION DOT POSITIONING

### Default Dot Offsets
From `Utilities/Utility.cp` lines 262-269:

**Function**: `GetLineAugDotPos(short voiceRole, Boolean stemDown)`

```c
short GetLineAugDotPos(short voiceRole, Boolean stemDown) {
    if (voiceRole==VCROLE_SINGLE) return 1;
    else                          return (stemDown? 3 : 1);
}
```

**Return values** (half-space Y-offsets):
- Single voice (middle of staff): 1 (dot offset)
- Upper voice: 1 (above note)
- Lower voice + stem down: 3 (below note for alignment)
- Lower voice + stem up: 1

### Dot Position in Note Structure
From `NObjTypes.h` lines 93-94:
```c
Byte xMoveDots;  /* X-offset on aug. dot position (quarter-spaces) */
Byte yMoveDots;  /* Y-offset on aug. dot pos. (half-spaces, 2=same as note, except 0=invisible) */
```

**Encoding**:
- `yMoveDots=0`: dots invisible
- `yMoveDots=1`: dot is 1 half-space above note
- `yMoveDots=2`: dot is at note position (default)
- `yMoveDots=3`: dot is 1 half-space below note

### Default Dot Position Initialization
From `Objects.cp` lines 857-859:
```c
aNote->xMoveDots = 3+WIDEHEAD(aNote->subType);
/* = 3 for normal heads, 4 for whole notes, 5 for breves */

if (MainNote(aNoteL))
    aNote->yMoveDots = GetLineAugDotPos(voiceRole, makeLower);
else
    aNote->yMoveDots = 2;  /* Non-main notes use note position */
```

### Dot Drawing Position Calculation
From `DrawNRGR.cp` lines 268-281:
```c
dhalfSp = LNSPACE(pContext)/2;  /* Half-space in DDIST */

ydDots = yd + (theNote->yMoveDots-2)*dhalfSp;  /* Apply Y offset */

xdDots = xdNorm + 2*dhalfSp;  /* Default: 2 spaces right of notehead */
xdDots += MusCharXOffset(doc->musFontInfoIndex, glyph, dhalfSp*2);
ydDots += MusCharYOffset(doc->musFontInfoIndex, glyph, dhalfSp*2);

/* Adjust for user-specified position */
if (theNote->xMoveDots != 3) {
    xdDots += (dhalfSp*(theNote->xMoveDots-3))/2;  /* Quarter-space units */
}
```

### Default X-Position for Dots
From `defs.h` line 294:
```c
#define DFLT_XMOVEACC 5  /* Default note <xMoveAcc> */
```

Augmentation dots start at code 3 (default) and can be offset in quarter-space increments.

### Width for Dot Spacing
From `SpaceTime.cp` line 292:
```c
nsWidth = (STD_LINEHT*2)+2;  /* For 1st dot default position */
nsWidth += (STD_LINEHT*(aNote->xMoveDots-3))/4;  /* Adjust for desired position */
```

---

## PART 7: ACCIDENTAL POSITIONING

### Accidental Width
From `style.h` line 23:
```c
#define STD_ACCWIDTH (9*STD_LINEHT/8)
/* = 1.125 * line height in staff units */
```

Converted to DDIST in `DrawNRGR.cp` line 368:
```c
dAccWidth = std2d(STD_ACCWIDTH, pContext->staffHeight, pContext->staffLines);
```

### Default Accidental Position
From `Objects.cp` line 877:
```c
aNote->xmoveAcc = DFLT_XMOVEACC;  /* = 5 quarter-space units left */
```

### Drawing Position
From `DrawNRGR.cp` lines 387-388:
```c
xdAcc += SizePercentSCALE(MusCharXOffset(doc->musFontInfoIndex, accGlyph, lnSpace));
yd += SizePercentSCALE(MusCharYOffset(doc->musFontInfoIndex, accGlyph, lnSpace));
```

---

## PART 8: CHORD STEM ATTACHMENT (Notehead-to-Stem Connection)

### Stem Position Relative to Notehead
From `style.h` line 44-45:
```c
#define NHEAD_XD_GLYPH_ADJ(stemDn, headRSize) \
    (!stemDn? ((headRSize-100L)*HeadWidth(lnSpace))/100L : 0L)
```

**Rule**: For **upstemmed notes with non-normal noteheads**:
- Adjustment = `(headRelSize - 100) * HeadWidth(lnSpace) / 100`
- Move stem to right edge of notehead for wider heads (breves, whole notes)
- No adjustment for downstemmed notes (stem already left)

### Wide Head Stem Attachment
Applied in `DrawNRGR.cp` line 1054:
```c
xdAdj = (headRelSize==100? 0 : NHEAD_XD_GLYPH_ADJ(stemDown, headRelSize));
PS_NoteStem(doc, (xd+xoff)-xdAdj, yd+yoff, xd+xoff, glyph, dStemLen, ...);
```

### Multi-Note Chord Positioning
From `DrawNRGR.cp` lines 742-743:
```c
chordNoteToR = (!aNote->rest && ChordNoteToRight(pL, aNote->voice));
chordNoteToL = (!aNote->rest && ChordNoteToLeft(pL, aNote->voice));
```

This determines if notes are to the right/left of the main note in a chord.

---

## PART 9: CHORD SLASHES

### Slash Thickness
From `style.h` line 75:
```c
#define SLASH_THICK 4    /* PostScript chord slash thickness (eighth-spaces) */
```

### Slash Position (PostScript)
From `PS_Stdio.cp` lines 1678-1683:
```c
if (!sym) {  /* Chord slash */
    yoff = y+2*dhalfSp;
    thick = SLASH_THICK*dhalfSp/4;
    if (stemDown)     xoff = x-SLASH_XTWEAK;
    else              xoff = x-(3*thick)/4+SLASH_XTWEAK;
    if (headVisible) PS_LineHT(xoff, yoff, xoff+2*dhalfSp, yoff-4*dhalfSp, thick);
}
```

### Slash Tweak Constant
From `PS_Stdio.cp` line 1655:
```c
#define SLASH_XTWEAK (DDIST)2    /* Experimental constant */
```

**Slash Rules**:
- Positioned 2 half-spaces above notehead Y-center
- Stem down: slash shifted left by `SLASH_XTWEAK`
- Stem up: slash shifted right by `(3*thick)/4 - SLASH_XTWEAK`
- Width: 2 half-spaces, Height: 4 half-spaces, slope 45 degrees

---

## PART 10: REST POSITIONING AND STEMLETS

### Rest Stemlet Length
From `style.h` line 63:
```c
#define REST_STEMLET_LEN 4    /* For "stemlets" on beamed rests (quarter-spaces) */
```

Applied in `DrawNRGR.cp` lines 1392-1396:
```c
if (aRest->beamed && REST_STEMLET_LEN>=0 && config.drawStemlets) {
    stemDown = (config.stemLenNormal>0);
    qStemLen = 2*NFLAGS(aRest->subType)-1+REST_STEMLET_LEN;
    dStemLen = lnSpace*qStemLen/4;
}
```

---

## PART 11: GRACE NOTE SIZING

### Grace Note Scale Factor
From `style.h` line 15:
```c
#define GRACESIZE(size) (7*(size)/10)    /* 70% of normal size */
```

### Grace Note Stem Length
From `Initialize.cp` line 748:
```c
config.stemLenGrace  /* Default: 10 quarter-spaces */
```

Applied in `Utility.cp` lines 121-128:
```c
stemLen = qd2d(config.stemLenGrace, context.staffHeight, context.staffLines);
if (GRMainNote(aGRNoteL))
    return GRNoteYD(aGRNoteL)-stemLen;
else
    return GRNoteYD(aGRNoteL);
```

---

## PART 12: TREMOLO SLASHES (Tremolo Marks)

### Slash Parameters
From `DrawNRGR.cp` lines 41-60:
```c
lnSpace = LNSPACE(pContext);
dEighthLn = LNSPACE(pContext)/8;

slashLeading = (stemUp? 6*dEighthLn : -6*dEighthLn);  /* Spacing between slashes */
slashWidth = HeadWidth(lnSpace);                      /* ~1.125 line spaces */
slashHeight = lnSpace/2;                              /* Half a line space */
slashThick = (config.tremSlashLW*lnSpace)/100L;       /* % of line space */

dxpos = (stemUp? 4*dEighthLn : -5*dEighthLn);         /* Horizontal offset */
dypos = (stemUp? 8*dEighthLn : -8*dEighthLn);         /* Vertical offset */
```

---

## PART 13: SIZE SCALING (Small Notes, Grace Notes, etc.)

### Size Percent Application
From `DrawNRGR.cp` lines 732-732:
```c
sizePercent = (aNote->small? SMALLSIZE(100) : 100);
```

From `style.h` line 16:
```c
#define SMALLSIZE(size) (3*(size)/4)    /* 75% of normal */
```

### Font-Relative Offset Scaling
From `DrawNRGR.cp` line 906, 915, 955:
```c
yoff = SizePercentSCALE(MusCharYOffset(...));
```

From `defs.h` line 420:
```c
#define SizePercentSCALE(value) ((int)(((long)sizePercent*(value))/100L))
```

---

## PART 14: MULTI-VOICE NOTE PLACEMENT

### Voice Role Constants
From `Multivoice.cp` line 27:
```c
#define SINGLE_DI 6    /* Radio button code for single voice */
```

### Voice Role Selection Rules
From `Utilities/Utility.cp` lines 168-190:

**VCROLE_SINGLE**: 
```c
stemDown = (halfLn<=context.staffLines-1);  /* Bottom half = down */
*qStemLen = QSTEMLEN(True, ShortenStem(...));
```

**VCROLE_UPPER**:
```c
stemDown = False;  /* Always up */
*qStemLen = QSTEMLEN(False, ShortenStem(...));
```

**VCROLE_LOWER**:
```c
stemDown = True;   /* Always down */
*qStemLen = QSTEMLEN(False, ShortenStem(...));
```

**VCROLE_CROSS**:
```c
stemDown = (NoteSTAFF(aNoteL)==pPart->firstStaff);  /* Top staff = down */
*qStemLen = QSTEMLEN(False, ShortenStem(...));
```

---

## PART 15: COMPLETE DRAWING SEQUENCE

### DrawNote (Main Rendering Loop)
From `DrawNRGR.cp` lines 680-1005:

1. **Get staff metrics** (line 709):
   ```c
   lnSpace = LNSPACE(pContext);
   dhalfSp = lnSpace/2;
   ```

2. **Get notehead position** (line 712):
   ```c
   xd = NoteXLoc(pL, aNoteL, pContext->measureLeft, HeadWidth(lnSpace), &xdNorm);
   ```

3. **Get note Y position** (line 715):
   ```c
   yd = dTop + aNote->yd;
   ```

4. **Get glyph and stem info** (lines 725-727):
   ```c
   glyph = GetNoteheadInfo(appearance, noteType, &headRelSize, &stemShorten);
   dStemShorten = (dhalfSp*stemShorten)/4;
   ```

5. **Draw accidental** (line 798):
   ```c
   DrawAcc(doc, pContext, aNoteL, xdNorm, yd, dim, sizePercent, chordNoteToL);
   ```

6. **Draw ledger lines** (line 805):
   ```c
   DrawNCLedgers(pL, pContext, aNoteL, xd, dTop, ledgerSizePct);
   ```

7. **Draw stem and flags** (lines 825-961):
   ```c
   if (MainNote(aNoteL)) {
       stemDown = (dStemLen>0);
       /* Draw stem: Line(0, d2p(dStemLen)) */
       /* Draw flags at ypStem = yhead+d2p(dStemLen) */
   }
   ```

8. **Draw notehead** (lines 964-970):
   ```c
   MoveTo(xadjhead, yadjhead);
   DrawNotehead(doc, glyph, appearance, dim, dhalfSp);
   ```

9. **Draw modifiers and dots** (lines 975-976):
   ```c
   DrawModNR(doc, aNoteL, xd, pContext);
   DrawAugDots(doc, aNoteL, xdNorm, yd, pContext, chordNoteToR);
   ```

---

## PART 16: POSTSCRIPT RENDERING (PS_NoteStem)

From `PS_Stdio.cp` lines 1657-1735:

### Function Parameters
```c
OSErr PS_NoteStem(
    Document *doc,
    DDIST x, DDIST y,        /* Notehead origin (x=left edge) */
    DDIST xNorm,             /* If stem up, draw stem as if left edge were here */
    char sym,                /* Notehead glyph, or null = chord slash */
    DDIST stemLen,           /* Length (>0 = down, <0 = up) */
    DDIST stemShorten,       /* Shorten stem from notehead by this */
    Boolean beamed,
    Boolean headVisible,
    short sizePercent)
```

### Stem-Down Rendering
```c
if (stemDown) {
    if (sym) PS_Print("(%P)",str);           /* Draw notehead */
    
    if (drawStem) {
        yShorten = y+stemShorten;
        stemLen -= stemShorten;
        PS_Print("%ld %ld %ld %ld %ld %ld SD\r",
            (long)x, (long)y,
            (long)x, (long)yShorten+stemFudge,
            (long)x, (long)(yShorten+stemLen));
    }
}
```

**PostScript "SD" operator**: `x1 y1 x2 y2 x3 y3 SD`
- (x1,y1) = notehead position
- (x2,y2) = stem top (adjusted for shortening)
- (x3,y3) = stem bottom

### Stem-Up Rendering
```c
else {  /* Stem up */
    PS_MusChar(doc, x, y, sym, headVisible, sizePercent);  /* Notehead */
    
    if (drawStem) {
        yShorten = y-stemShorten;
        stemLen += stemShorten;  /* stemLen is negative, so += moves down */
        PS_Print("%ld %ld %ld SU\r",
            (long)xNorm, (long)yShorten+stemLen+(DDIST)(beamed ? 8 : 0),
            (long)(-stemShorten));
    }
}
```

**PostScript "SU" operator**: `xStemEnd yStemEnd yShorten SU`
- Draws upward stem from notehead

---

## SUMMARY TABLE: Key Constants for Rendering

| Constant | Value | Units | Meaning |
|----------|-------|-------|---------|
| HeadWidth | 9*lnSp/8 | DDIST | Standard notehead width |
| FlagLeading | 3*lnSp/4 | DDIST | Space between flags |
| STD_ACCWIDTH | 9*STD_LINEHT/8 | STDIST | Accidental width |
| STEMLEN_NORMAL_DFLT | 14 | 1/4 space | Single-voice stem |
| STEMLEN_2V_DFLT | 12 | 1/4 space | 2-voice stem |
| STEMLEN_GRACE_DFLT | 10 | 1/4 space | Grace note stem |
| BREVEYOFFSET | 2 | 1/2 space | Breve Y correction |
| STEMSHORTEN_NOHEAD | 6 | 1/8 space | No-head shortening |
| STEMSHORTEN_XSHAPEHEAD | 3 | 1/8 space | X-head shortening |
| REST_STEMLET_LEN | 4 | 1/4 space | Beamed rest stemlet |
| SLASH_THICK | 4 | 1/8 space | Chord slash thickness |
| STEMLW_DFLT | 8 | % of lnSp | Default stem width |
| DFLT_XMOVEACC | 5 | 1/4 space | Default accidental offset |
| GRACESIZE | 70% | percent | Grace note scaling |
| SMALLSIZE | 75% | percent | Small note scaling |
| STD_LINEHT | 8 | STDIST | Staff interline in STDIST units |

---

## CRITICAL FORMULAS

### Stem Calculation
```
stemLen_quarters = config.stemLen[role]
if (nFlags>1): stemLen_quarters += 4*(nFlags-1)  [or +4*(nFlags-2) with 16th flag]

stemLen_DDIST = stemLen_quarters * staffHeight / (4 * (staffLines-1))

stemEnd_Y = (stemDown ? noteY + stemLen_DDIST : noteY - stemLen_DDIST)

if (!noExtend && distance_to_midline < stemLen_DDIST:
    stemEnd_Y = midline
```

### Dot Positioning
```
yDot = noteY + (yMoveDots - 2) * half_line_space_DDIST
xDot = noteX + 2 * half_line_space_DDIST + fontOffset
```

### Accidental Positioning
```
xAcc = noteX - (xMoveAcc - 5) * quarter_line_space_DDIST
xAcc += fontXOffset
```

### Wide Notehead Adjustment (Upstem Only)
```
stemX_adjust = (headRelSize - 100) * HeadWidth(lnSpace) / 100  [if upstem]
```

