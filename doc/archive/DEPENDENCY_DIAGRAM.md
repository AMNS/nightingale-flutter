# Nightingale Dependency Chain (Mermaid)

```mermaid
graph TD
    NBasicTypes["NBasicTypes.h<br/>(206 lines)<br/>DDIST, LINK, etc.<br/><b>PORTED</b>"]
    NLimits["NLimits.h<br/>(77 lines)<br/>MAX_*, limits<br/><b>PORTED</b>"]
    Defs["Defs.h<br/>(601 lines)<br/>Enums, macros<br/><b>PORTED</b>"]
    Style["Style.h<br/>(137 lines)<br/>Notation params"]
    NObjTypes["NObjTypes.h<br/>(994 lines)<br/>All object structs<br/><b>PORTED</b>"]
    NObjTypesN105["NObjTypesN105.h<br/>(642 lines)<br/>N105 format variants<br/><b>PORTED</b>"]
    NDocCnfg["NDocAndCnfgTypes.h<br/>(785 lines)<br/>Document, Config<br/><b>PORTED</b>"]

    StringPool["StringPool.cp<br/>(852 lines)<br/>String mgmt<br/><i>partial</i>"]
    Objects["Objects.cp<br/>(2103 lines)<br/>HeapAlloc, Init*"]
    HeapFileIO["HeapFileIO.cp<br/>(1346 lines)<br/>Read/Write .ngl<br/><b>PORTED (read)</b>"]

    Context["Context.cp<br/>(1200 lines)<br/>GetContext, Fix*<br/><b>PORTED</b>"]
    SpaceTime["SpaceTime.cp<br/>(1981 lines)<br/>Spacing calcs"]
    SpaceHigh["SpaceHighLevel.cp<br/>(1781 lines)<br/>Layout justification"]

    Beam["Beam.cp<br/>(2403 lines)<br/>Beam calculation"]
    GRBeam["GRBeam.cp<br/>(1283 lines)<br/>Grace beams"]
    Slurs["Slurs.cp<br/>(1504 lines)<br/>Bezier curves"]
    Tuplet["Tuplet.cp<br/>(1866 lines)<br/>Tuplet logic"]
    Utility["Utility.cp<br/>(1695 lines)<br/>CalcYStem, etc."]

    DrawHigh["DrawHighLevel.cp<br/>(735 lines)<br/>Draw orchestration"]
    DrawObj["DrawObject.cp<br/>(3160 lines)<br/>Draw clef, measure, etc."]
    DrawNRGR["DrawNRGR.cp<br/>(2252 lines)<br/>Draw notes, rests"]
    PSStdio["PS_Stdio.cp<br/>(2388 lines)<br/>PostScript primitives"]

    %% TIER 1: Base types (no deps within set)
    NBasicTypes -.->|"used by"| Defs
    NBasicTypes -.->|"used by"| Style
    NBasicTypes -.->|"used by"| NObjTypes
    NBasicTypes -.->|"used by"| NObjTypesN105
    NBasicTypes -.->|"used by"| NDocCnfg

    %% TIER 2: More base types
    Style -->|depends| Defs
    NObjTypes -->|depends| NBasicTypes
    NObjTypesN105 -->|depends| NBasicTypes
    NDocCnfg -->|depends| NObjTypes

    %% TIER 3: Infrastructure
    StringPool -->|depends| NBasicTypes
    Objects -->|depends| NObjTypes
    Objects -->|depends| NDocCnfg
    Objects -->|depends| Defs

    %% TIER 4: File I/O
    HeapFileIO -->|depends| NObjTypes
    HeapFileIO -->|depends| NObjTypesN105
    HeapFileIO -->|depends| Objects
    HeapFileIO -->|depends| StringPool

    %% TIER 5: Context
    Context -->|depends| NObjTypes
    Context -->|depends| Defs
    Context -->|depends| Utility

    %% TIER 6: Spacing
    SpaceTime -->|depends| NObjTypes
    SpaceTime -->|depends| Context
    SpaceTime -->|depends| Utility
    SpaceTime -->|depends| Defs

    %% TIER 7: High-level layout
    SpaceHigh -->|depends| SpaceTime
    SpaceHigh -->|depends| Context
    SpaceHigh -->|depends| Utility

    %% TIER 8: Engraving
    Beam -->|depends| Context
    Beam -->|depends| SpaceTime
    Beam -->|depends| Utility

    GRBeam -->|depends| Beam
    GRBeam -->|depends| Context
    GRBeam -->|depends| Utility

    Slurs -->|depends| NObjTypes
    Slurs -->|depends| Context
    Slurs -->|depends| Utility

    Tuplet -->|depends| SpaceTime
    Tuplet -->|depends| Objects
    Tuplet -->|depends| Context
    Tuplet -->|depends| Utility

    Utility -->|depends| NObjTypes
    Utility -->|depends| Defs
    Utility -->|depends| Style

    %% TIER 9: Drawing
    DrawHigh -->|depends| Objects
    DrawHigh -->|depends| Context
    DrawHigh -->|depends| Utility

    DrawObj -->|depends| Context
    DrawObj -->|depends| Beam
    DrawObj -->|depends| GRBeam
    DrawObj -->|depends| Slurs
    DrawObj -->|depends| Tuplet
    DrawObj -->|depends| Utility

    DrawNRGR -->|depends| Context
    DrawNRGR -->|depends| DrawObj
    DrawNRGR -->|depends| Style
    DrawNRGR -->|depends| Utility

    PSStdio -->|depends| Defs
    PSStdio -->|depends| Utility

    style NBasicTypes fill:#1565c0,color:#ffffff,stroke:#0d47a1
    style NLimits fill:#1565c0,color:#ffffff,stroke:#0d47a1
    style Defs fill:#1565c0,color:#ffffff,stroke:#0d47a1
    style Style fill:#6a1b9a,color:#ffffff,stroke:#4a148c
    style NObjTypes fill:#6a1b9a,color:#ffffff,stroke:#4a148c
    style NObjTypesN105 fill:#6a1b9a,color:#ffffff,stroke:#4a148c
    style NDocCnfg fill:#6a1b9a,color:#ffffff,stroke:#4a148c

    style StringPool fill:#2e7d32,color:#ffffff,stroke:#1b5e20
    style Objects fill:#2e7d32,color:#ffffff,stroke:#1b5e20
    style HeapFileIO fill:#e65100,color:#ffffff,stroke:#bf360c

    style Context fill:#ad1457,color:#ffffff,stroke:#880e4f
    style SpaceTime fill:#558b2f,color:#ffffff,stroke:#33691e
    style SpaceHigh fill:#558b2f,color:#ffffff,stroke:#33691e

    style Beam fill:#00695c,color:#ffffff,stroke:#004d40
    style GRBeam fill:#00695c,color:#ffffff,stroke:#004d40
    style Slurs fill:#00695c,color:#ffffff,stroke:#004d40
    style Tuplet fill:#00695c,color:#ffffff,stroke:#004d40
    style Utility fill:#00695c,color:#ffffff,stroke:#004d40

    style DrawHigh fill:#4527a0,color:#ffffff,stroke:#311b92
    style DrawObj fill:#4527a0,color:#ffffff,stroke:#311b92
    style DrawNRGR fill:#4527a0,color:#ffffff,stroke:#311b92
    style PSStdio fill:#4527a0,color:#ffffff,stroke:#311b92
```

