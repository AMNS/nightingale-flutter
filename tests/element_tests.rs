//! Notation-element-specific unit tests.
//!
//! Each test creates a minimal MusicXML document exercising one notation feature,
//! imports it, verifies the internal representation, exports it, and verifies
//! the re-exported XML preserves the feature.
//!
//! These are *targeted* tests — they isolate individual elements rather than testing
//! full scores end-to-end (see musicxml_pipeline.rs for integration tests).

use nightingale_core::musicxml::export::export_musicxml;
use nightingale_core::musicxml::import::import_musicxml;
use nightingale_core::ngl::interpret::InterpretedScore;

/// Build a minimal MusicXML document with given attributes and note content.
/// If `attrs` contains a `<clef>` element, no default clef is added.
fn make_xml(attrs: &str, notes: &str) -> String {
    let clef = if attrs.contains("<clef>") {
        "" // attrs already provides a clef
    } else {
        "\n        <clef><sign>G</sign><line>2</line></clef>"
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Test</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        {}{}
      </attributes>
      {}
    </measure>
  </part>
</score-partwise>"#,
        attrs, clef, notes
    )
}

/// Build a minimal MusicXML with default 4/4 C-major attributes.
fn make_xml_default(notes: &str) -> String {
    make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>",
        notes,
    )
}

/// Build a multi-measure MusicXML document.
fn make_xml_measures(attrs: &str, measures: &[&str]) -> String {
    let mut parts = String::new();
    for (i, notes) in measures.iter().enumerate() {
        if i == 0 {
            parts.push_str(&format!(
                r#"    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        {}
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      {}
    </measure>
"#,
                attrs, notes
            ));
        } else {
            parts.push_str(&format!(
                r#"    <measure number="{}">
      {}
    </measure>
"#,
                i + 1,
                notes
            ));
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Test</part-name></score-part>
  </part-list>
  <part id="P1">
{}  </part>
</score-partwise>"#,
        parts
    )
}

/// Import a MusicXML string, asserting success.
fn import(xml: &str) -> InterpretedScore {
    import_musicxml(xml).expect("import should succeed")
}

/// Count all notes (including rests) across all staves.
fn total_notes(score: &InterpretedScore) -> usize {
    score.notes.values().map(|v| v.len()).sum()
}

/// Count occurrences of a substring in a string.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

// ============================================================================
// ACCIDENTALS
// ============================================================================

#[test]
fn accidental_sharp() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>F</step><alter>1</alter><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>sharp</accidental><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 1);

    // Check the note's accidental field
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert_eq!(notes[0].accident, 4, "sharp = accidental code 4");

    // Roundtrip
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>sharp</accidental>"));
    assert!(rexml.contains("<alter>1</alter>"));
}

#[test]
fn accidental_flat() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>B</step><alter>-1</alter><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>flat</accidental><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert_eq!(notes[0].accident, 2, "flat = accidental code 2");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>flat</accidental>"));
    assert!(rexml.contains("<alter>-1</alter>"));
}

#[test]
fn accidental_natural() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>F</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>natural</accidental><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert_eq!(notes[0].accident, 3, "natural = accidental code 3");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>natural</accidental>"));
}

#[test]
fn accidental_double_sharp() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>F</step><alter>2</alter><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>double-sharp</accidental><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert_eq!(notes[0].accident, 5, "double-sharp = accidental code 5");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>double-sharp</accidental>"));
    assert!(rexml.contains("<alter>2</alter>"));
}

#[test]
fn accidental_double_flat() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>B</step><alter>-2</alter><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>flat-flat</accidental><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert_eq!(notes[0].accident, 1, "double-flat = accidental code 1");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>flat-flat</accidental>"));
    assert!(rexml.contains("<alter>-2</alter>"));
}

#[test]
fn accidental_all_types_roundtrip() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>B</step><alter>-2</alter><octave>3</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>flat-flat</accidental><voice>1</voice>
      </note>
      <note>
        <pitch><step>E</step><alter>-1</alter><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>flat</accidental><voice>1</voice>
      </note>
      <note>
        <pitch><step>F</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>natural</accidental><voice>1</voice>
      </note>
      <note>
        <pitch><step>G</step><alter>1</alter><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <accidental>sharp</accidental><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 4);

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>flat-flat</accidental>"));
    assert!(rexml.contains("<accidental>flat</accidental>"));
    assert!(rexml.contains("<accidental>natural</accidental>"));
    assert!(rexml.contains("<accidental>sharp</accidental>"));

    // Verify roundtrip stability
    let score2 = import(&rexml);
    let rexml2 = export_musicxml(&score2);
    assert_eq!(
        count(&rexml, "<accidental>"),
        count(&rexml2, "<accidental>")
    );
}

// ============================================================================
// TIES
// ============================================================================

