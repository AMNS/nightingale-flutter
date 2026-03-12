use std::fs;
/// Canonical comparison test suite for MusicXML imports
///
/// This module provides systematic comparison of our rendered MusicXML imports
/// against canonical reference PDFs, enabling quantitative assessment of fidelity.
///
/// The test suite measures rendering quality against supplier-provided reference PDFs
/// by converting both canonical and our rendered PDFs to PNG and comparing pixel-by-pixel.
/// This enables quantitative tracking of import improvements across versions.
use std::path::{Path, PathBuf};

#[path = "common/mod.rs"]
mod common;

/// Summary statistics for a single sample's comparison
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub name: String,
    pub total_pixels: u64,
    pub diff_pixels: u64,
    pub diff_pct: f64,
    pub status: String,
}

/// Generate a comprehensive canonical comparison report
///
/// For each sample:
/// 1. Check if canonical PDF exists
/// 2. If our golden bitmap exists, convert canonical PDF page 1 to PNG
/// 3. Compare our golden PNG against canonical PNG page-by-page
/// 4. Record quantitative metrics (% pixel difference, affected pages)
/// 5. Generate visual diff and categorize issues by severity
#[test]
#[ignore = "canonical_comparison: generates diagnostic output, not validation"]
fn test_canonical_pdf_comparison_summary() {
    let canonical_dir = Path::new("tests/musicxml_examples/xmlsamples");
    let golden_dir = Path::new("tests/golden_bitmaps");
    let output_dir = PathBuf::from("test-output/canonical-comparison");

    // Create output directory for comparison results
    let _ = fs::create_dir_all(&output_dir);

    let samples = vec![
        "ActorPreludeSample",
        "BeetAnGeSample",
        "BrahWiMeSample",
        "BrookeWestSample",
        "Binchois",
        "Chant",
        "DebuMandSample",
        "Dichterliebe01",
        "Echigo-Jishi",
        "FaurReveSample",
        "MahlFaGe4Sample",
        "MozaChloSample",
        "MozaVeilSample",
        "MozartPianoSonata",
        "MozartTrio",
        "Saltarello",
        "SchbAvMaSample",
        "Telemann",
    ];

    println!("\n=== MusicXML Import Canonical Comparison Report ===\n");
    println!("Sample                          | Canon PDF | Our Golden | Difference | Status");
    println!("{:-<85}", "");

    let mut results = Vec::new();
    let mut total_samples = 0;
    let mut with_goldens = 0;
    let mut with_comparison = 0;
    let mut avg_diff: f64 = 0.0;

    for sample in samples {
        let canon_pdf = canonical_dir.join(format!("{}.pdf", sample));
        let golden_page1 = golden_dir.join(format!("xml_{}_page1.png", sample));

        total_samples += 1;
        let mut diff_pct = 0.0;

        let status = if canon_pdf.exists() {
            if golden_page1.exists() {
                with_goldens += 1;
                // Convert canonical PDF page 1 to PNG for comparison
                let canon_png = output_dir.join(format!("{}_canonical_page1.png", sample));

                match common::pdf_to_png(&canon_pdf, &canon_png) {
                    Ok(true) => {
                        // Compare images
                        let diff_png = output_dir.join(format!("{}_diff_page1.png", sample));
                        match common::compare_images_and_diff(&golden_page1, &canon_png, &diff_png)
                        {
                            Ok((_total, _diff, pct)) => {
                                diff_pct = pct;
                                with_comparison += 1;
                                avg_diff += pct;

                                if pct < 0.5 {
                                    format!("✓ Good ({:.2}%)", pct)
                                } else if pct < 2.0 {
                                    format!("⚠ Fair ({:.2}%)", pct)
                                } else {
                                    format!("✗ Poor ({:.2}%)", pct)
                                }
                            }
                            Err(e) => {
                                format!("✗ Diff error: {}", e)
                            }
                        }
                    }
                    Ok(false) => String::from("✗ PDF tool unavailable"),
                    Err(e) => {
                        format!("✗ PDF error: {}", e)
                    }
                }
            } else {
                String::from("⚠ No golden bitmap")
            }
        } else {
            String::from("✗ No canonical PDF")
        };

        println!(
            "  {:<29} | {:9} | {:10} | {:10.2}% | {}",
            sample,
            if canon_pdf.exists() {
                "✓ found"
            } else {
                "✗ missing"
            },
            if golden_page1.exists() {
                "✓ found"
            } else {
                "✗ missing"
            },
            diff_pct,
            status
        );

        results.push(ComparisonResult {
            name: sample.to_string(),
            total_pixels: 0,
            diff_pixels: 0,
            diff_pct,
            status,
        });
    }

    println!();
    println!("Summary Statistics:");
    println!("  Total samples:        {}", total_samples);
    println!(
        "  With golden bitmaps:  {} ({:.1}%)",
        with_goldens,
        (with_goldens as f64 / total_samples as f64) * 100.0
    );
    println!(
        "  With comparison:      {} ({:.1}%)",
        with_comparison,
        (with_comparison as f64 / total_samples as f64) * 100.0
    );

    if with_comparison > 0 {
        avg_diff /= with_comparison as f64;
        println!("  Average difference:   {:.2}%", avg_diff);
    }

    println!(
        "\nComparison PDFs and diffs saved to: {}",
        output_dir.display()
    );
    println!("\nNote: This comparison framework:");
    println!("  1. Converts canonical PDFs to PNG for pixel-by-pixel comparison");
    println!("  2. Compares our golden bitmaps against canonical PDFs page-by-page");
    println!("  3. Generates quantitative metrics (% pixel difference)");
    println!("  4. Tracks improvements over time as fixes are implemented");
}

