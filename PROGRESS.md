# Nightingale Modernization — Progress Tracker

## Current Phase: 2 (Drawing / Rendering Layer)

## Phase 0: Source Archaeology — COMPLETE
- [x] Classify core source files by role (DATA_MODEL / ENGRAVING / UI / PLATFORM)
- [x] Build dependency graph (DEPENDENCY_CHAIN.csv, DEPENDENCY_DIAGRAM.md)
- [x] Produce porting roadmap (superseded by this file)

## Phase 1: Rust Data Model — COMPLETE

### Foundation (commit 7da7522)
- [x] Rust workspace with cargo, pre-commit hooks (fmt + clippy + tests)
- [x] DDIST/STDIST/SHORTQD coordinate types (basic_types.rs)
- [x] Constants/limits (limits.rs), enums/macros (defs.rs)
- [x] 25 object/subobject struct definitions (obj_types.rs)
- [x] Document/score header types (doc_types.rs)
- [x] .ngl binary reader, N103 format (ngl/reader.rs)
- [x] Heap interpreter, variable-stride, all 25 types (ngl/interpret.rs)

### Infrastructure (commit cb8d791)
- [x] Variable-stride object heap decoding (critical bug fix)
- [x] Full interpret_heap() with subobject unpacking (AStaff, AMeasure, AClef, AKeySig, ATimeSig)
- [x] Notelist (.nl) parser, V1/V2, 13 record types (notelist/parser.rs)
- [x] Musical context system, forward-traversal propagation (context.rs)

### Accessors & Math (commit 81c75fe)
- [x] Cross-validation: NGL interpreter across all 16 fixtures
- [x] Score accessors: head(), tail(), num_staves(), score_list(), syncs(), measure_objects()
- [x] Document header parser (ngl/doc_header.rs)
- [x] Duration math: code_to_l_dur, measure_dur, beat_l_dur, etc. (duration.rs)

## Phase 2: Drawing / Rendering Layer — IN PROGRESS

### Rendering Architecture (commits f1ff974, 42aa184)
- [x] MusicRenderer trait: 32 methods mirroring PS_Stdio.cp's 27 primitives + state mgmt
- [x] RenderCommand enum: serializable commands for Flutter bridge and test recording
- [x] CommandRenderer: records commands for structural testing
- [x] PdfRenderer (pdf-writer): PS_Stdio.cp PostScript operators mapped to PDF content stream
- [x] Embedded Bravura SMuFL font rendering in PDF output
- [x] score_renderer.rs: staff lines, barlines, clefs, time sigs, noteheads, stems, accidentals, ledger lines, beams (flat), flags
- [x] to_score.rs: Notelist->InterpretedScore converter (measure spacing, chords, voice filtering, beam grouping)
- [x] Preamble layout from CreateSystem (Score.cp:1785-1814), Ross-convention spacing
- [x] Stem direction: NormalStemUpDown + CalcYStem for single notes and chords
- [x] Ledger lines from NoteLedgers (DrawUtils.cp)
- [x] Stem/beam X from OG HeadWidth (defs.h:355)
- [x] Invisible initial measure (no spurious barline before anacrusis)
- [x] 20 PDF primitive smoke tests + 2 HBD_33 pipeline tests + 4 punted roadmap tests

### Recently Completed
- [x] **Barline fix**: barlines at end of system only, not start of next (system-boundary xd fix)
- [x] **Continuation preamble**: narrower preamble for systems 2+ (no time sig space), clef_xd + 2.5*dLineSp
- [x] **Beam system-boundary break**: beams no longer span across system breaks
- [x] **Multi-system layout**: port of CreateSystem/NewSysNums — measures grouped into N systems (default 4/system), each with SYSTEM→STAFF→CONNECT→CLEF→[content], stacked vertically via inter_system spacing. Time sig only on system 1. Renderer required zero changes.
- [x] **Multiple voices per staff**: VoiceRole enum (Single/Upper/Lower), auto voice role detection, UPPER stems-up, LOWER stems-down, shorter 2v stems (stemLen2v=12), multi-voice rest offset
- [x] **Visual regression test framework**: insta snapshot-based, HBD_33 blessed snapshot with command counts, staff/barline/beam geometry, glyph distribution
- [x] **Beam slope**: port GetBeamEndYStems (Beam.cp:181) + FixSyncInBeamset (Beam.cp:272), 33% slope reduction
- [x] **Beam group stem unification**: port NormalStemUpDown (Objects.cp:1594) for beam groups — voice-role-aware
- [x] **Renderer stem direction fix**: beam renderer now uses per-note ystem vs yd (matching OG) instead of heuristic
- [x] **OG source line endings**: converted all 276 .cp/.h files to Unix LF — no more `tr` preprocessing