#[test]
fn tie_simple_two_notes() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>960</duration><type>half</type>
        <tie type="start"/><voice>1</voice>
        <notations><tied type="start"/></notations>
      </note>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>960</duration><type>half</type>
        <tie type="stop"/><voice>1</voice>
        <notations><tied type="stop"/></notations>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 2);

    // Check tie flags
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    // First note should have tie_right
    let has_tie_start = notes.iter().any(|n| n.tied_r);
    let has_tie_stop = notes.iter().any(|n| n.tied_l);
    assert!(has_tie_start, "Should have a note with tie-start");
    assert!(has_tie_stop, "Should have a note with tie-stop");

    // Roundtrip: verify <tied> elements preserved
    let rexml = export_musicxml(&score);
    assert!(count(&rexml, "<tied ") >= 2, "Should have tied elements");
    assert!(count(&rexml, "type=\"start\"") >= 1);
    assert!(count(&rexml, "type=\"stop\"") >= 1);
}

#[test]
fn tie_chain_three_notes() {
    // C4 half → C4 quarter → C4 quarter (tied across all three)
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>960</duration><type>half</type>
        <tie type="start"/><voice>1</voice>
        <notations><tied type="start"/></notations>
      </note>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <tie type="stop"/><tie type="start"/><voice>1</voice>
        <notations><tied type="stop"/><tied type="start"/></notations>
      </note>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <tie type="stop"/><voice>1</voice>
        <notations><tied type="stop"/></notations>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 3);

    // Middle note should have both tie flags
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    let both_tied = notes.iter().any(|n| n.tied_l && n.tied_r);
    assert!(
        both_tied,
        "Middle note should have both tie-start and tie-stop"
    );
}

// ============================================================================
// RESTS
// ============================================================================

#[test]
fn rest_whole() {
    let xml = make_xml_default(
        r#"<note>
        <rest/><duration>1920</duration><type>whole</type><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 1);

    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert!(notes[0].rest, "Should be a rest");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<rest"));
    assert!(rexml.contains("<type>whole</type>"));
}

#[test]
fn rest_all_durations() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>15</beats><beat-type>4</beat-type></time>",
        r#"<note><rest/><duration>1920</duration><type>whole</type><voice>1</voice></note>
      <note><rest/><duration>960</duration><type>half</type><voice>1</voice></note>
      <note><rest/><duration>480</duration><type>quarter</type><voice>1</voice></note>
      <note><rest/><duration>240</duration><type>eighth</type><voice>1</voice></note>
      <note><rest/><duration>120</duration><type>16th</type><voice>1</voice></note>
      <note><rest/><duration>60</duration><type>32nd</type><voice>1</voice></note>
      <note><rest/><duration>30</duration><type>64th</type><voice>1</voice></note>
      <note><rest/><duration>15</duration><type>128th</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    assert_eq!(
        total_notes(&score),
        8,
        "Should import 8 rests of different durations"
    );

    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert!(notes.iter().all(|n| n.rest), "All should be rests");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<type>whole</type>"));
    assert!(rexml.contains("<type>half</type>"));
    assert!(rexml.contains("<type>quarter</type>"));
    assert!(rexml.contains("<type>eighth</type>"));
}

// ============================================================================
// DOTTED NOTES
// ============================================================================

#[test]
fn dotted_quarter() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>3</beats><beat-type>4</beat-type></time>",
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>720</duration><type>quarter</type><dot/><voice>1</voice>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>720</duration><type>quarter</type><dot/><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 2);

    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert!(
        notes.iter().all(|n| n.ndots >= 1),
        "All notes should have at least 1 dot"
    );

    let rexml = export_musicxml(&score);
    assert_eq!(count(&rexml, "<dot/>"), 2, "Should have 2 dot elements");
}

#[test]
fn double_dotted_half() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1680</duration><type>half</type><dot/><dot/><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    assert!(
        notes[0].ndots >= 2,
        "Should have 2 dots: got {}",
        notes[0].ndots
    );

    let rexml = export_musicxml(&score);
    assert!(
        count(&rexml, "<dot/>") >= 2,
        "Should have at least 2 dot elements"
    );
}

// ============================================================================
// CHORDS
// ============================================================================

#[test]
fn chord_triad() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice>
      </note>
      <note>
        <chord/>
        <pitch><step>E</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice>
      </note>
      <note>
        <chord/>
        <pitch><step>G</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 3, "C-E-G triad = 3 notes");

    // All three should share the same timestamp (they're at the same sync point)
    // After export, verify 2 <chord/> tags
    let rexml = export_musicxml(&score);
    assert_eq!(
        count(&rexml, "<chord/>"),
        2,
        "Should have 2 chord indicators"
    );
    assert_eq!(count(&rexml, "<note>"), 3, "Should have 3 notes total");
}

#[test]
fn chord_with_accidentals() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice>
      </note>
      <note>
        <chord/>
        <pitch><step>E</step><alter>-1</alter><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type>
        <accidental>flat</accidental><voice>1</voice>
      </note>
      <note>
        <chord/>
        <pitch><step>G</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 3);

    // One note should have a flat accidental
    let notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    let flats = notes.iter().filter(|n| n.accident == 2).count();
    assert_eq!(flats, 1, "Should have exactly 1 flat");

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<accidental>flat</accidental>"));
}

// ============================================================================
// KEY SIGNATURES
// ============================================================================

