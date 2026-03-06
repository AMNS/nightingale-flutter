# MusicXML Import/Export Research

*Generated 2026-03-05 from analysis of MusicXML spec, crates, and existing icebox code*

## MusicXML Specification

- **Current version**: MusicXML 4.0 (W3C Community Group Standard)
- **Repo**: https://github.com/w3c/musicxml
- **Spec**: https://w3c.github.io/musicxml/
- **Format**: XML-based, with both partwise and timewise representations
- **Compression**: `.mxl` files are ZIP archives containing `.musicxml` + `META-INF/container.xml`

## Recommended Rust Crate: `musicxml` v1.1.2

- **crates.io**: https://crates.io/crates/musicxml
- **Approach**: Typed Rust DOM — full MusicXML 4.0 schema as Rust structs
- **Parsing**: Built on `roxmltree` (read-only XML parser)
- **Serialization**: Built on `quick-xml` for writing
- **API**: `musicxml::read_score_partwise(xml_string)` → typed DOM
- **Pros**:
  - Native Rust, no FFI
  - Full MusicXML 4.0 coverage
  - Typed — catches structural errors at compile time
  - Both read and write support
- **Cons**:
  - Large API surface (mirrors full MusicXML complexity)
  - v1.1.2 may have edge cases with some files

## Existing Icebox Code

The project has ~90% complete MusicXML **export** code in the icebox (from earlier
Python-based NGL→MusicXML converter work). Key details:

- Uses `quick-xml 0.37` for XML generation
- Handles: notes, rests, chords, beams, ties, slurs, key signatures, time signatures,
  clefs, dynamics, barlines, directions
- Missing: grace notes, tuplets, ottava lines, lyrics, chord symbols
- Architecture: Walks the Nightingale score data model, emits MusicXML elements

## Architecture Recommendation

### Import (MusicXML → Nightingale Score)

1. Use `musicxml` crate to parse MusicXML into typed DOM
2. Walk the DOM, converting to Nightingale's internal `Score` struct
3. Key mappings:
   - `<part>` → Staff/Part
   - `<measure>` → Measure objects
   - `<note>` → SYNC + ANOTE (handling chords via `<chord/>` tag)
   - `<beam>` → BeamSet objects
   - `<slur>` → Slur objects
   - `<direction>` → Dynamic, Tempo, GRAPHIC objects
   - `<attributes>` → Clef, KeySig, TimeSig objects

### Export (Nightingale Score → MusicXML)

1. Walk Nightingale score object list
2. Group objects by measure (between barlines)
3. Emit MusicXML using `quick-xml` or the `musicxml` crate's write API
4. Handle voice assignment (Nightingale voices → MusicXML `<voice>`)
5. Emit `<backup>` elements for multi-voice measures

### Estimated Effort

| Component | Days | Notes |
|-----------|------|-------|
| Import: basic (notes, rests, measures) | 3-4 | Core mapping |
| Import: beams, ties, slurs | 2-3 | Relationship objects |
| Import: key/time/clef | 1-2 | Attribute changes |
| Import: dynamics, text | 2-3 | Direction elements |
| Import: grace notes, tuplets | 2-3 | Special note types |
| Export: basic | 2-3 | Reverse of import |
| Export: relationships | 2-3 | Beams, ties, slurs |
| Testing & edge cases | 3-4 | Real-world files |
| **Total** | **15-20** | |

### Priority

**Import > Export**. Import enables interop with Dorico, MuseScore, Sibelius, Finale.
Export is less urgent since we already have PDF output for the "share a score" use case.

## Test Data Sources

- **MuseScore test corpus**: Hundreds of MusicXML files in MuseScore's test suite
- **Lilypond regression tests**: MusicXML import tests
- **MakeMusic samples**: Official MusicXML example files
- **seiso.com converters**: nl2xml (https://www.seiso.com/nl2xml/) and xml2nl (https://www.seiso.com/xml2nl/) for Notelist ↔ MusicXML conversion

## Dependencies to Add

```toml
[dependencies]
musicxml = "1.1.2"   # For import (typed DOM)
quick-xml = "0.37"   # For export (already in icebox code)
zip = "2.0"          # For .mxl compressed format support
```
