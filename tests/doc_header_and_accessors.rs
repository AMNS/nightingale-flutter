//! Integration tests for document header parsing and score accessors.
//!
//! Tests verify:
//! - Document header parsing from real .ngl files
//! - Score accessor methods (head, tail, num_staves, etc.)
//! - String pool integration
//! - Object list navigation

use nightingale_core::defs::{HEADER_TYPE, MEASURE_TYPE, SYNC_TYPE, TAIL_TYPE};
use nightingale_core::ngl::{interpret_heap, DocumentHeader, InterpretedScore, NglFile};

#[test]
fn test_parse_document_header_from_real_file() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");

    // Parse document header (uses binrw)
    let doc_hdr =
        DocumentHeader::from_n105_bytes(&ngl.doc_header_raw).expect("Failed to parse doc header");

    // Verify page dimensions are reasonable (letter size is 612x792 points)
    let page_w = doc_hdr.orig_paper_rect.right - doc_hdr.orig_paper_rect.left;
    let page_h = doc_hdr.orig_paper_rect.bottom - doc_hdr.orig_paper_rect.top;
    assert!(
        page_w > 0 && page_w < 2000,
        "Page width should be reasonable: {}",
        page_w
    );
    assert!(
        page_h > 0 && page_h < 2000,
        "Page height should be reasonable: {}",
        page_h
    );

    println!("\n=== Document Header ===");
    println!("Page: {}x{} points", page_w, page_h);
    println!("Sheets: {}", doc_hdr.num_sheets);
    println!("Origin: ({}, {})", doc_hdr.origin.v, doc_hdr.origin.h);
}

#[test]
fn test_head_and_tail_accessors() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    // Test head()
    let head = score.head().expect("Should have a head object");
    assert_eq!(
        head.header.obj_type, HEADER_TYPE as i8,
        "Head should be HEADER type"
    );
    println!("\n=== Head Object ===");
    println!("Index: {}", head.index);
    println!("Type: {}", head.header.obj_type);
    println!("Right link: {}", head.header.right);

    // Test tail()
    let tail = score.tail().expect("Should have a tail object");
    assert_eq!(
        tail.header.obj_type, TAIL_TYPE as i8,
        "Tail should be TAIL type"
    );
    assert_eq!(
        tail.header.right, 0,
        "Tail's right link should be NILINK (0)"
    );
    println!("\n=== Tail Object ===");
    println!("Index: {}", tail.index);
    println!("Type: {}", tail.header.obj_type);
    println!("Right link: {}", tail.header.right);
}

#[test]
fn test_num_staves_all_fixtures() {
    // Test num_staves() on all fixture files
    let fixture_dir = std::path::Path::new("tests/fixtures");
    let mut files: Vec<_> = std::fs::read_dir(fixture_dir)
        .expect("Failed to read fixtures directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ngl") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();

    println!("\n=== Staves Per File ===");
    for path in &files {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let ngl = NglFile::read_from_file(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));
        let score = interpret_heap(&ngl)
            .unwrap_or_else(|e| panic!("Failed to interpret {}: {}", filename, e));

        let num_staves = score.num_staves();
        assert!(
            num_staves > 0,
            "{}: Should have at least one staff",
            filename
        );
        assert!(
            num_staves <= 64,
            "{}: Should have <= 64 staves (got {})",
            filename,
            num_staves
        );
        println!("  {}: {} staves", filename, num_staves);
    }
}

#[test]
fn test_count_by_type() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    // Count various object types
    let header_count = score.count_by_type(HEADER_TYPE);
    let tail_count = score.count_by_type(TAIL_TYPE);
    let sync_count = score.count_by_type(SYNC_TYPE);
    let measure_count = score.count_by_type(MEASURE_TYPE);

    println!("\n=== Object Counts by Type ===");
    println!("HEADERs: {}", header_count);
    println!("TAILs: {}", tail_count);
    println!("SYNCs: {}", sync_count);
    println!("MEASUREs: {}", measure_count);

    // Sanity checks
    assert_eq!(
        header_count, tail_count,
        "Should have equal HEADERs and TAILs"
    );
    assert!(
        header_count >= 2,
        "Should have at least 2 HEADERs (score + master page)"
    );
    assert!(sync_count > 0, "Should have at least one SYNC");
    assert!(measure_count > 0, "Should have at least one MEASURE");
}