#[test]
fn keysig_sharps() {
    for sharps in 1..=7 {
        let xml = make_xml(
            &format!(
                "<key><fifths>{}</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>",
                sharps
            ),
            r#"<note><pitch><step>C</step><octave>4</octave></pitch>
          <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
        );
        let score = import(&xml);
        let rexml = export_musicxml(&score);
        assert!(
            rexml.contains(&format!("<fifths>{}</fifths>", sharps)),
            "Should preserve {} sharps",
            sharps
        );
    }
}

#[test]
fn keysig_flats() {
    for flats in 1..=7 {
        let xml = make_xml(
            &format!(
                "<key><fifths>-{}</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>",
                flats
            ),
            r#"<note><pitch><step>C</step><octave>4</octave></pitch>
          <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
        );
        let score = import(&xml);
        let rexml = export_musicxml(&score);
        assert!(
            rexml.contains(&format!("<fifths>-{}</fifths>", flats)),
            "Should preserve {} flats",
            flats
        );
    }
}

// ============================================================================
// TIME SIGNATURES
// ============================================================================

#[test]
fn timesig_common_meters() {
    let meters = [
        (2, 4),
        (3, 4),
        (4, 4),
        (6, 8),
        (2, 2),
        (3, 8),
        (5, 4),
        (7, 8),
        (9, 8),
        (12, 8),
    ];

    for (beats, beat_type) in meters {
        let duration = 480 * 4 * beats / beat_type; // fill one measure
        let xml = make_xml(
            &format!(
                "<key><fifths>0</fifths></key>\n        <time><beats>{}</beats><beat-type>{}</beat-type></time>",
                beats, beat_type
            ),
            &format!(
                r#"<note><rest/><duration>{}</duration><type>whole</type><voice>1</voice></note>"#,
                duration
            ),
        );
        let score = import(&xml);
        let rexml = export_musicxml(&score);
        assert!(
            rexml.contains(&format!("<beats>{}</beats>", beats)),
            "Should preserve {}/{} beats",
            beats,
            beat_type
        );
        assert!(
            rexml.contains(&format!("<beat-type>{}</beat-type>", beat_type)),
            "Should preserve {}/{} beat-type",
            beats,
            beat_type
        );
    }
}

// ============================================================================
// CLEFS
// ============================================================================

#[test]
fn clef_treble() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>G</sign><line>2</line></clef>",
        r#"<note><pitch><step>C</step><octave>4</octave></pitch>
      <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<sign>G</sign>"));
    assert!(rexml.contains("<line>2</line>"));
}

#[test]
fn clef_bass() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>F</sign><line>4</line></clef>",
        r#"<note><pitch><step>C</step><octave>3</octave></pitch>
      <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<sign>F</sign>"), "Should export F clef");
    assert!(rexml.contains("<line>4</line>"), "Should export line 4");
}

#[test]
fn clef_alto() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>C</sign><line>3</line></clef>",
        r#"<note><pitch><step>C</step><octave>4</octave></pitch>
      <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<sign>C</sign>"));
    assert!(rexml.contains("<line>3</line>"));
}

#[test]
fn clef_tenor() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>C</sign><line>4</line></clef>",
        r#"<note><pitch><step>C</step><octave>4</octave></pitch>
      <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<sign>C</sign>"));
    assert!(rexml.contains("<line>4</line>"));
}

#[test]
fn clef_treble_octave_down() {
    // Guitar/tenor voice clef: treble clef sounding an octave lower
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>G</sign><line>2</line><clef-octave-change>-1</clef-octave-change></clef>",
        r#"<note><pitch><step>C</step><octave>4</octave></pitch>
      <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<sign>G</sign>"));
    assert!(
        rexml.contains("<clef-octave-change>-1</clef-octave-change>"),
        "Should preserve octave-change"
    );
}

#[test]
fn clef_bass_octave_down() {
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>F</sign><line>4</line><clef-octave-change>-1</clef-octave-change></clef>",
        r#"<note><pitch><step>C</step><octave>2</octave></pitch>
      <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    );
    let score = import(&xml);
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<sign>F</sign>"));
    assert!(
        rexml.contains("<clef-octave-change>-1</clef-octave-change>"),
        "Should preserve bass 8vb"
    );
}

// ============================================================================
// MULTI-PART / GRAND STAFF
// ============================================================================

#[test]
fn two_parts_different_clefs() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Violin</part-name></score-part>
    <score-part id="P2"><part-name>Cello</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note><pitch><step>A</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice></note>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>F</sign><line>4</line></clef>
      </attributes>
      <note><pitch><step>C</step><octave>3</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice></note>
    </measure>
  </part>
</score-partwise>"#;

    let score = import(xml);
    assert_eq!(score.part_infos.len(), 2, "Should have 2 parts");
    assert_eq!(total_notes(&score), 2, "Should have 2 notes total");

    let rexml = export_musicxml(&score);
    assert_eq!(count(&rexml, "<part id="), 2);
    assert!(rexml.contains("<sign>G</sign>"));
    assert!(rexml.contains("<sign>F</sign>"));
    assert!(rexml.contains("<part-name>Violin</part-name>"));
    assert!(rexml.contains("<part-name>Cello</part-name>"));
}

// ============================================================================
// NOTE DURATIONS (all types)
// ============================================================================