## Legend
| Color | Category | Status |
|-------|----------|--------|
| Blue | Base types (no dependencies) | **PORTED** |
| Purple | Object type definitions | **PORTED** |
| Green | Infrastructure (alloc, strings) | Partial (read-side done) |
| Orange | File I/O | **PORTED** (read only) |
| Pink | Context propagation | **PORTED** |
| Olive | Spacing/layout | Not started |
| Teal | Engraving algorithms | Not started |
| Deep purple | Drawing/rendering | Not started |

## Rust Module Mapping
| C++ Source | Rust Module | Status |
|------------|-------------|--------|
| NBasicTypes.h | `basic_types.rs` | Done |
| NLimits.h | `limits.rs` | Done |
| defs.h | `defs.rs` | Done |
| NObjTypes.h + N105 | `obj_types.rs` | Done |
| NDocAndCnfgTypes.h | `doc_types.rs` | Done |
| HeapFileIO.cp | `ngl/reader.rs` + `ngl/interpret.rs` | Done (read) |
| Context.cp | `context.rs` | Done |
| StringPool.cp | `ngl/reader.rs` (decode only) | Partial |
| — (new) | `notelist/parser.rs` | Done |
| style.h | — | Not started |
| Objects.cp | — | Not started |
| SpaceTime.cp | — | Not started |
| SpaceHighLevel.cp | — | Not started |
| Utility.cp | — | Not started |
| Beam.cp / GRBeam.cp | — | Not started |
| Slurs.cp | — | Not started |
| Tuplet.cp | — | Not started |
| Draw*.cp | — | Not started |
