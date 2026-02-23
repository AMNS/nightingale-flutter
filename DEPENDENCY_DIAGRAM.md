# Nightingale Dependency Chain (Mermaid)

```mermaid
graph TD
    NBasicTypes["NBasicTypes.h<br/>(206 lines)<br/>DDIST, LINK, etc."]
    NLimits["NLimits.h<br/>(77 lines)<br/>MAX_*, limits"]
    Defs["Defs.h<br/>(601 lines)<br/>Enums, macros"]
    Style["Style.h<br/>(137 lines)<br/>Notation params"]
    NObjTypes["NObjTypes.h<br/>(994 lines)<br/>All object structs"]
    NObjTypesN105["NObjTypesN105.h<br/>(642 lines)<br/>N105 format variants"]
    NDocCnfg["NDocAndCnfgTypes.h<br/>(785 lines)<br/>Document, Config"]
    
    StringPool["StringPool.cp<br/>(852 lines)<br/>String mgmt"]
    Objects["Objects.cp<br/>(2103 lines)<br/>HeapAlloc, Init*"]
    HeapFileIO["HeapFileIO.cp<br/>(1346 lines)<br/>Read/Write .ngl"]
    
    Context["Context.cp<br/>(1200 lines)<br/>GetContext, Fix*"]
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
    
    style NBasicTypes fill:#e1f5ff
    style NLimits fill:#e1f5ff
    style Defs fill:#e1f5ff
    style Style fill:#f3e5f5
    style NObjTypes fill:#f3e5f5
    style NObjTypesN105 fill:#f3e5f5
    style NDocCnfg fill:#f3e5f5
    
    style StringPool fill:#e8f5e9
    style Objects fill:#e8f5e9
    style HeapFileIO fill:#fff3e0
    
    style Context fill:#fce4ec
    style SpaceTime fill:#f1f8e9
    style SpaceHigh fill:#f1f8e9
    
    style Beam fill:#e0f2f1
    style GRBeam fill:#e0f2f1
    style Slurs fill:#e0f2f1
    style Tuplet fill:#e0f2f1
    style Utility fill:#e0f2f1
    
    style DrawHigh fill:#ede7f6
    style DrawObj fill:#ede7f6
    style DrawNRGR fill:#ede7f6
    style PSStdio fill:#ede7f6
```

## Legend
- Blue: Base types (no dependencies)
- Purple: Object type definitions
- Green: Infrastructure (alloc, strings)
- Orange: File I/O
- Pink: Context propagation
- Light green: Spacing/layout
- Cyan: Engraving algorithms
- Gray-purple: Drawing/rendering