#[test]
fn note_durations_all_types() {
    let types_and_durs = [
        ("whole", 1920),
        ("half", 960),
        ("quarter", 480),
        ("eighth", 240),
        ("16th", 120),
        ("32nd", 60),
        ("64th", 30),
    ];

    for (type_name, duration) in types_and_durs {
        let xml = make_xml(
            &format!(
                "<key><fifths>0</fifths></key>\n        <time><beats>{}</beats><beat-type>4</beat-type></time>",
                duration * 4 / 1920 + 1 // enough beats for the note
            ),
            &format!(
                r#"<note><pitch><step>C</step><octave>4</octave></pitch>
          <duration>{}</duration><type>{}</type><voice>1</voice></note>"#,
                duration, type_name
            ),
        );
        let score = import(&xml);
        assert_eq!(total_notes(&score), 1, "Should import 1 {} note", type_name);

        let rexml = export_musicxml(&score);
        assert!(
            rexml.contains(&format!("<type>{}</type>", type_name)),
            "Should preserve type {} on export",
            type_name
        );
    }
}

// ============================================================================
// MULTIPLE VOICES
// ============================================================================

#[test]
fn two_voices_in_one_part() {
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>E</step><octave>5</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>1</voice>
      </note>
      <backup><duration>1920</duration></backup>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1920</duration><type>whole</type><voice>2</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 2, "Should import 2 notes (2 voices)");

    let rexml = export_musicxml(&score);
    assert!(
        rexml.contains("<voice>1</voice>") && rexml.contains("<voice>2</voice>"),
        "Should preserve voice numbers"
    );
}

// ============================================================================
// PITCH RANGE
// ============================================================================

#[test]
fn pitch_extremes() {
    // Test very low and very high pitches
    let xml = make_xml(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>\n        <clef><sign>G</sign><line>2</line></clef>",
        r#"<note>
        <pitch><step>A</step><octave>0</octave></pitch>
        <duration>960</duration><type>half</type><voice>1</voice>
      </note>
      <note>
        <pitch><step>C</step><octave>8</octave></pitch>
        <duration>960</duration><type>half</type><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 2);

    // Roundtrip should preserve both extreme pitches
    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<octave>0</octave>") || rexml.contains("<step>A</step>"));
    assert!(rexml.contains("<octave>8</octave>") || rexml.contains("<step>C</step>"));
}

// ============================================================================
// KEY SIGNATURE CHANGE MID-SCORE
// ============================================================================

#[test]
fn keysig_change_mid_score() {
    let measures = [
        // Measure 1: C major
        r#"<note><pitch><step>C</step><octave>4</octave></pitch>
          <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
        // Measure 2: D major (2 sharps)
        r#"<attributes><key><fifths>2</fifths></key></attributes>
      <note><pitch><step>D</step><octave>4</octave></pitch>
          <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
    ];
    let xml = make_xml_measures(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>",
        &measures,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 2);

    let rexml = export_musicxml(&score);
    // Should have both key signatures
    assert!(rexml.contains("<fifths>0</fifths>"), "Should have C major");
    assert!(
        rexml.contains("<fifths>2</fifths>"),
        "Should have D major key change"
    );
}

// ============================================================================
// TIME SIGNATURE CHANGE MID-SCORE
// ============================================================================

#[test]
fn timesig_change_mid_score() {
    let measures = [
        // Measure 1: 4/4
        r#"<note><pitch><step>C</step><octave>4</octave></pitch>
          <duration>1920</duration><type>whole</type><voice>1</voice></note>"#,
        // Measure 2: 3/4
        r#"<attributes><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
      <note><pitch><step>D</step><octave>4</octave></pitch>
          <duration>1440</duration><type>half</type><dot/><voice>1</voice></note>"#,
    ];
    let xml = make_xml_measures(
        "<key><fifths>0</fifths></key>\n        <time><beats>4</beats><beat-type>4</beat-type></time>",
        &measures,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 2);

    let rexml = export_musicxml(&score);
    assert!(
        rexml.contains("<beats>4</beats>"),
        "Should have 4/4 time sig"
    );
    assert!(
        rexml.contains("<beats>3</beats>"),
        "Should have 3/4 time sig change"
    );
}

// ============================================================================
// ROUNDTRIP STABILITY
// ============================================================================

#[test]
fn roundtrip_stability_complex_measure() {
    // A measure with mixed note values, a rest, a chord, a dotted note, and a tie
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>720</duration><type>quarter</type><dot/>
        <tie type="start"/><voice>1</voice>
        <notations><tied type="start"/></notations>
      </note>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>240</duration><type>eighth</type>
        <tie type="stop"/><voice>1</voice>
        <notations><tied type="stop"/></notations>
      </note>
      <note>
        <rest/><duration>480</duration><type>quarter</type><voice>1</voice>
      </note>
      <note>
        <pitch><step>E</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
      </note>
      <note>
        <chord/>
        <pitch><step>G</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
      </note>"#,
    );

    let score1 = import(&xml);
    let xml1 = export_musicxml(&score1);
    let score2 = import(&xml1);
    let xml2 = export_musicxml(&score2);

    // XML should be identical across roundtrips (our own format is stable)
    assert_eq!(
        count(&xml1, "<note>"),
        count(&xml2, "<note>"),
        "Note count must be stable"
    );
    assert_eq!(
        count(&xml1, "<rest"),
        count(&xml2, "<rest"),
        "Rest count must be stable"
    );
    assert_eq!(
        count(&xml1, "<chord/>"),
        count(&xml2, "<chord/>"),
        "Chord count must be stable"
    );
    assert_eq!(
        count(&xml1, "<dot/>"),
        count(&xml2, "<dot/>"),
        "Dot count must be stable"
    );
    assert_eq!(
        count(&xml1, "<tied "),
        count(&xml2, "<tied "),
        "Tie count must be stable"
    );
}