### Recently Completed (previous sessions)
- [x] **Key signatures**: DrawKEYSIG port — full position tables for all 7 clef types, preamble KEYSIG objects, SetupKeySig circle-of-fifths order, SMuFL sharp/flat glyphs, preamble width adjustment for accidental count.
- [x] **Tied notes**: visual ties between notes across beats/measures + cross-system partial ties
- [x] **Notelist stem_info parser fix**: accept all 6 flag characters (ties, slurs, tuplets were silently dropped)
- [x] **Comprehensive notelist test suite**: 6 tests × 20 .nl files (parse, convert, render, geometry, PDF, insta snapshots)
- [x] **Ddist overflow fix**: i16 arithmetic overflow in scores with many systems — widened to i32 in to_score.rs, context.rs, score_renderer.rs
- [x] **Tuplet rendering**: Port of DrawTUPLET/DrawPSTupletBracket (Tuplet.cp) — bracket with cutoff lines + gap for number, SMuFL timeSig digit numerals, stem_info 'T' flag → in_tuplet, ANoteTuple subobjects linking to syncs.
- [x] **Custom noteheads**: Port of NoteGlyph/GetNoteheadInfo (DrawUtils.cp) — X-shaped, harmonic, square, diamond, halfnote, slash notation via SMuFL alternate notehead glyphs.
- [x] **Extra blank page fix**: Fixed unconditional page push in PdfRenderer::finish() — now checks `in_page` flag.