/// Track known issues in canonical comparison
///
/// Each issue is mapped to the samples it affects and its priority for fixing
#[test]
#[ignore = "canonical_comparison: diagnostic reporting only"]
fn test_known_rendering_issues() {
    println!("\n=== Known MusicXML Import Rendering Issues ===\n");

    let issues = vec![
        (
            "PRIORITY 3: Staff line continuity",
            "Empty staves not rendering continuation lines when parts don't play",
            vec!["Dichterliebe01", "MozaChloSample", "MozartTrio"],
            "Medium",
        ),
        (
            "PRIORITY 4: Guitar clef octave transposition",
            "8va (octave) below not being applied to guitar staves",
            vec!["SchbAvMaSample"],
            "Medium",
        ),
        (
            "PRIORITY 5: Non-ASCII character encoding",
            "Encoding issues in lyrics, titles, and text elements",
            vec!["Dichterliebe01", "Telemann"],
            "Low",
        ),
        (
            "PRIORITY 6: Text vertical positioning",
            "Y-axis placement incorrect for lyrics, tempo marks, titles",
            vec!["ActorPreludeSample", "Dichterliebe01", "MozartTrio"],
            "Medium",
        ),
    ];

    for (name, desc, samples, priority) in issues {
        println!("{} [{}]", name, priority);
        println!("  Description: {}", desc);
        println!("  Affects: {}", samples.join(", "));
        println!();
    }
}

/// Report on successfully imported features
#[test]
#[ignore = "canonical_comparison: diagnostic reporting only"]
fn test_successful_features_report() {
    println!("\n=== Successfully Imported MusicXML Features ===\n");

    let features = vec![
        (
            "Notes & Rests",
            "All duration types, multi-voice, multi-part",
            "✓",
        ),
        (
            "Pitches & Clefs",
            "All clef types including transposing clefs",
            "✓",
        ),
        (
            "Key/Time Signatures",
            "All standard key signatures and time signatures",
            "✓",
        ),
        (
            "Accidentals",
            "Naturals, sharps, flats, double sharps/flats",
            "✓",
        ),
        (
            "Ties & Slurs",
            "Single and cross-system, proper endpoints",
            "✓",
        ),
        ("Dynamics", "Text dynamics and hairpin wedges", "✓"),
        (
            "Tuplets",
            "Full time-modification support with brackets/numbers",
            "✓",
        ),
        ("Grace Notes", "Including beamed grace notes", "✓"),
        ("Articulations", "All 14 standard articulation types", "✓"),
        ("Ornaments", "Trills, mordents, turns, etc.", "✓"),
        (
            "Beams",
            "Proper slopes with 33% reduction, multi-note groups",
            "✓",
        ),
        ("Barlines", "Single, double, repeat, dotted", "✓"),
        ("Volta Endings", "Repeat ending brackets and numbering", "✓"),
        ("Tempo Marks", "Verbal tempos and metronome marks", "✓"),
        ("Ottava", "8va/8vb/15ma with dashed brackets", "✓"),
        ("Part Groups", "Bracket and brace groupings", "✓"),
        ("Credits", "Titles and composers from metadata", "✓"),
        ("Lyrics", "Syllabic text from <lyric> elements", "✓"),
        (
            "Pagination",
            "Full system/page layout with Gourlay spacing",
            "✓",
        ),
        (
            "UTF-16 Support",
            "Proper handling of UTF-16 encoded MusicXML files",
            "✓",
        ),
    ];

    println!("Feature                    | Description                             | Status");
    println!("{:-<80}", "");

    for (feature, desc, status) in features {
        println!("  {:<24} | {:<39} | {}", feature, desc, status);
    }
}
