//! Integration test for NGL file reading
//!
//! Tests reading real N103/N105 format files and verifies:
//! - Version tag is correct
//! - All 25 heaps are present
//! - Key heaps have expected subobjects/objects
//! - String pool is non-empty

use nightingale_core::ngl::{NglFile, NglVersion};

#[test]
fn test_read_n105_file() {
    let path = "tests/fixtures/01_me_and_lucy_simple.ngl";

    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");

    // Verify version
    assert_eq!(ngl.version, NglVersion::N105, "Expected N105 format");

    // Verify all 25 heaps were read
    assert_eq!(ngl.heaps.len(), 25, "Expected 25 heaps (types 0-24)");

    // Verify string pool is non-empty
    assert!(
        !ngl.string_pool.is_empty(),
        "String pool should not be empty"
    );

    // Print heap summary for diagnostics
    println!("\n=== NGL File Info ===");
    println!("Version: {}", ngl.version.as_str());
    println!("Timestamp: {}", ngl.timestamp);
    println!("String pool size: {} bytes", ngl.string_pool.len());
    println!("\n=== Heap Counts ===");

    for (i, heap) in ngl.heaps.iter().enumerate() {
        if heap.obj_count > 0 {
            println!(
                "Heap {:2}: {:4} objects, obj_size={:3}, {:6} bytes",
                i,
                heap.obj_count,
                heap.obj_size,
                heap.obj_data.len()
            );
        }
    }

    // Verify key subobject heaps have data.
    // NOTE: Heaps 0-23 are SUBOBJECT heaps. Types like TAIL (1), PAGE (4),
    // SYSTEM (5) have zero subobjects — they exist only in the object heap (24).
    assert!(
        ngl.heaps[2].obj_count > 0,
        "SYNC heap should have subobjects (notes/rests)"
    );
    assert!(
        ngl.heaps[7].obj_count > 0,
        "MEASURE heap should have subobjects"
    );

    // Type 24 = OBJ (object heap), should have many objects
    assert!(ngl.heaps[24].obj_count > 0, "OBJ heap should have objects");
}

#[test]
fn test_read_n103_file() {
    // File 02 is N103 format
    let path = "tests/fixtures/02_cloning_frank_blacks.ngl";

    let ngl = NglFile::read_from_file(path).expect("Failed to read N103 file");

    assert_eq!(ngl.version, NglVersion::N103, "Expected N103 format");
    assert_eq!(ngl.heaps.len(), 25, "Expected 25 heaps");
    assert!(!ngl.string_pool.is_empty());
    assert!(ngl.heaps[24].obj_count > 0, "OBJ heap should have objects");

    println!("N103 file: {} objects in OBJ heap", ngl.heaps[24].obj_count);
}

#[test]
fn test_decode_strings() {
    use nightingale_core::ngl::decode_string;

    let path = "tests/fixtures/01_me_and_lucy_simple.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");

    // Offset 0 is the canonical empty string
    let empty = decode_string(&ngl.string_pool, 0);
    assert_eq!(
        empty,
        Some(String::new()),
        "Offset 0 should be empty string"
    );

    // Negative offsets return empty
    let invalid = decode_string(&ngl.string_pool, -1);
    assert_eq!(
        invalid,
        Some(String::new()),
        "Negative offset should return empty"
    );
}

#[test]
fn test_read_all_fixture_files() {
    // Read every .ngl file in fixtures — they are our test oracle
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

    assert!(!files.is_empty(), "Should have .ngl fixture files");

    let mut n103_count = 0;
    let mut n105_count = 0;

    for path in &files {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let ngl = NglFile::read_from_file(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));

        assert_eq!(ngl.heaps.len(), 25, "{}: expected 25 heaps", filename);
        assert!(
            !ngl.string_pool.is_empty(),
            "{}: string pool should not be empty",
            filename
        );
        assert!(
            ngl.heaps[24].obj_count > 0,
            "{}: OBJ heap should have objects",
            filename
        );

        match ngl.version {
            NglVersion::N103 => n103_count += 1,
            NglVersion::N105 => n105_count += 1,
        }

        let total_objects: u32 = ngl.heaps.iter().map(|h| h.obj_count as u32).sum();
        println!(
            "  {} [{}]: {} total objects, {} string pool bytes",
            filename,
            ngl.version.as_str(),
            total_objects,
            ngl.string_pool.len()
        );
    }

    println!(
        "\nRead {} files ({} N103, {} N105)",
        files.len(),
        n103_count,
        n105_count
    );
}
