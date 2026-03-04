//! Integration test for NGL file reading
//!
//! Tests reading real N103/N105 format files and verifies:
//! - Version tag is correct
//! - All 25 heaps are present
//! - Key heaps have expected subobjects/objects
//! - String pool is non-empty

use nightingale_core::ngl::{NglFile, NglVersion};

#[test]
fn test_read_n103_primary_file() {
    let path = "tests/fixtures/01_me_and_lucy.ngl";

    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");

    // All our current fixture files are N103 (written by Nightingale 5.x but tagged N103).
    // N105 test fixtures will be added when available.
    assert_eq!(ngl.version, NglVersion::N103, "Expected N103 format");

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

    let path = "tests/fixtures/01_me_and_lucy.ngl";
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
            NglVersion::N101 | NglVersion::N102 | NglVersion::N103 => n103_count += 1,
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
        "\nRead {} files ({} legacy, {} N105)",
        files.len(),
        n103_count,
        n105_count
    );
}

#[test]
fn test_interpret_heap_basic() {
    use nightingale_core::ngl::interpret_heap;

    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");

    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    // Should have objects (at least 2 HEADERs and 2 TAILs: score + master page lists)
    assert!(
        score.objects.len() >= 4,
        "Should have at least 2 HEADERs and 2 TAILs (score + master page)"
    );

    // First object should be HEADER (type 0) — the score object list head
    assert_eq!(
        score.objects[0].header.obj_type, 0,
        "First object should be HEADER"
    );

    // Should have notes, staves, measures, etc.
    println!("\n=== Interpreted Score Stats ===");
    println!("Total objects: {}", score.objects.len());
    println!("Notes: {} sync groups", score.notes.len());
    println!("Staffs: {} staff groups", score.staffs.len());
    println!("Measures: {} measure groups", score.measures.len());
    println!("Clefs: {} clef groups", score.clefs.len());
    println!("KeySigs: {} keysig groups", score.keysigs.len());
    println!("TimeSigs: {} timesig groups", score.timesigs.len());
    println!("NoteBeams: {} beam groups", score.notebeams.len());
    println!("Slurs: {} slur groups", score.slurs.len());
}

#[test]
fn test_object_walker() {
    use nightingale_core::ngl::interpret_heap;

    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    // Walk the score object list using the iterator (follows right links from HEADER)
    let mut walk_count = 0;
    let mut obj_types: std::collections::HashMap<i8, usize> = std::collections::HashMap::new();

    for obj in score.walk() {
        walk_count += 1;
        *obj_types.entry(obj.header.obj_type).or_insert(0) += 1;
    }

    println!("\n=== Object Walk Stats ===");
    println!("Total objects walked (score list): {}", walk_count);
    println!("Total objects in heap: {}", score.objects.len());
    println!("Object counts by type:");
    let mut types: Vec<_> = obj_types.iter().collect();
    types.sort_by_key(|(t, _)| *t);
    for (obj_type, count) in types {
        println!("  Type {:2}: {}", obj_type, count);
    }

    // Should have walked some objects (the score list, excluding master page list)
    assert!(walk_count > 0, "Should have walked at least one object");

    // Walk follows one linked list (score), so it may be less than total objects
    // (the master page list is a separate linked list: HEADER->PAGE->SYSTEM->STAFF->...->TAIL)
    assert!(
        walk_count <= score.objects.len(),
        "Walk should not exceed total objects"
    );

    // Score list should have at least 100 objects for a real score
    assert!(
        walk_count > 100,
        "Score list should have substantial objects, got {}",
        walk_count
    );
}

#[test]
fn test_count_objects_by_type() {
    use nightingale_core::defs::*;
    use nightingale_core::ngl::interpret_heap;

    let path = "tests/fixtures/01_me_and_lucy.ngl";
    let ngl = NglFile::read_from_file(path).expect("Failed to read NGL file");
    let score = interpret_heap(&ngl).expect("Failed to interpret heap");

    let mut counts: std::collections::HashMap<u8, usize> = std::collections::HashMap::new();

    for obj in &score.objects {
        *counts.entry(obj.header.obj_type as u8).or_insert(0) += 1;
    }

    // Must have exactly 2 HEADERs and 2 TAILs (score list + master page list)
    assert_eq!(
        *counts.get(&HEADER_TYPE).unwrap_or(&0),
        2,
        "Should have exactly 2 HEADERs (score + master page)"
    );
    assert_eq!(
        *counts.get(&TAIL_TYPE).unwrap_or(&0),
        2,
        "Should have exactly 2 TAILs (score + master page)"
    );

    // Should have at least one PAGE and one SYSTEM
    assert!(
        *counts.get(&PAGE_TYPE).unwrap_or(&0) > 0,
        "Should have at least 1 PAGE"
    );
    assert!(
        *counts.get(&SYSTEM_TYPE).unwrap_or(&0) > 0,
        "Should have at least 1 SYSTEM"
    );

    // Should have at least one SYNC (notes)
    assert!(
        *counts.get(&SYNC_TYPE).unwrap_or(&0) > 0,
        "Should have at least 1 SYNC"
    );

    println!("\n=== Object Type Counts ===");
    println!("HEADER: {}", counts.get(&HEADER_TYPE).unwrap_or(&0));
    println!("TAIL: {}", counts.get(&TAIL_TYPE).unwrap_or(&0));
    println!("SYNC: {}", counts.get(&SYNC_TYPE).unwrap_or(&0));
    println!("PAGE: {}", counts.get(&PAGE_TYPE).unwrap_or(&0));
    println!("SYSTEM: {}", counts.get(&SYSTEM_TYPE).unwrap_or(&0));
    println!("STAFF: {}", counts.get(&STAFF_TYPE).unwrap_or(&0));
    println!("MEASURE: {}", counts.get(&MEASURE_TYPE).unwrap_or(&0));
    println!("CLEF: {}", counts.get(&CLEF_TYPE).unwrap_or(&0));
    println!("KEYSIG: {}", counts.get(&KEYSIG_TYPE).unwrap_or(&0));
    println!("TIMESIG: {}", counts.get(&TIMESIG_TYPE).unwrap_or(&0));
}