### Recently Completed (this session)
- [x] **QA Compare button fix**: Replaced hardcoded `/Users/chirgwin/Nightingale-Phoenix/nightingale-modernize` path with dynamic project root discovery via `Directory.current.path` (dev mode) and `Platform.resolvedExecutable` fallback (release builds). Fixes regression after repo move to `/Users/chirgwin/nightingale-flutter/`. Commit 410eeb0.
- [x] **Ottava investigation**: Confirmed draw_ottava() implementation is complete (205 lines, src/draw/draw_object.rs:2558-2762). Handles all 6 octave types (8va/15ma/22ma alta/bassa), Sonata italic digit rendering, dashed brackets, vertical cutoffs. Wired into render loop dispatch. **No test fixtures contain OTTAVA objects** — tc_05.ngl has zero ottavas despite test name. Ottavas are NOT GRAPHIC objects (they're a dedicated object type with active behavior: adjusting note positions, MIDI playback transposition). Implementation ready, just untested due to lack of fixture coverage.

**Stashed work** (git stash -m 'WIP fixture and RPTEND changes'):
- Capital Regiment March added to OG_FIXTURES in src/comparison.rs (for visual diff testing)
- RPTEND documentation updates in src/draw/draw_object.rs (clarifying two repeat rendering paths)
- User decided to punt on both for now

### Previously Completed (previous session)
- [x] **NGL chord symbol normalization**: Port of ChordSym.cp ParseChordSym field structure. NGL chord symbols use 0x7F delimiters to separate 7 fields (root|qual|ext|extStk1|extStk2|extStk3|bass) and Sonata font accidental codes (0xBA=dbl-flat, 0xDC=dbl-sharp). `normalize_chord_symbol()` splits on 0x7F, replaces Sonata accidentals with text equivalents, joins fields with proper separators. 11 unit tests.
- [x] **Flutter bridge SetPageSize fix**: `render_to_dtos()` was missing the `set_page_size()` call before `render_score()`, causing NGL files to render at wrong page dimensions in the Flutter app.
- [x] **Rehearsal mark enclosures**: Port of DrawEnclosure (DrawObject.cp:1490-1535). GRAPHIC objects with `enclosure != ENCL_NONE` get a framed rectangle around the text using OG defaults: 2pt margin (ENCLMARGIN_DFLT) and 1pt line width (ENCLLW_DFLT=4 quarter-points). Text width measured via `measure_text_width()` with character-count fallback. Only 17_capital_regiment_march affected (rehearsal marks A-F on 4 pages).

### Recently Completed (previous session)
- [x] **VexFlow-inspired Notelist test fixtures**: Expanded test coverage from 20 to 41 Notelist fixtures with 21 new focused tests for individual engraving features: accidentals (all 5 types), dotted notes (single/double), rests (all durations), ledger lines (extreme range A2–C7), beamed eighths (ascending/descending/flat/zigzag), mixed durations, whole notes, 16ths/32nds, quintuplet tuplets, grace notes, key signatures (1–7 sharps, 1–7 flats), time signature changes (4/4→3/4→6/8→2/4→5/4), barline types, text annotations, two voices, bass clef melody, wide intervals, chromatic scale, tied notes, and compound meter (6/8, 12/8). Each fixture has golden bitmap, insta snapshot, and command-stream hash for full regression coverage.
- [x] **Flutter app: file browser + multi-page rendering**: Rewrote the Flutter app from single-hardcoded-file viewer to a full score browser. File browser sidebar with directory tabs (NGL Fixtures / Notelists), auto-discovers test fixture dirs relative to working directory, click-to-render any .ngl or .nl file. Rust bridge additions: `renderScoreFromPath()` (filesystem load), `listScoreFiles()` (directory scan), `ScoreFileEntry` DTO. Multi-page rendering: BeginPage/EndPage commands offset canvas by page index with 16px gaps, page backgrounds with white rectangles/drop shadows. Proper barline rendering: single, double, final (thin+thick), repeat left/right/both with dots. Zoom slider (50%–400%), status bar, dark theme support.
- [x] **Multi-page bitmap regression**: Extended both NGL and Notelist bitmap tests to loop over ALL pages (was page 1 only). Golden bitmaps go from 58 to 491 PNGs covering every page of every fixture. Full visual regression coverage for multi-page scores.
- [x] **Page number rendering**: Port of DrawPageNum() from DrawObject.cp. Pages 2+ display a centered page number (10pt Helvetica) at the bottom margin. InterpretedScore gains page_width_pt, page_height_pt, and first_page_number fields (parsed from NGL document header orig_paper_rect or Notelist layout config).

### Recently Completed (previous sessions)
- [x] **Extra measure number fix**: Spurious measure number at the score's final barline in 13 of 17 NGL fixtures. Root cause: final barline's `measure_left` was 72 DDIST (N103) or 64 DDIST (N105) from `staff_right`, exceeding the 48 DDIST suppression threshold. Diagnostic context walk confirmed system-end barlines have dist=1, score-end dist=64-72, mid-system dist≥595. Increased threshold from 48 to 80 DDIST (5 points) — safely between score-end max (72) and mid-system min (595).
- [x] **Sonata→SMuFL character mapping**: Complete mapping of OG Nightingale Sonata font characters to SMuFL/Bravura codepoints (90+ characters). GRAPHIC text objects using Sonata font (e.g., segno '%', coda, dynamics) now detected and rendered via `music_char()` with correct SMuFL glyphs instead of wrong Helvetica text. Port of MapMusChar() concept from DrawUtils.cp. Covers: clefs, accidentals, time sigs, noteheads, flags, dots, articulations, dynamics, repeat dots, braces/brackets, segno (0x25→U+E047), coda (0x9E→U+E048), rests.
- [x] **Cancelling key signatures**: When a key signature changes to fewer accidentals (n_ks_items==0), naturals are now drawn at the positions of the previous key signature's accidentals. Port of DrawUtils.cp:988-1010 (LSSearch backward for previous keysig). Added `prev_ks_info` to Context struct, saved before each keysig update. SMUFL_NATURAL (U+E261) glyph rendering. 30 new natural glyphs appear in Capital Regiment March (the fixture with mid-score key changes).
- [x] **Flutter app revival**: Moved iceboxed `nightingale_app/` back to project root. Rewrote Rust bridge (`score.rs`) to match current 32-variant RenderCommand API with flat DTO (no nested structs). Added `render_notelist_from_text()` for Notelist support. Rewrote Dart `ScorePainter` to handle all 32 command kinds (was 11). Ran `flutter_rust_bridge_codegen` to regenerate bindings. `cargo check` + `flutter analyze` both clean. Removed stale iceboxed tests.
- [x] **OG Gourlay spacing pipeline**: Full port of the OG Nightingale spacing engine from SpaceTime.cp and SpaceHighLevel.cp. Replaces simple duration-proportional spacing with the complete Gourlay pipeline:
  - `SymWidthRight/Left` for computing horizontal extent of all object types (sync, grsync, measure, clef, keysig, timesig) in STDIST
  - `FIdealSpace` with spaceProp parameter for duration-proportional ideal widths
  - `ConsiderITWidths` for collision avoidance (ensures accidentals/dots/flags don't overlap)
  - `Respace1Bar` for per-measure Gourlay spacing with collision resolution
  - 22 unit tests covering all width functions and the complete pipeline
  - Integrated into to_score.rs steps 3-5: builds SpaceTimeInfo per measure, calls respace_1bar, scales to system width
  - All 20 notelist golden bitmaps updated with improved spacing
- [x] **Notelist grace note support**: Port of ConvertGRNoteRest (NotelistOpen.cp). Pre-scan G records, group by following Note/Rest time, create GrSync objects with ANote subobjects. Grace notes render at 70% via draw_grsync. Handles cross-barline grace notes. 3 targeted tests (HBD_33, Schoenberg, Mahler) verifying pitches, clefs, accidentals, and 70% rendering.
- [x] **Grace note rendering (draw_grsync)**: Port of DrawNRGR.cp DrawGRSYNC()/DrawGRNote(). 70% size noteheads, accidentals, ledger lines, stems, flags, diagonal stem slash on unbeamed eighth grace notes, augmentation dots. Wired into render loop dispatch. NGL parser ready but no NGL fixtures contain GrSync objects.
- [x] **FONT_* text style off-by-one fix**: FONT_MN=1, FONT_PN=2, etc. are 1-based constants but text_styles[] is 0-indexed. All four lookup sites (graphic text, measure numbers, part names, tempo font) were picking the wrong style. Composer text was rendered as 32pt italic Briard (FONT_R2) instead of 9pt Helvetica (FONT_R1). Fixed: text_styles[constant - 1].
- [x] **SMuFL glyph braces/brackets**: U+E000 brace glyph with 2× weight boost, U+E002 bracket glyph (both PdfRenderer + BitmapRenderer). Non-uniform text matrix scaling. Bezier/line fallback preserved.
- [x] **Beam/stem gap fix**: 0.5pt stem extension for beamed notes (port of PS_NoteStem's 8 DDIST, PS_Stdio.cp:1729).
- [x] **Tuplet bracket orientation fix**: Staff-relative DDIST comparison for bracket_below (was cross-domain comparison).
- [x] **Notelist tempo mark conversion**: NotelistRecord::Tempo → Tempo objects in to_score.rs. Positional anchoring (tempo applies at next note/rest). beat_char→l_dur mapping, verbal string + metronome mark. 7 .nl files affected (Debussy, GoodbyePorkPieHat, KillingMe, Mendelssohn, Schoenberg, TestMIDI, Webern).
- [x] **Tempo SMuFL note glyph fix**: tempo_glyph() was returning Sonata characters (both renderers early-return on Sonata variants). Replaced with SMuFL Individual Notes codepoints (U+E1D0-E1DF): quarter=0xE1D5, half=0xE1D3, etc. Also fixed augmentation dot to SMuFL U+E1E7.
- [x] **NGL tempo Y positioning fix**: TEMPO objects appear before MEASURE in NGL object list (PAGE→SYSTEM→STAFF→TEMPO→MEASURE→SYNC), so measure_top is still 0 when draw_tempo runs. Fixed by using staff_top directly (equivalent to OG's GetContext at the anchor). All 17 NGL fixtures now render tempo marks.

### Recently Completed (previous session)
- [x] **NGL slur rendering**: Filled tapered Bezier shapes (PS_Stdio.cp:1933 PS_Slur port). Two offset curves with configurable mid-line width (SLURMIDLW_DFLT=30). ASlur spline data from NGL files rendered directly.
- [x] **Notelist slur rendering**: Endpoint collection from stem_info slurred_l/slurred_r flags (NotelistSave.cp:130). IICreateAllSlurs-style voice-based matching (InternalInput.cp:881). SetSlurCtlPoints port with short/long blending thresholds + RotateSlurCtrlPts for slanted slurs (Slurs.cp:1021-1122). slurCurvature=50 vs tieCurvature=85.
- [x] **Final barline flush-right fix**: System-boundary and final barlines now use config.content_width() (staff_right) instead of computed measure edge.

### Next: Engraving & Layout (priority order — USER PRIORITIZED)

#### Tier 1 — High Priority (core multi-page & cross-staff engraving)
- [x] **Cross-system slurs**: Fixed early-return bug in draw_slur() that silently dropped ALL cross-system slurs (982 across 24 fixtures). The existing endpoint computation code already handled cross-system positions correctly — the only bug was a validation at line 990 that required both note endpoints, but cross-system slur pieces always have one boundary endpoint (SYSTEM or MEASURE). Split into per-piece validation: 1st piece needs first_note, 2nd piece needs last_note. Port of GetSlurContext logic (Slurs.cp:865-980).
- [x] **Pagination with page breaks**: NGL PAGE objects trigger begin_page/end_page in render_score(), with page-relative system_rect coordinates. Multi-page rendering fully functional: 622 golden bitmaps across 26 fixtures (up to 54 pages for 07_new_york_debutante). Page numbers rendered on pages 2+. No PageFixSysRects port needed — NGL files already store correct page-relative coordinates.
- [ ] **Cross-staff notation**: Notes/beams drawn on a different staff than their anchor (OG uses staffn vs voice assignment to handle piano cross-staff beaming, arpeggios across staves, etc. — port relevant logic from DrawNRGR.cp and Beam.cp). Advanced feature but needed for real piano scores.

#### Tier 1B — Already Complete
- [x] **Clef changes**: mid-score clef objects for Notelist pipeline — detects real type changes (filters system-boundary restatements), Gourlay spacing with OG formula (0.85*STD_LINEHT*4*0.75 STDIST), 75% small clefs (SMALLSIZE macro), NGL pipeline small flag. 4 Notelist + 7 NGL files affected. New clef_change.nl fixture (all 7 clef types).
- [x] **Tuplets**: render tuplet brackets/numbers (DrawTUPLET port from Tuplet.cp)
- [x] **Slurs** (single-system + cross-system): NGL filled tapered Beziers from ASlur data; Notelist endpoint collection + IICreateAllSlurs matching + SetSlurCtlPoints. Cross-system slurs fixed (see Tier 1 above).
- [x] **System layout / spacing improvements**: full OG Gourlay pipeline (SymWidthRight/Left, FIdealSpace, ConsiderITWidths, Respace1Bar) — duration-proportional spacing with collision avoidance
- [x] **Ottava (8va/8vb)**: OTTAVA_5 parsing (40 bytes, bitfields, ANOTEOTTAVA subobjects), draw_ottava() with Sonata italic digit glyphs (MCH_idigits), dashed bracket (hdashed_line), vertical cutoff, alta/bassa distinction. Port of DrawOTTAVA/DrawOctBracket/GetOctTypeNum from Ottava.cp. No test fixtures contain ottavas, but code compiles and is wired into render loop.

#### Tier 2 — Text & Markings
- [x] **Dynamics**: hairpin crescendo/diminuendo lines + dynamic text (pp, ff, etc.) (DrawDYNAMIC port from DrawObject.cp)
- [x] **Text attached to notes**: lyrics, expression text, other note-attached annotations (DrawGRAPHIC port from DrawObject.cp)
- [x] **Part names**: staff labels at start of first system (and abbreviated on continuation systems)
- [x] **Tempo markings**: TEMPO_5 parsing (38 bytes), verbal tempo string + optional metronome mark (note glyph + dot + "= N"). Font from FONT_TM text style. Port of DrawTEMPO/TempoGlyph/GetGraphicOrTempoDrawInfo from DrawObject.cp/DrawUtils.cp. Notelist pipeline: NotelistRecord::Tempo → Tempo objects with positional anchoring in to_score.rs.
- [x] **Score markings**: fermata, other articulations — all 22 MODNR types implemented (MOD_FERMATA through MOD_LONG_INVMORDENT). Tremolo slashes. Fingerings (0-5) acknowledged but need text rendering. Port of DrawModNR/GetModNRInfo from DrawNRGR.cp/DrawUtils.cp.
- [x] **Volta brackets (Endings)**: ENDING_5 parsing (32 bytes), horizontal bracket with optional left/right cutoffs, ending number labels. Port of DrawENDING from DrawObject.cp.
- [x] **Rehearsal marks**: boxed/circled text above system — port of DrawEnclosure (DrawObject.cp:1490-1535), ENCL_BOX type with 2pt margin and 1pt frame. Only 17_capital_regiment_march affected (rehearsal marks A-F on 4 pages).
- [x] **Common/cut time**: C and ₵ time signatures (DrawObject.cp C_TIME/CUT_TIME special cases → SMuFL U+E08A timeSigCommon / U+E08B timeSigCutCommon). Checks ATimeSig.header.sub_type and draws single centered glyph at half-line 4. Affects 5 fixtures: 05_abigail, 13_miss_b, 17_capital_regiment_march, tc_old_komm_heiliger_geist, tc_old_komm_heiliger_geist_qt.
- [ ] **RPTEND** (DrawObject.cp DrawRPTEND): segno (%), coda, D.C., D.S. al fine — repeat-to-end symbols on barlines. Not rendered at all.
- [x] **Alias clefs** (TREBLE8_CLEF=1, TRTENOR_CLEF=7, BASS8B_CLEF=11): Fixed 3 incorrect SMuFL glyph mappings in clef_glyph(). TREBLE8_CLEF→U+E053 (gClef8va, 8 above), TRTENOR_CLEF→U+E052 (gClef8vb, guitar/vocal 8 below), BASS8B_CLEF→U+E064 (fClef8vb, 8 below). SMuFL glyphs include the "8" indicator natively — no separate sub/superscript rendering needed. Affects 16 NGL fixtures that use TRTENOR_CLEF as default treble clef.
- [ ] **Header/footer text** (DrawHeaderFooter, DrawObject.cp): score title, composer, copyright on page 1 header; running headers/footers on subsequent pages. Not started.

#### Tier 3 — Engraving Polish
- [ ] **Arpeggio signs** (DrawObject.cp DrawArpSign, GRArpeggio): wavy vertical lines indicating rolled chords. Not rendered.
- [ ] **PSMEAS** (DrawObject.cp DrawPSMEAS): pseudo-measure marker used for partial measures and system overlaps. Not rendered.
- [ ] **GRDraw** (DrawObject.cp DrawGRDraw): arbitrary line-drawing GRAPHIC type (straight lines as score annotations). Not rendered.
- [x] **Grace notes**: small grace notes before principal notes — DrawGRSync rendering + Notelist G-record pipeline
- [x] **Notehead collision avoidance**: seconds in chords — ported ArrangeChordNotes (PitchUtils.cp) to objects.rs, NoteXLoc offset in draw_nrgr.rs, ChordNoteToLeft for accidental anchoring. Multi-voice X offsets still TODO.
- [x] **Accidental staggering**: port ArrangeNCAccs (PitchUtils.cp) → arrange_nc_accs (objects.rs)
- [x] **Final barline**: double barline at end of piece (already working — BAR_FINALDBL mapped and rendered)
- [x] **Anacrusis measure width**: proportional min-width floor for pickup measures — scale floor by `actual_dur / full_measure_dur`, clamped to 25% minimum. Port of implicit fraction-based narrowing in OG Respace1Bar (SpaceHighLevel.cp:899). Affects 3 fixtures: BinchoisDePlus_17, GoodbyePorkPieHat, TestMIDIChannels_3.
- [x] **Mid-score time signature changes**: pre-scan T records in Notelist pipeline, insert TimeSig objects with `in_measure=true`, Gourlay spacing + indent. Port of ConvertTimesig (NotelistOpen.cp:717-737). Affects 7 fixtures.
- [x] **Ledger line weight**: compute all line widths (staff, ledger, stem, barline) from lnSpace using OG config percentages (8%, 13%, 8%, 10%). Port of PS_Stdio.cp PS_Recompute() lines 2023-2048, Initialize.cp:952-955. Added first_staff_lnspace() helper. Affects all 491 golden bitmaps.
- [x] **Rest rendering improvements**: added missing 32nd/64th/128th rest glyph mappings (SMuFL U+E4E8–E4EA), added pseudo-ledger lines for whole/half rests positioned outside the staff (port of DrawNRGR.cp lines 1329-1342). All rest durations breve through 128th now render correctly.

#### Tier 4 — Advanced Layout
- [ ] **Cross-staff notation**: notes/beams drawn on a different staff than they belong to (OG uses staffn vs voice assignment to handle piano cross-staff beaming, arpeggios across staves, etc. — port relevant logic from DrawNRGR.cp and Beam.cp)

### Module Refactor — COMPLETE
Reorganized code to mirror OG Nightingale C source file organization, with shared
modules used by both the NGL binary pipeline and Notelist text pipeline:

#### Shared modules (1:1 with OG C files)
- [x] `pitch_utils.rs` <- PitchUtils.cp: nl_midi_to_half_ln, clef_middle_c_half_ln, half_ln_to_yd
- [x] `utility.rs` <- Utility.cp: calc_ystem, nflags, head_width, std2d, acc_x_offset
- [x] `music_font.rs` <- MusicFont.cp: stem_space_width_ddist
- [x] `objects.rs` <- Objects.cp: VoiceRole, normal_stem_up_down_single/chord, get_nc_ystem, setup_ks_info
- [x] `beam.rs` <- Beam.cp: BeamNoteInfo, compute_beam_slope
- [x] `space_time.rs` <- SpaceTime.cp + SpaceHighLevel.cp: ideal_space, stdist_to_ddist, SymWidthRight/Left, ConsiderITWidths, Respace1Bar

#### draw/ submodules (1:1 with OG C files)
- [x] `draw/draw_high_level.rs` <- DrawHighLevel.cp: render_score() main loop
- [x] `draw/draw_object.rs` <- DrawObject.cp: staff, measure, connect, clef, keysig, timesig, ties
- [x] `draw/draw_nrgr.rs` <- DrawNRGR.cp: sync (notes/rests), ledger lines, tie endpoints
- [x] `draw/draw_utils.rs` <- DrawUtils.cp: glyph mapping, key signature Y offsets
- [x] `draw/draw_beam.rs` <- DrawBeam.cp: beam sets
- [x] `draw/draw_tuplet.rs` <- Tuplet.cp: tuplet brackets and numbers
- [x] `draw/helpers.rs`: d2r_sum, d2r_sum3, count_staves, TieEndpoint, lnspace_for_staff

#### to_score.rs deduplication
- [x] Replaced 7 inline implementations with calls to shared modules (-154 lines)

### Known Bugs
- [x] ~~Treble clefs render one staff line too high (B instead of G) for NGL files~~ — **Fixed**: NGL files use TRTENOR_CLEF (sub_type=7) not TREBLE_CLEF (3); added all 12 clef types to clef_glyph() and clef_halfline_position()

### Deferred
- [x] Port MapMusChar() (Sonata->SMuFL glyph mapping) — done in draw_utils.rs
- [ ] SMuFL metadata loading (anchors, engraving defaults)
- [ ] .ngl binary writer
- [x] **N105 format support** — DONE (both N103 and N105 fully supported via unpack_*_n105() functions)

## Phase 3: Engraving Engine — PARTIALLY IN PROGRESS
- [x] Port Beam.cp GetBeamEndYStems/FixSyncInBeamset -> beam.rs (shared)
- [x] Port Objects.cp NormalStemUpDown -> objects.rs (shared)
- [x] Port SpaceTime.cp + SpaceHighLevel.cp -> space_time.rs: IdealSpace, SymWidthRight/Left, ConsiderITWidths, Respace1Bar (complete Gourlay pipeline)
- [x] Port Slurs.cp -> slur module (including cross-system/page slurs)
- [x] Port Tuplet.cp -> tuplet rendering (DrawTUPLET/DrawPSTupletBracket)
- [ ] Port SFormat.cp / SFormatHighLevel.cp -> format module (pagination, system layout)
- [x] Port DrawObject.cp OTTAVA/DYNAMIC/GRAPHIC/TEMPO/ENDING sections

## Rendering & Testing Architecture (DECIDED)

Three rendering layers, all implementing the same `MusicRenderer` trait:

### Layer 1: PdfRenderer (done)
PDF output via `pdf-writer`. Maps MusicRenderer methods to PDF content stream operators.
Used for document export / printing. Embeds Bravura SMuFL font.

### Layer 2: BitmapRenderer (next up)
Pure-Rust bitmap rendering via `tiny-skia` + `ttf-parser`/`ab_glyph`.
Used for **test-loop visual regression** — runs in `cargo test` with zero external
dependencies. Produces per-page PNGs directly (no PDF→PNG conversion tooling needed).
Enables the autonomous render→inspect→fix cycle.

### Layer 3: Flutter Canvas (production UI)
Flutter's Skia-backed Canvas via `flutter_rust_bridge`. The real UI renderer.
Architecture: Rust sends `RenderCommand` stream → Dart replays on Canvas → CustomPaint.

### Visual regression strategy
- **Rust tests** (`cargo test`): BitmapRenderer produces PNGs, compared against golden
  bitmaps pixel-by-pixel. Fast, CI-friendly, no system dependencies.
- **Flutter golden tests** (`flutter test`): CommandRenderer captures command stream →
  Dart test replays on real Flutter Canvas → `matchesGoldenFile()` comparison.
  Ensures visual fidelity with production output.
- **Command-stream hashes**: structural regression check independent of rendering.

### Flutter visual diff UI (future)
Interactive diff review tool built in Flutter. Features:
- **Overlay mode**: superimpose old/new renders with variable opacity slider
- **A/B toggle**: tap to flip between before/after
- **Zoom & pan**: inspect fine details (stem widths, glyph alignment)
- **Batch approval**: review all changed goldens, approve/reject per-file
- **Side-by-side**: old | diff | new (like the current HTML report, but interactive)

This replaces the current static HTML diff report and enables rapid human approval
of intentional rendering changes during the engraving polish phase.

### VexFlow test suite — DONE
21 VexFlow-inspired Notelist test fixtures covering individual engraving features.
See `tests/notelist_examples/` for the full set (41 total fixtures).

## Phase 4: Flutter Shell — IN PROGRESS
- [x] flutter_rust_bridge setup (v2.11.1, flat DTO bridge)
- [x] FlutterRenderer backend (command-based -> CustomPaint, 32 command types)
- [x] Score view widget with multi-page rendering, zoom, file browser
- [ ] Tool palette, basic editing

## Phase 5: MusicXML — NOT STARTED (de-prioritized)
- [ ] MusicXML 4.0 export/import
- [ ] Validate against MuseScore / Dorico round-trip

## Phase 6: Sound Playback / MIDI — NOT STARTED
- [ ] MIDI export (port NightingaleMIDI.cp / MIDIRecieveGlobals.cp logic — duration/pitch/velocity/channel)
- [ ] Real-time playback via Flutter (use `flutter_midi_pro` or platform MIDI API; Rust emits events, Flutter drives the synth)
- [ ] Metronome / click track
- [ ] General MIDI soundfont support (optional; default to platform synth)
- [ ] Tempo map from TEMPO objects (ritardandi, accelerandi not currently modelled)
- [ ] Playback cursor: highlight currently playing measure/note in the UI

## Stats
| Metric | Value |
|--------|-------|
| Rust source lines | ~24,600 |
| Rust test lines | ~5,500 |
| Test count | 350 (346 passed + 4 ignored) |
| Test fixture files | 17 .ngl + 41 .nl |
| Insta snapshots | 59 |
| Bitmap goldens | 491 (17 NGL + 41 Notelist, all pages) |
| Modules | 18 (basic_types, beam, context, defs, doc_types, draw, duration, limits, music_font, ngl, notelist, obj_types, objects, pitch_utils, render, space_time, utility, lib) |