#[test]
fn test_syncs_accessor() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    let syncs = score.syncs();
    assert!(!syncs.is_empty(), "Should have SYNCs");

    // Verify all returned objects are SYNCs
    for sync in &syncs {
        assert_eq!(
            sync.header.obj_type, SYNC_TYPE as i8,
            "All objects should be SYNCs"
        );
    }

    println!("\n=== SYNC Objects ===");
    println!("Total SYNCs: {}", syncs.len());
    println!("First SYNC index: {}", syncs[0].index);
    println!("First SYNC has {} notes", syncs[0].header.n_entries);
}

#[test]
fn test_measure_objects_accessor() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    let measures = score.measure_objects();
    assert!(!measures.is_empty(), "Should have MEASUREs");

    // Verify all returned objects are MEASUREs
    for measure in &measures {
        assert_eq!(
            measure.header.obj_type, MEASURE_TYPE as i8,
            "All objects should be MEASUREs"
        );
    }

    println!("\n=== MEASURE Objects ===");
    println!("Total MEASUREs: {}", measures.len());
    println!("First MEASURE index: {}", measures[0].index);
    println!(
        "First MEASURE has {} submeasures",
        measures[0].header.n_entries
    );
}

#[test]
fn test_score_list_navigation() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    let score_list = score.score_list();
    assert!(!score_list.is_empty(), "Score list should not be empty");

    // First should be HEADER
    assert_eq!(
        score_list[0].header.obj_type, HEADER_TYPE as i8,
        "First object should be HEADER"
    );

    // Last should be TAIL
    let last_idx = score_list.len() - 1;
    assert_eq!(
        score_list[last_idx].header.obj_type, TAIL_TYPE as i8,
        "Last object should be TAIL"
    );

    // Verify linked list integrity (each right points to next, or NILINK at end)
    for i in 0..score_list.len() - 1 {
        let current = score_list[i];
        let next = score_list[i + 1];
        assert_eq!(
            current.header.right, next.index,
            "Object {} right link should point to next object {}",
            current.index, next.index
        );
    }

    println!("\n=== Score List Navigation ===");
    println!("Score list length: {}", score_list.len());
    println!("First object: {:?}", score_list[0].header.obj_type);
    println!("Last object: {:?}", score_list[last_idx].header.obj_type);
}

#[test]
fn test_master_page_list() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    let master_list = score.master_page_list();

    if !master_list.is_empty() {
        // If there's a master page list, it should start with HEADER
        assert_eq!(
            master_list[0].header.obj_type, HEADER_TYPE as i8,
            "Master page list should start with HEADER"
        );

        // And end with TAIL
        let last_idx = master_list.len() - 1;
        assert_eq!(
            master_list[last_idx].header.obj_type, TAIL_TYPE as i8,
            "Master page list should end with TAIL"
        );

        println!("\n=== Master Page List ===");
        println!("Master page list length: {}", master_list.len());
    } else {
        println!("\n=== Master Page List ===");
        println!("No master page list found");
    }
}

#[test]
fn test_decode_string_integration() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");

    // Test with offset 0 (canonical empty string)
    let empty = InterpretedScore::decode_string(&ngl.string_pool, 0);
    assert_eq!(empty, Some(String::new()), "Offset 0 should be empty");

    // Test with negative offset
    let invalid = InterpretedScore::decode_string(&ngl.string_pool, -1);
    assert_eq!(
        invalid,
        Some(String::new()),
        "Negative offset should return empty"
    );

    println!("\n=== String Pool Info ===");
    println!("String pool size: {} bytes", ngl.string_pool.len());
}

#[test]
fn test_document_header_all_fixtures() {
    // Parse document header from all fixture files
    let fixture_dir = std::path::Path::new("tests/fixtures");
    let mut files: Vec<_> = std::fs::read_dir(fixture_dir)
        .expect("Failed to read fixtures directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ngl") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();

    println!("\n=== Document Headers ===");
    for path in &files {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let ngl = NglFile::read_from_file(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));

        let doc_hdr = DocumentHeader::from_n105_bytes(&ngl.doc_header_raw)
            .unwrap_or_else(|e| panic!("Failed to parse doc header in {}: {}", filename, e));

        let page_w = doc_hdr.orig_paper_rect.right - doc_hdr.orig_paper_rect.left;
        let page_h = doc_hdr.orig_paper_rect.bottom - doc_hdr.orig_paper_rect.top;

        // Basic sanity checks
        assert!(page_w > 0, "{}: page width should be positive", filename);
        assert!(page_h > 0, "{}: page height should be positive", filename);

        println!("  {}: {}x{} pts", filename, page_w, page_h);
    }
}