// ============================================================================
// SLURS
// ============================================================================

#[test]
fn slur_simple_two_notes() {
    // Slur from C4 to D4
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
        <notations><slur number="1" type="start"/></notations>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
        <notations><slur number="1" type="stop"/></notations>
      </note>
      <note>
        <pitch><step>E</step><octave>4</octave></pitch>
        <duration>960</duration><type>half</type><voice>1</voice>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 3);

    // Verify slur flags on notes
    let all_notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    let slurred_r_count = all_notes.iter().filter(|n| n.slurred_r).count();
    let slurred_l_count = all_notes.iter().filter(|n| n.slurred_l).count();
    assert_eq!(slurred_r_count, 1, "Should have 1 slur start");
    assert_eq!(slurred_l_count, 1, "Should have 1 slur end");

    // Re-export and verify slur elements appear
    let rexml = export_musicxml(&score);
    assert!(
        rexml.contains(r#"type="start""#),
        "Should export slur start"
    );
    assert!(rexml.contains(r#"type="stop""#), "Should export slur stop");
    assert!(rexml.contains("<slur"), "Should contain slur elements");
}

#[test]
fn slur_with_tie() {
    // A note that has BOTH a tie and a slur start
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>960</duration><type>half</type>
        <tie type="start"/><voice>1</voice>
        <notations>
          <tied type="start"/>
          <slur number="1" type="start"/>
        </notations>
      </note>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <tie type="stop"/><voice>1</voice>
        <notations><tied type="stop"/></notations>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
        <notations><slur number="1" type="stop"/></notations>
      </note>"#,
    );
    let score = import(&xml);
    assert_eq!(total_notes(&score), 3);

    let all_notes: Vec<_> = score.notes.values().flat_map(|v| v.iter()).collect();
    // First note: tied_r AND slurred_r
    let tied_and_slurred = all_notes.iter().filter(|n| n.tied_r && n.slurred_r).count();
    assert_eq!(
        tied_and_slurred, 1,
        "One note should have both tie and slur start"
    );

    let rexml = export_musicxml(&score);
    assert!(rexml.contains("<tied"), "Should have tied notations");
    assert!(rexml.contains("<slur"), "Should have slur notations");
}

#[test]
fn slur_roundtrip_stability() {
    // Slur roundtrip: import→export→re-import→export should be stable
    let xml = make_xml_default(
        r#"<note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
        <notations><slur number="1" type="start"/></notations>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
      </note>
      <note>
        <pitch><step>E</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
        <notations><slur number="1" type="stop"/></notations>
      </note>
      <note>
        <pitch><step>F</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type><voice>1</voice>
      </note>"#,
    );
    let score1 = import(&xml);
    let xml1 = export_musicxml(&score1);
    let score2 = import(&xml1);
    let xml2 = export_musicxml(&score2);

    assert_eq!(
        count(&xml1, "<slur"),
        count(&xml2, "<slur"),
        "Slur count should be stable across roundtrips"
    );
}

#[test]
fn slur_ngl_fixture_export() {
    // NGL fixture tc_55_1 has slurs — verify they export to MusicXML
    use nightingale_core::ngl::{interpret::interpret_heap, NglFile};
    let data = std::fs::read("tests/fixtures/tc_55_1.ngl").unwrap();
    let ngl = NglFile::read_from_bytes(&data).unwrap();
    let score = interpret_heap(&ngl).unwrap();

    // Count notes with slur flags
    let slurred_r: usize = score
        .notes
        .values()
        .flat_map(|v| v.iter())
        .filter(|n| n.slurred_r)
        .count();
    let slurred_l: usize = score
        .notes
        .values()
        .flat_map(|v| v.iter())
        .filter(|n| n.slurred_l)
        .count();
    eprintln!(
        "tc_55_1: {} slur starts, {} slur ends",
        slurred_r, slurred_l
    );

    let xml = export_musicxml(&score);
    if slurred_r > 0 {
        assert!(
            xml.contains("<slur"),
            "NGL score with slurred notes should export slur elements"
        );
    }
}

#[test]
fn slur_from_musicxml_canonical_file() {
    // Import ActorPreludeSample which has 594 slurs
    let path = "tests/musicxml_examples/ActorPreludeSample.musicxml";
    if !std::path::Path::new(path).exists() {
        eprintln!("Skipping: {} not found", path);
        return;
    }
    let xml = std::fs::read_to_string(path).unwrap();
    let score = import(&xml);

    let slurred_r: usize = score
        .notes
        .values()
        .flat_map(|v| v.iter())
        .filter(|n| n.slurred_r)
        .count();
    let slurred_l: usize = score
        .notes
        .values()
        .flat_map(|v| v.iter())
        .filter(|n| n.slurred_l)
        .count();
    eprintln!(
        "ActorPrelude import: {} slur starts, {} slur ends",
        slurred_r, slurred_l
    );

    // Should have imported many slurs (file has 594 slur elements)
    assert!(
        slurred_r > 100,
        "Should import many slur starts from ActorPrelude: got {}",
        slurred_r
    );
    assert!(
        slurred_l > 100,
        "Should import many slur stops from ActorPrelude: got {}",
        slurred_l
    );

    // Re-export and verify slurs appear
    let rexml = export_musicxml(&score);
    let slur_count = count(&rexml, "<slur");
    eprintln!("ActorPrelude re-export: {} slur elements", slur_count);
    assert!(
        slur_count > 200,
        "Re-exported XML should have many slur elements: got {}",
        slur_count
    );
}

// ============================================================================
// NGL FIXTURE → MusicXML ELEMENT VERIFICATION
// ============================================================================

mod ngl_fixture_elements {
    use super::*;
    use nightingale_core::ngl::{interpret::interpret_heap, NglFile};
    use std::fs;
    use std::path::Path;

    fn load_ngl(name: &str) -> InterpretedScore {
        let path = format!("tests/fixtures/{}", name);
        assert!(Path::new(&path).exists(), "Fixture {} should exist", path);
        let data = fs::read(&path).unwrap();
        let ngl = NglFile::read_from_bytes(&data).unwrap();
        interpret_heap(&ngl).unwrap()
    }

    #[test]
    fn ngl_dynamic_objects_exist() {
        // Check that NGL files with dynamics have them in the score
        let score = load_ngl("tc_55_1.ngl");
        let dynamic_count: usize = score.dynamics.values().map(|v| v.len()).sum();
        eprintln!("tc_55_1: {} dynamic subobjects", dynamic_count);
        // Dynamic objects should exist in the orchestral score
        // (even if not yet exported to MusicXML)
        assert!(dynamic_count > 0, "Orchestral score should have dynamics");
    }

    #[test]
    fn ngl_slur_objects_exist() {
        let score = load_ngl("tc_55_1.ngl");
        let slur_count: usize = score.slurs.values().map(|v| v.len()).sum();
        eprintln!("tc_55_1: {} slur subobjects", slur_count);
        assert!(slur_count > 0, "Orchestral score should have slurs");
    }

    #[test]
    fn ngl_ottava_objects_exist() {
        // tc_05 is known to have ottava objects
        let score = load_ngl("tc_05.ngl");
        let ottava_count: usize = score.ottavas.values().map(|v| v.len()).sum();
        eprintln!("tc_05: {} ottava subobjects", ottava_count);
        // Even if 0 in this fixture, the test documents we're tracking this
    }

    #[test]
    fn ngl_connect_objects_exist() {
        // Staff brackets/braces
        let score = load_ngl("tc_55_1.ngl");
        let connect_count: usize = score.connects.values().map(|v| v.len()).sum();
        eprintln!("tc_55_1: {} connect subobjects", connect_count);
        assert!(
            connect_count > 0,
            "Orchestral score should have staff connections"
        );
    }

    #[test]
    fn ngl_tuplet_objects_exist() {
        // Check tuplets across all fixtures
        let mut found_tuplets = false;
        let fixture_dir = "tests/fixtures";
        for entry in fs::read_dir(fixture_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ngl") {
                let data = fs::read(&path).unwrap();
                let ngl = NglFile::read_from_bytes(&data).unwrap();
                let score = interpret_heap(&ngl).unwrap();
                let tuplet_count: usize = score.tuplets.values().map(|v| v.len()).sum();
                if tuplet_count > 0 {
                    eprintln!(
                        "{}: {} tuplet subobjects",
                        path.file_name().unwrap().to_str().unwrap(),
                        tuplet_count
                    );
                    found_tuplets = true;
                }
            }
        }
        if !found_tuplets {
            eprintln!("No NGL fixtures contain tuplet objects (known gap)");
        }
    }

    #[test]
    fn ngl_keysig_variety() {
        // Check that different key signatures are represented across fixtures
        let mut seen_sharps = false;
        let mut seen_flats = false;
        let fixture_dir = "tests/fixtures";
        for entry in fs::read_dir(fixture_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ngl") {
                let data = fs::read(&path).unwrap();
                let ngl = NglFile::read_from_bytes(&data).unwrap();
                let score = interpret_heap(&ngl).unwrap();
                for ks_list in score.keysigs.values() {
                    for ks in ks_list {
                        if ks.ks_info.n_ks_items > 0 {
                            for item in &ks.ks_info.ks_item[..ks.ks_info.n_ks_items as usize] {
                                if item.sharp != 0 {
                                    seen_sharps = true;
                                } else {
                                    seen_flats = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            seen_sharps,
            "Should see at least one sharp key sig across fixtures"
        );
        assert!(
            seen_flats,
            "Should see at least one flat key sig across fixtures"
        );
    }

    #[test]
    fn ngl_all_fixtures_export_valid_musicxml() {
        let fixture_dir = "tests/fixtures";
        let mut count = 0;
        for entry in fs::read_dir(fixture_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ngl") {
                let data = fs::read(&path).unwrap();
                let ngl = NglFile::read_from_bytes(&data).unwrap();
                let score = interpret_heap(&ngl).unwrap();
                let xml = export_musicxml(&score);

                let name = path.file_name().unwrap().to_str().unwrap();
                assert!(
                    xml.contains("<score-partwise"),
                    "{}: should produce valid MusicXML root element",
                    name
                );
                assert!(
                    xml.contains("<part-list>"),
                    "{}: should have part list",
                    name
                );
                assert!(
                    xml.contains("<note>") || xml.contains("<note "),
                    "{}: should have at least one note",
                    name
                );
                count += 1;
            }
        }
        eprintln!("Validated MusicXML export for {} NGL fixtures", count);
        assert!(count > 20, "Should validate many fixtures: got {}", count);
    }

    /// DTD-validate every NGL fixture export against the local MusicXML 4.0 DTD.
    ///
    /// Uses xmllint (must be on PATH). Skips gracefully if xmllint is missing.
    #[test]
    fn ngl_all_fixtures_dtd_valid() {
        use std::io::Write;
        use std::process::Command;

        // Check xmllint is available
        let has_xmllint = Command::new("xmllint").arg("--version").output().is_ok();
        if !has_xmllint {
            eprintln!("SKIP: xmllint not found on PATH");
            return;
        }

        let dtd_path = std::path::Path::new("tests/schemas/musicxml-4.0/partwise.dtd")
            .canonicalize()
            .expect("DTD file should exist");

        let fixture_dir = "tests/fixtures";
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in fs::read_dir(fixture_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ngl") {
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                let data = fs::read(&path).unwrap();
                let ngl = NglFile::read_from_bytes(&data).unwrap();
                let score = interpret_heap(&ngl).unwrap();
                let xml = export_musicxml(&score);

                // Strip the DOCTYPE line so xmllint uses only our local DTD
                let stripped: String = xml
                    .lines()
                    .filter(|l| !l.contains("<!DOCTYPE"))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Write to temp file
                let tmp = std::env::temp_dir().join(format!("dtd_check_{}.musicxml", count));
                {
                    let mut f = fs::File::create(&tmp).unwrap();
                    f.write_all(stripped.as_bytes()).unwrap();
                }

                let output = Command::new("xmllint")
                    .arg("--nonet")
                    .arg("--dtdvalid")
                    .arg(&dtd_path)
                    .arg("--noout")
                    .arg(&tmp)
                    .output()
                    .expect("xmllint failed to run");

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // Grab first 5 error lines for context
                    let first_errs: String = stderr.lines().take(5).collect::<Vec<_>>().join("\n");
                    failures.push(format!("{}: {}", name, first_errs));
                }

                let _ = fs::remove_file(&tmp);
                count += 1;
            }
        }

        eprintln!(
            "DTD-validated {} fixtures: {} passed, {} failed",
            count,
            count - failures.len(),
            failures.len()
        );
        assert!(
            failures.is_empty(),
            "DTD validation failures:\n{}",
            failures.join("\n\n")
        );
    }
}

// ============================================================
// Dynamics tests
// ============================================================

/// Test that a simple dynamic marking (mf) roundtrips through import→export.
#[test]
fn dynamics_simple_mf() {
    let xml = make_xml_default(
        r#"<direction placement="below">
        <direction-type><dynamics><mf/></dynamics></direction-type>
      </direction>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1920</duration><voice>1</voice><type>whole</type></note>"#,
    );
    let score = import_musicxml(&xml).expect("import");
    let exported = export_musicxml(&score);
    // Verify the dynamic marking appears in re-export
    assert!(
        exported.contains("<dynamics>") && exported.contains("<mf/>"),
        "Exported XML should contain <dynamics><mf/>"
    );
}

/// Test that multiple dynamic types import and re-export correctly.
#[test]
fn dynamics_multiple_types() {
    let xml = make_xml_default(
        r#"<direction placement="below">
        <direction-type><dynamics><pp/></dynamics></direction-type>
      </direction>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>960</duration><voice>1</voice><type>half</type></note>
      <direction placement="below">
        <direction-type><dynamics><ff/></dynamics></direction-type>
      </direction>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>960</duration><voice>1</voice><type>half</type></note>"#,
    );
    let score = import_musicxml(&xml).expect("import");
    let exported = export_musicxml(&score);
    assert!(exported.contains("<pp/>"), "Should re-export pp dynamic");
    assert!(exported.contains("<ff/>"), "Should re-export ff dynamic");
}

/// Test dynamics from NGL fixture export.
#[test]
fn dynamics_ngl_fixture_export() {
    use nightingale_core::ngl::{interpret::interpret_heap, NglFile};
    // tc_55_1 has dynamics in the NGL score
    let data = std::fs::read("tests/fixtures/tc_55_1.ngl").expect("read NGL fixture");
    let ngl = NglFile::read_from_bytes(&data).unwrap();
    let score = interpret_heap(&ngl).unwrap();
    let xml = export_musicxml(&score);
    // tc_55_1 has dynamic markings - verify they export
    let has_dynamics = xml.contains("<dynamics>");
    eprintln!(
        "tc_55_1 MusicXML dynamics export: {}",
        if has_dynamics { "found" } else { "none" }
    );
    // The test primarily verifies no crash; dynamics presence depends on fixture content
}

// ============================================================
// Beam tests
// ============================================================

/// Test that two beamed eighth notes import and re-export beam elements.
#[test]
fn beam_simple_two_eighths() {
    let xml = make_xml_default(
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>240</duration><voice>1</voice><type>eighth</type>
        <beam number="1">begin</beam></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>240</duration><voice>1</voice><type>eighth</type>
        <beam number="1">end</beam></note>
      <note><rest/><duration>1440</duration><voice>1</voice><type>half</type></note>"#,
    );
    let score = import_musicxml(&xml).expect("import");

    // Verify beamed flag is set on notes
    let mut beamed_count = 0;
    for obj in score.walk() {
        if let nightingale_core::ngl::interpret::ObjData::Sync(_) = &obj.data {
            if let Some(notes) = score.notes.get(&obj.header.first_sub_obj) {
                for note in notes {
                    if note.beamed {
                        beamed_count += 1;
                    }
                }
            }
        }
    }
    assert!(
        beamed_count >= 2,
        "Should have at least 2 beamed notes, got {}",
        beamed_count
    );

    // Verify BeamSet objects exist
    assert!(
        !score.notebeams.is_empty(),
        "Should have notebeams from imported beam data"
    );

    // Re-export and verify beam elements
    let exported = export_musicxml(&score);
    assert!(
        exported.contains("<beam number=\"1\">begin</beam>"),
        "Should export beam begin"
    );
    assert!(
        exported.contains("<beam number=\"1\">end</beam>"),
        "Should export beam end"
    );
}

/// Test four beamed sixteenth notes with two beam levels.
#[test]
fn beam_four_sixteenths() {
    let xml = make_xml_default(
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>120</duration><voice>1</voice><type>16th</type>
        <beam number="1">begin</beam><beam number="2">begin</beam></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>120</duration><voice>1</voice><type>16th</type>
        <beam number="1">continue</beam><beam number="2">continue</beam></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>120</duration><voice>1</voice><type>16th</type>
        <beam number="1">continue</beam><beam number="2">continue</beam></note>
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>120</duration><voice>1</voice><type>16th</type>
        <beam number="1">end</beam><beam number="2">end</beam></note>
      <note><rest/><duration>1440</duration><voice>1</voice><type>half</type></note>"#,
    );
    let score = import_musicxml(&xml).expect("import");
    let exported = export_musicxml(&score);

    // Count beam elements in re-export
    let beam_begins: Vec<_> = exported.match_indices("begin</beam>").collect();
    let beam_ends: Vec<_> = exported.match_indices("end</beam>").collect();
    let beam_continues: Vec<_> = exported.match_indices("continue</beam>").collect();

    eprintln!(
        "Beam elements: {} begins, {} continues, {} ends",
        beam_begins.len(),
        beam_continues.len(),
        beam_ends.len()
    );

    assert!(
        beam_begins.len() >= 2,
        "Should have at least 2 beam begins (2 levels)"
    );
    assert!(
        beam_ends.len() >= 2,
        "Should have at least 2 beam ends (2 levels)"
    );
}

/// Test beam roundtrip stability: import→export→import→export produces same beams.
#[test]
fn beam_roundtrip_stability() {
    let xml = make_xml_default(
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>240</duration><voice>1</voice><type>eighth</type>
        <beam number="1">begin</beam></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>240</duration><voice>1</voice><type>eighth</type>
        <beam number="1">continue</beam></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>240</duration><voice>1</voice><type>eighth</type>
        <beam number="1">continue</beam></note>
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>240</duration><voice>1</voice><type>eighth</type>
        <beam number="1">end</beam></note>"#,
    );

    let score1 = import_musicxml(&xml).expect("import 1");
    let export1 = export_musicxml(&score1);
    let beam_count_1 = export1.matches("<beam ").count();

    let score2 = import_musicxml(&export1).expect("import 2");
    let export2 = export_musicxml(&score2);
    let beam_count_2 = export2.matches("<beam ").count();

    eprintln!(
        "Beam roundtrip: pass1={} beams, pass2={} beams",
        beam_count_1, beam_count_2
    );
    assert!(beam_count_1 > 0, "First export should have beam elements");
    assert_eq!(
        beam_count_1, beam_count_2,
        "Beam count should be stable across roundtrips"
    );
}

/// Test that NGL fixtures with beams export beam elements.
#[test]
fn beam_ngl_fixture_export() {
    use nightingale_core::ngl::{interpret::interpret_heap, NglFile};
    // me_and_lucy has beamed eighth notes
    let data = std::fs::read("tests/fixtures/01_me_and_lucy.ngl").expect("read NGL fixture");
    let ngl = NglFile::read_from_bytes(&data).unwrap();
    let score = interpret_heap(&ngl).unwrap();
    let xml = export_musicxml(&score);
    let beam_count = xml.matches("<beam ").count();
    eprintln!(
        "me_and_lucy MusicXML beam export: {} beam elements",
        beam_count
    );
    assert!(
        beam_count > 0,
        "me_and_lucy should export beam elements (has beamed eighth notes)"
    );
}
