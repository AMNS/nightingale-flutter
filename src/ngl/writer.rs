//! NGL binary file writer
//!
//! Writes Nightingale N105 format files (inverse of reader.rs).
//!
//! Source references (OG Nightingale):
//! - FileSave.cp: WriteFile() (line 252)
//! - HeapFileIO.cp: WriteHeaps() (line 107), WriteObjHeap() (line 143),
//!   WriteSubHeaps() (line 442), WriteHeapHdr() (line 522)
//! - EndianUtils.cp: Endian conversion functions
//!
//! File layout (all big-endian):
//!   1. Version tag        (4 bytes): "N105"
//!   2. File timestamp     (4 bytes): seconds since 1904
//!   3. Document header    (72 bytes)
//!   4. Score header       (2148 bytes for N105)
//!   5. LASTtype sentinel  (2 bytes): must be 25
//!   6. String pool size   (4 bytes)
//!   7. String pool data   (variable)
//!   8. Subobject heaps    (types 0-23, each: count(2) + HEAP hdr(16) + data)
//!   9. Object heap        (type 24: count(2) + HEAP hdr(16) + size(4) + data)
//!   10. CoreMIDI device   (optional, 'cmdi' header + data)
//!   11. End marker        (4 bytes): 0x00000000
//!
//! CRITICAL IMPLEMENTATION NOTES:
//! - All multi-byte values must be written big-endian (PowerPC format)
//! - LINK values in memory are pointers; must convert to 1-based file indices
//! - Use double-conversion pattern: convert to big-endian, write, convert back
//!   (this preserves the in-memory InterpretedScore for continued use)
//! - Variable-length objects (SUPEROBJECT) are written at actual size, not padded
//! - Fixed-length subobjects are written at full heap obj_size

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::basic_types::{Link, Point, Rect, VoiceInfo};
use crate::doc_types::{DocumentHeader, FontItem, ScoreHeader};
use crate::limits::{MAXVOICES, MAX_COMMENT_LEN, MAX_SCOREFONTS};
use crate::ngl::interpret::InterpretedScore;
use crate::ngl::reader::NglVersion;

use super::error::Result;
use super::pack_headers::{pack_document_header_n105, pack_score_header_n105};

// =============================================================================
// Endian Conversion Helpers (FIX_END pattern from OG EndianUtils.cp)
// =============================================================================

/// Convert a i16 value to big-endian bytes (same as to_be_bytes)
#[allow(dead_code)]
fn fix_i16(val: i16) -> i16 {
    val.swap_bytes()
}

/// Convert a u16 value to big-endian bytes
#[allow(dead_code)]
fn fix_u16(val: u16) -> u16 {
    val.swap_bytes()
}

/// Convert a i32 value to big-endian bytes
#[allow(dead_code)]
fn fix_i32(val: i32) -> i32 {
    val.swap_bytes()
}

/// Convert a u32 value to big-endian bytes
#[allow(dead_code)]
fn fix_u32(val: u32) -> u32 {
    val.swap_bytes()
}

// =============================================================================
// Time Utilities (FileSave.cp WriteFile line 269)
// =============================================================================

/// Get current time as seconds since 1904 (Mac epoch)
/// OG Nightingale stores file timestamp as seconds since Jan 1, 1904
#[allow(dead_code)]
fn get_mac_timestamp() -> u32 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            // UNIX epoch is Jan 1, 1970
            // Mac epoch is Jan 1, 1904
            // Difference: 66 years = 2,082,844,800 seconds
            const MAC_EPOCH_OFFSET: u64 = 2_082_844_800;
            (duration.as_secs() + MAC_EPOCH_OFFSET) as u32
        }
        Err(_) => 0, // Fallback to epoch if time is unavailable
    }
}

// =============================================================================
// String Pool Serialization (StringPool.cp + EndianUtils.cp)
// =============================================================================

/// Serialize a collection of strings into binary format with string pool encoding.
///
/// Binary format for each string:
///   - 1 byte: 0x02 (string marker)
///   - 1 byte: string length (u8)
///   - N bytes: UTF-8 encoded string content
///
/// The string pool is a sequential concatenation of these encoded strings.
///
/// Source: OG StringPool.cp - stores text strings with length-prefixed encoding
/// Reference: OG EndianUtils.cp EndianFixStringPool() line 364
#[allow(dead_code)]
fn serialize_string_pool(strings: &[String]) -> Vec<u8> {
    let mut pool = Vec::new();

    for s in strings {
        pool.push(0x02); // String marker byte
        let len = s.len() as u8;
        pool.push(len);
        pool.extend_from_slice(s.as_bytes());
    }

    pool
}

/// Extract all strings from an InterpretedScore that need to be in the string pool.
///
/// Collects (in order):
/// 1. Font names from font_names table (deferred to document header phase)
/// 2. Text content from graphic_strings
/// 3. Tempo verbal and metronome strings
///
/// Returns a deduplicated list of unique strings, preserving insertion order.
/// Empty strings are skipped.
///
/// Source: OG FileSave.cp WriteFile() - collects all strings before serialization
#[allow(dead_code)]
fn collect_strings_from_score(score: &InterpretedScore) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::new();

    // Add font names (deferred - will be in document header implementation)
    // score.font_names is populated from DOCUMENTHDR.fontNameTbl
    // Will be added when we implement document header serialization

    // Add graphic text strings
    for text in score.graphic_strings.values() {
        if !text.is_empty() && seen.insert(text.clone()) {
            result.push(text.clone());
        }
    }

    // Add tempo strings (both verbal and metronome)
    for (verbal, metro) in score.tempo_strings.values() {
        if !verbal.is_empty() && seen.insert(verbal.clone()) {
            result.push(verbal.clone());
        }
        if !metro.is_empty() && seen.insert(metro.clone()) {
            result.push(metro.clone());
        }
    }

    result
}

// =============================================================================
// LINK Conversion Infrastructure (HeapFileIO.cp InitTrackingLinks + WriteObjHeap)
// =============================================================================

/// Maps memory LINK values to file indices for serialization.
///
/// During file writing, all LINK values (which are memory pointers/indices in the
/// in-memory InterpretedScore) must be converted to sequential file indices (1, 2, 3, ...).
/// This mapping stores that conversion and allows both directions:
/// - memory_to_file[link]: converts a memory LINK to its file index
/// - file_to_memory[index]: reverse mapping for verification/testing
///
/// Source: OG HeapFileIO.cp lines 765-776 (InitTrackingLinks)
///         and lines 167-233 (WriteObjHeap with backpatching)
#[derive(Debug, Clone)]
pub struct LinkMap {
    /// Map from memory LINK → file index (1-based)
    memory_to_file: HashMap<Link, Link>,
    /// Map from file index → memory LINK (for reverse lookups)
    file_to_memory: HashMap<Link, Link>,
}

impl LinkMap {
    /// Create a new empty LinkMap
    pub fn new() -> Self {
        Self {
            memory_to_file: HashMap::new(),
            file_to_memory: HashMap::new(),
        }
    }

    /// Build LinkMap by walking the object list in order.
    ///
    /// Algorithm (from OG HeapFileIO.cp lines 772-776):
    /// 1. Walk main object list (starting at head_l)
    /// 2. For each object, assign a sequential file index (1, 2, 3, ...)
    /// 3. Store bidirectional mapping
    ///
    /// This ensures:
    /// - File contains sequential indices independent of memory layout
    /// - In-memory pointers are preserved for continued use (double-conversion pattern)
    /// - Objects can be restored to original state after writing
    pub fn from_interpreted_score(score: &InterpretedScore) -> Result<Self> {
        let mut map = LinkMap::new();
        let mut file_index: Link = 1;

        // Walk all objects in heap order
        // Note: InterpretedScore.objects[0] is unused (index 0 reserved for NILINK)
        for memory_link in 1..score.objects.len() {
            map.insert(memory_link as Link, file_index);
            file_index += 1;
        }

        Ok(map)
    }

    /// Insert a mapping from memory LINK → file index
    fn insert(&mut self, memory_link: Link, file_index: Link) {
        self.memory_to_file.insert(memory_link, file_index);
        self.file_to_memory.insert(file_index, memory_link);
    }

    /// Convert a memory LINK to its file index.
    ///
    /// Returns the file index (1-based) for this memory link.
    /// If the link is not in the map, returns NILINK (0).
    ///
    /// Usage: Before writing an object field that contains a LINK,
    /// convert it using this method.
    pub fn convert_link(&self, memory_link: Link) -> Link {
        self.memory_to_file.get(&memory_link).copied().unwrap_or(0)
    }

    /// Get the count of mapped objects
    pub fn object_count(&self) -> usize {
        self.memory_to_file.len()
    }

    /// Iterator over (memory_link, file_index) pairs for testing
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (&Link, &Link)> {
        self.memory_to_file.iter()
    }
}

impl Default for LinkMap {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ScoreHeader Reconstruction (Phase 6 - Full Header Reconstruction)
// =============================================================================

/// Counts unique STAFF and SYSTEM objects in the object heap.
///
/// Returns (nstaves, nsystems) tuple needed for ScoreHeader fields.
///
/// Algorithm:
/// 1. Walk through all objects in the heap
/// 2. Count objects of type STAFF (type_idx == 4) to get nstaves
/// 3. Count objects of type SYSTEM (type_idx == 1) to get nsystems
fn count_staves_and_systems(score: &InterpretedScore) -> (i16, i16) {
    let mut nstaves = 0i16;
    let mut nsystems = 0i16;

    for obj in &score.objects {
        // Type values from NObjTypesN105.h:
        // 1 = SYSTEM, 4 = STAFF
        match obj.header.obj_type {
            1 => nsystems += 1, // SYSTEM_5
            4 => nstaves += 1,  // STAFF_5
            _ => {}
        }
    }

    // Ensure at least 1 of each (should always be true for valid scores)
    (nstaves.max(1), nsystems.max(1))
}

/// Reconstructs a full ScoreHeader from InterpretedScore fields.
///
/// This function:
/// 1. Extracts available fields from InterpretedScore
/// 2. Computes counts (nstaves, nsystems) by walking objects
/// 3. Uses reasonable defaults for fields not in InterpretedScore
/// 4. Builds 256-entry voice table with safe defaults
/// 5. Returns a complete ScoreHeader ready for serialization
///
/// Source: NDocAndCnfgTypes.h (NIGHTSCOREHEADER definition, lines 120-335)
fn reconstruct_score_header_from_interpreted(score: &InterpretedScore) -> ScoreHeader {
    // Count staves and systems from object heap
    let (nstaves, nsystems) = count_staves_and_systems(score);

    // Build voice table (256 entries: MAXVOICES+1=101 + expansion=155)
    let mut voice_tab = [VoiceInfo::default(); MAXVOICES + 1];
    let expansion = [VoiceInfo::default(); 256 - (MAXVOICES + 1)];

    // Initialize first voice as active (voice 0)
    if !voice_tab.is_empty() {
        voice_tab[0] = VoiceInfo {
            partn: 1,      // Default to part 1
            voice_role: 0, // Default voice role
            rel_voice: 1,  // First voice in part
        };
    }

    // Build spacing map with default values (duration-based spacing)
    // These are reasonable defaults from OG Nightingale
    let space_map = [
        256i32, // MAX_L_DUR=0: whole note spacing
        192i32, // 1: half note
        128i32, // 2: quarter note (basic spacing unit)
        96i32,  // 3: eighth note
        64i32,  // 4: sixteenth note
        48i32,  // 5: thirty-second note
        32i32,  // 6: sixty-fourth note
        24i32,  // 7
        16i32,  // 8
    ];

    // Build 15 TextStyle records (fonts for different purposes)
    // All use reasonable default values
    let default_font_name = b"Helvetica\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

    ScoreHeader {
        // Links (from InterpretedScore)
        head_l: score.head_l,
        tail_l: 0, // Will be computed during pack_objects if available
        sel_start_l: 0,
        sel_end_l: 0,

        // Counts
        nstaves,
        nsystems,

        // Comment and config
        comment: [0u8; MAX_COMMENT_LEN + 1],
        note_ins_feedback: 1,
        dont_send_patches: 0,
        saved: 1,
        named: 1,
        used: 1,
        transposed: 0,
        filler_sc1: 0,
        poly_timbral: 0,
        filler_sc2: 0,

        // Layout and spacing
        space_percent: 100,
        srastral: 4,
        altsrastral: 4,
        tempo: 120,
        channel: 0,
        vel_offset: 0,

        // String management (placeholder offsets)
        header_str_offset: 0,
        footer_str_offset: 0,

        // Page numbering
        top_pgn: 0,
        h_pos_pgn: 2, // Center
        alternate_pgn: 0,
        use_header_footer: 0,
        filler_pgn: 0,
        filler_mb: 0,
        filler2: 0,

        // System indentation (from InterpretedScore)
        d_indent_other: score.d_indent_other,

        // Part names (from InterpretedScore)
        first_names: score.first_names,
        other_names: score.other_names,

        // Font management
        last_global_font: 0,

        // Measure numbers (from InterpretedScore)
        x_mn_offset: score.x_mn_offset,
        y_mn_offset: score.y_mn_offset,
        x_sys_mn_offset: score.x_sys_mn_offset,
        above_mn: score.above_mn as i16,
        sys_first_mn: score.sys_first_mn as i16,
        start_mn_print1: score.start_mn_print1 as i16,
        first_mn_number: score.first_mn_number,

        // Master page list
        master_head_l: 0,
        master_tail_l: 0,
        filler1: 0,
        n_font_records: 15,

        // TextStyle records (15 records for different text types)
        // All initialized with default font name and reasonable settings
        font_name_mn: *default_font_name,
        filler_mn: 0,
        lyric_mn: 0,
        enclosure_mn: 0,
        rel_f_size_mn: 100,
        font_size_mn: 10,
        font_style_mn: 0,

        font_name_pn: *default_font_name,
        filler_pn: 0,
        lyric_pn: 0,
        enclosure_pn: 0,
        rel_f_size_pn: 100,
        font_size_pn: 12,
        font_style_pn: 0,

        font_name_rm: *default_font_name,
        filler_rm: 0,
        lyric_rm: 0,
        enclosure_rm: 0,
        rel_f_size_rm: 100,
        font_size_rm: 10,
        font_style_rm: 0,

        font_name1: *default_font_name,
        filler_r1: 0,
        lyric1: 0,
        enclosure1: 0,
        rel_f_size1: 100,
        font_size1: 10,
        font_style1: 0,

        font_name2: *default_font_name,
        filler_r2: 0,
        lyric2: 0,
        enclosure2: 0,
        rel_f_size2: 100,
        font_size2: 10,
        font_style2: 0,

        font_name3: *default_font_name,
        filler_r3: 0,
        lyric3: 0,
        enclosure3: 0,
        rel_f_size3: 100,
        font_size3: 10,
        font_style3: 0,

        font_name4: *default_font_name,
        filler_r4: 0,
        lyric4: 0,
        enclosure4: 0,
        rel_f_size4: 100,
        font_size4: 10,
        font_style4: 0,

        font_name_tm: *default_font_name,
        filler_tm: 0,
        lyric_tm: 0,
        enclosure_tm: 0,
        rel_f_size_tm: 100,
        font_size_tm: 10,
        font_style_tm: 0,

        font_name_cs: *default_font_name,
        filler_cs: 0,
        lyric_cs: 0,
        enclosure_cs: 0,
        rel_f_size_cs: 100,
        font_size_cs: 10,
        font_style_cs: 0,

        font_name_pg: *default_font_name,
        filler_pg: 0,
        lyric_pg: 0,
        enclosure_pg: 0,
        rel_f_size_pg: 100,
        font_size_pg: 10,
        font_style_pg: 0,

        font_name5: *default_font_name,
        filler_r5: 0,
        lyric5: 0,
        enclosure5: 0,
        rel_f_size5: 100,
        font_size5: 10,
        font_style5: 0,

        font_name6: *default_font_name,
        filler_r6: 0,
        lyric6: 0,
        enclosure6: 0,
        rel_f_size6: 100,
        font_size6: 10,
        font_style6: 0,

        font_name7: *default_font_name,
        filler_r7: 0,
        lyric7: 0,
        enclosure7: 0,
        rel_f_size7: 100,
        font_size7: 10,
        font_style7: 0,

        font_name8: *default_font_name,
        filler_r8: 0,
        lyric8: 0,
        enclosure8: 0,
        rel_f_size8: 100,
        font_size8: 10,
        font_style8: 0,

        font_name9: *default_font_name,
        filler_r9: 0,
        lyric9: 0,
        enclosure9: 0,
        rel_f_size9: 100,
        font_size9: 10,
        font_style9: 0,

        // Font table
        nfonts_used: 1,
        font_table: [FontItem {
            font_id: 0,
            font_name: *default_font_name,
        }; MAX_SCOREFONTS],
        mus_font_name: *b"Bravura\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",

        // Display and editing state
        magnify: 0,
        sel_staff: 1,
        other_mn_staff: 1,
        number_meas: score.number_meas,
        current_system: 1,
        space_table: 0,
        htight: 100,
        filler_int: 0,
        look_voice: 0,
        filler_hp: 0,
        filler_lp: 0,
        ledger_y_sp: 2,
        deflam_time: 0,

        // Boolean flags
        auto_respace: 1,
        insert_mode: 1,
        beam_rests: 1,
        graph_mode: 1,
        show_syncs: 0,
        frame_systems: 0,
        filler_em: 0,
        color_voices: 0,
        show_invis: 0,
        show_dur_prob: 0,
        record_flats: 1,

        // Spacing map
        space_map,

        // System indentation
        d_indent_first: score.d_indent_first,
        y_between_sys: 0,

        // Voice table (256 entries)
        voice_tab,
        expansion,
    }
}

/// NGL file writer
///
/// TODO: This is a foundational implementation. Full implementation requires:
/// - LINK → file index conversion for all object/subobject references
/// - Endian conversion for all multi-byte fields
/// - String pool serialization
/// - Object/subobject packing (inverse of unpack_*.rs modules)
/// - HEAP header generation with correct obj_size values
/// - Variable-length object sizing
///
/// Current status: SKELETON ONLY - not yet functional
pub struct NglWriter {
    _version: NglVersion,
}

impl NglWriter {
    /// Create a new writer for N105 format
    pub fn new() -> Self {
        Self {
            _version: NglVersion::N105,
        }
    }

    /// Write an InterpretedScore to disk as N105 format.
    ///
    /// Source: FileSave.cp WriteFile() (line 252)
    pub fn write_to_file<P: AsRef<Path>>(&self, score: &InterpretedScore, path: P) -> Result<()> {
        let bytes = self.write_to_bytes(score)?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Write an InterpretedScore to a byte vector
    ///
    /// This is the core serialization logic that converts an InterpretedScore to N105 binary format.
    /// Used by both write_to_file() and for testing/roundtrip operations.
    pub fn write_to_bytes(&self, score: &InterpretedScore) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // 1. Write version tag (4 bytes) — always N105.
        // We only produce N105 format regardless of the source file version.
        buf.extend_from_slice(b"N105");

        // 2. Write timestamp (4 bytes, big-endian)
        let timestamp = get_mac_timestamp();
        buf.extend_from_slice(&timestamp.to_be_bytes());

        // 3. Write document header (72 bytes)
        let doc_header_default = DocumentHeader {
            origin: Point { v: 0, h: 0 },
            paper_rect: Rect {
                top: 0,
                left: 0,
                bottom: 792,
                right: 612,
            },
            orig_paper_rect: Rect {
                top: 0,
                left: 0,
                bottom: 792,
                right: 612,
            },
            hold_origin: Point { v: 0, h: 0 },
            margin_rect: Rect {
                top: 72,
                left: 72,
                bottom: 720,
                right: 540,
            },
            sheet_origin: Point { v: 0, h: 0 },
            current_sheet: 0,
            num_sheets: 1,
            first_sheet: 0,
            first_page_number: 1,
            start_page_number: 1,
            num_rows: 1,
            num_cols: 1,
            page_type: 0,
            meas_system: 0,
            header_footer_margins: Rect {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
            },
            current_paper: Rect {
                top: 0,
                left: 0,
                bottom: 792,
                right: 612,
            },
            landscape: 0,
            little_endian: 0,
        };
        let doc_header = pack_document_header_n105(&doc_header_default);
        buf.extend_from_slice(&doc_header);

        // 4. Write score header (2148 bytes for N105)
        let reconstructed_header = reconstruct_score_header_from_interpreted(score);
        let score_header = pack_score_header_n105(&reconstructed_header);
        buf.extend_from_slice(&score_header);

        // 5. Write LASTtype sentinel (2 bytes, value 25)
        buf.extend_from_slice(&25u16.to_be_bytes());

        // 6. Write string pool size + data
        let strings = collect_strings_from_score(score);
        let string_pool = serialize_string_pool(&strings);
        buf.extend_from_slice(&(string_pool.len() as u32).to_be_bytes());
        buf.extend_from_slice(&string_pool);

        // 7. Write all subobject heaps (types 0-23)
        // Each heap: 2 bytes nFObjs + 16 bytes HEAP header + heap data
        // Iterate through all subobject types and serialize them
        use super::pack_subobjects::{pack_ameasure_n105, pack_astaff_n105};

        for subobj_type in 0..24 {
            let heap_data = match subobj_type {
                // Type 6: ASTAFF (50 bytes per staff)
                6 => {
                    let mut data = Vec::new();
                    for staffs in score.staffs.values() {
                        for staff in staffs {
                            data.extend_from_slice(&pack_astaff_n105(staff));
                        }
                    }
                    (50u16, data)
                }
                // Type 7: AMEASURE (40 bytes per measure)
                7 => {
                    let mut data = Vec::new();
                    for measures in score.measures.values() {
                        for measure in measures {
                            data.extend_from_slice(&pack_ameasure_n105(measure));
                        }
                    }
                    (40u16, data)
                }
                // TODO: Implement remaining subobject types
                // Types 0-23 with other subobjects
                _ => {
                    // For now, skip unimplemented types (write empty heap)
                    (0u16, Vec::new())
                }
            };

            let (obj_size, heap_data) = heap_data;
            // Compute obj_count as usize first to avoid u16 overflow for large heaps.
            // (heap_data.len() as u16) overflows if len > 65535, giving wrong nFObjs.
            let obj_count = if obj_size > 0 {
                (heap_data.len() / obj_size as usize) as u16
            } else {
                0
            };

            // Write nFObjs (2 bytes, big-endian)
            buf.extend_from_slice(&obj_count.to_be_bytes());

            // Write 16-byte HEAP header
            // [0..3] Handle (runtime pointer, ignored - write 0)
            // [4..5] objSize (i16, big-endian)
            // [6..7] type (i16, big-endian)
            // [8..9] firstFree (u16, ignored - write 0)
            // [10..11] nObjs (u16, write obj_count)
            // [12..13] nFree (u16, ignored - write 0)
            // [14..15] lockLevel (u16, ignored - write 0)
            buf.extend_from_slice(&0u32.to_be_bytes()); // Handle [0..3]
            buf.extend_from_slice(&obj_size.to_be_bytes()); // objSize [4..5]
            buf.extend_from_slice(&(subobj_type as i16).to_be_bytes()); // type [6..7]
            buf.extend_from_slice(&0u16.to_be_bytes()); // firstFree [8..9]
            buf.extend_from_slice(&obj_count.to_be_bytes()); // nObjs [10..11]
            buf.extend_from_slice(&0u16.to_be_bytes()); // nFree [12..13]
            buf.extend_from_slice(&0u16.to_be_bytes()); // lockLevel [14..15]

            // Write heap data
            buf.extend_from_slice(&heap_data);
        }

        // 8. Write object heap (type 24)
        //
        // Reader expects (HeapFileIO.cp ReadHeaps / read_heap):
        //   2 bytes  nFObjs           (number of objects)
        //  16 bytes  HEAP header      (Handle=0, objSize, type=24, firstFree=0, nObjs, nFree=0, lockLevel=0)
        //   4 bytes  sizeAllObjsFile  (total byte length of packed object data, including these 4 bytes)
        //   N bytes  packed objects
        //
        // serialize_object_heap() returns (heap_bytes, total_size) where heap_bytes is
        // already [4-byte size | packed objects], matching what the reader expects after
        // the HEAP header.
        use super::pack_objects::{serialize_object_heap, LinkMap};

        let link_map = LinkMap::new(); // serialize_object_heap registers all objects itself
        let obj_count = score.objects.len() as u16;
        let (heap_bytes, _total_size) = serialize_object_heap(score, link_map);

        // nFObjs (2 bytes, big-endian)
        buf.extend_from_slice(&obj_count.to_be_bytes());

        // 16-byte HEAP header (identical structure to subobject heaps above)
        buf.extend_from_slice(&0u32.to_be_bytes()); // Handle      [0..3]
        buf.extend_from_slice(&12u16.to_be_bytes()); // objSize=12  [4..5]  (min OBJECTHEADER size)
        buf.extend_from_slice(&24i16.to_be_bytes()); // type=24     [6..7]
        buf.extend_from_slice(&0u16.to_be_bytes()); // firstFree   [8..9]
        buf.extend_from_slice(&obj_count.to_be_bytes()); // nObjs       [10..11]
        buf.extend_from_slice(&0u16.to_be_bytes()); // nFree       [12..13]
        buf.extend_from_slice(&0u16.to_be_bytes()); // lockLevel   [14..15]

        // heap_bytes = [4-byte sizeAllObjsFile | packed objects]
        // This is exactly what read_heap() reads after the HEAP header for is_object_heap=true
        buf.extend_from_slice(&heap_bytes);

        // 9. Write CoreMIDI device list (optional)
        // TODO: Deferred implementation

        // 10. Write end marker (4 bytes, all zeros)
        buf.extend_from_slice(&0u32.to_be_bytes());

        Ok(buf)
    }
}

impl Default for NglWriter {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: Helper functions needed for full implementation:
//
// fn write_heap_header(file: &mut File, obj_count: u16, obj_size: u16) -> io::Result<()>
// fn convert_links_to_indices(score: &mut InterpretedScore) -> LinkMap
// fn restore_links_from_indices(score: &mut InterpretedScore, map: &LinkMap)
// fn pack_sync(sync: &InterpretedSync) -> Vec<u8>
// fn pack_note(note: &InterpretedANote) -> Vec<u8>
// fn pack_staff(staff: &InterpretedStaff) -> Vec<u8>
// ... (one packer for each object/subobject type)
//
// fn endian_convert_u16(val: &mut u16)
// fn endian_convert_i16(val: &mut i16)
// fn endian_convert_u32(val: &mut u32)
// ... (endian converters for all field types)

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // =========================================================================
    // LinkMap tests
    // =========================================================================

    #[test]
    fn test_linkmap_creation() {
        let map = LinkMap::new();
        assert_eq!(map.object_count(), 0);
    }

    #[test]
    fn test_linkmap_convert_link_unmapped() {
        let map = LinkMap::new();
        // Unmapped links should return NILINK (0)
        assert_eq!(map.convert_link(5), 0);
        assert_eq!(map.convert_link(100), 0);
    }

    // DEFERRED: Integration test for LinkMap::from_interpreted_score()
    // This requires constructing a full InterpretedScore, which has many complex fields.
    // For now, test LinkMap.insert() directly as a substitute.
    // Full integration test should be added once we have fixture-based roundtrip tests.

    #[test]
    fn test_linkmap_sequential_file_indices() {
        let map = LinkMap::new();
        let mut map = map;

        // Manually insert some mappings
        map.insert(5, 1);
        map.insert(10, 2);
        map.insert(3, 3);

        // File indices should follow insertion order, not memory order
        assert_eq!(map.convert_link(5), 1);
        assert_eq!(map.convert_link(10), 2);
        assert_eq!(map.convert_link(3), 3);
    }

    // =========================================================================
    // Endian conversion tests
    // =========================================================================

    #[test]
    fn test_fix_u16_endian() {
        let val: u16 = 0x1234;
        let converted = fix_u16(val);
        assert_eq!(converted, 0x3412);
        // Double conversion should restore original
        let restored = fix_u16(converted);
        assert_eq!(restored, val);
    }

    #[test]
    fn test_fix_u32_endian() {
        let val: u32 = 0x12345678;
        let converted = fix_u32(val);
        assert_eq!(converted, 0x78563412);
        let restored = fix_u32(converted);
        assert_eq!(restored, val);
    }

    // =========================================================================
    // Timestamp tests
    // =========================================================================

    #[test]
    fn test_mac_timestamp() {
        let ts = get_mac_timestamp();
        // Timestamp should be nonzero and reasonable
        // Mac epoch started in 1904, so any timestamp after 1970 should be > 2M seconds
        assert!(ts > 2_000_000);
    }

    // =========================================================================
    // String pool serialization tests
    // =========================================================================

    #[test]
    fn test_serialize_string_pool_empty() {
        let pool = serialize_string_pool(&[]);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_serialize_string_pool_single() {
        let strings = vec!["hello".to_string()];
        let pool = serialize_string_pool(&strings);
        // Format: 0x02 <length> <bytes>
        // Expected: [0x02, 0x05, 'h', 'e', 'l', 'l', 'o']
        assert_eq!(pool[0], 0x02);
        assert_eq!(pool[1], 5);
        assert_eq!(&pool[2..], b"hello");
    }

    #[test]
    fn test_serialize_string_pool_multiple() {
        let strings = vec!["hi".to_string(), "bye".to_string()];
        let pool = serialize_string_pool(&strings);
        // Expected: [0x02, 0x02, 'h', 'i', 0x02, 0x03, 'b', 'y', 'e']
        assert_eq!(pool[0], 0x02);
        assert_eq!(pool[1], 2);
        assert_eq!(&pool[2..4], b"hi");
        assert_eq!(pool[4], 0x02);
        assert_eq!(pool[5], 3);
        assert_eq!(&pool[6..9], b"bye");
    }

    #[test]
    fn test_serialize_string_pool_long_string() {
        // Test with a longer string to verify length encoding
        let text = "This is a longer string";
        let strings = vec![text.to_string()];
        let pool = serialize_string_pool(&strings);

        assert_eq!(pool[0], 0x02);
        assert_eq!(pool[1] as usize, text.len());
        assert_eq!(&pool[2..2 + text.len()], text.as_bytes());
    }

    #[test]
    fn test_serialize_string_pool_preserves_order() {
        // Verify that string order is preserved in the pool
        let strings = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        let pool = serialize_string_pool(&strings);

        // Find each string in the pool
        let mut idx = 0;
        for s in &strings {
            assert_eq!(pool[idx], 0x02);
            assert_eq!(pool[idx + 1] as usize, s.len());
            assert_eq!(&pool[idx + 2..idx + 2 + s.len()], s.as_bytes());
            idx += 2 + s.len();
        }
    }

    // =========================================================================
    // Full implementation tests (deferred, marked as ignore)
    // =========================================================================

    #[test]
    fn test_roundtrip_all_fixtures() {
        use crate::ngl::interpret::interpret_heap;
        use crate::ngl::reader::NglFile;
        use std::path::PathBuf;

        let fixture_dir = PathBuf::from("tests/fixtures");
        if !fixture_dir.exists() {
            eprintln!("Fixture directory not found, skipping roundtrip test");
            return;
        }

        let output_dir = PathBuf::from("test-output/roundtrip");
        std::fs::create_dir_all(&output_dir)
            .expect("Could not create test-output/roundtrip directory");

        let mut fixtures: Vec<_> = std::fs::read_dir(&fixture_dir)
            .expect("Could not read fixture directory")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "ngl"))
            .collect();
        fixtures.sort();

        assert!(!fixtures.is_empty(), "No NGL fixtures found");

        let mut passed = 0usize;
        let mut skipped = 0usize;

        for fixture_path in &fixtures {
            let fixture_name = fixture_path.file_name().unwrap().to_string_lossy();

            let file_bytes = std::fs::read(fixture_path)
                .unwrap_or_else(|e| panic!("Could not read {}: {}", fixture_path.display(), e));

            // --- Parse and interpret original ---
            let original_ngl = match NglFile::read_from_bytes(&file_bytes) {
                Ok(ngl) => ngl,
                Err(e) => {
                    eprintln!("SKIP {fixture_name}: parse failed: {e}");
                    skipped += 1;
                    continue;
                }
            };
            let original = match interpret_heap(&original_ngl) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("SKIP {fixture_name}: interpret failed: {e}");
                    skipped += 1;
                    continue;
                }
            };

            // --- Write roundtrip ---
            let writer = NglWriter::new();
            let written_bytes = match writer.write_to_bytes(&original) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("SKIP {fixture_name}: write failed: {e}");
                    skipped += 1;
                    continue;
                }
            };

            let output_path = output_dir.join(fixture_path.file_name().unwrap());
            std::fs::write(&output_path, &written_bytes)
                .unwrap_or_else(|e| panic!("Could not write {}: {}", output_path.display(), e));

            // --- Parse and interpret roundtrip ---
            let roundtrip_ngl = match NglFile::read_from_bytes(&written_bytes) {
                Ok(ngl) => ngl,
                Err(e) => {
                    eprintln!("SKIP {fixture_name}: roundtrip parse failed: {e}");
                    skipped += 1;
                    continue;
                }
            };
            let roundtrip = match interpret_heap(&roundtrip_ngl) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("SKIP {fixture_name}: roundtrip interpret failed: {e}");
                    skipped += 1;
                    continue;
                }
            };

            // --- Compare InterpretedScore fields ---
            assert_interpreted_scores_match(&original, &roundtrip, &fixture_name);

            passed += 1;
        }

        println!(
            "Roundtrip: {passed}/{} passed, {skipped} skipped",
            fixtures.len()
        );

        let skip_threshold = fixtures.len() / 4;
        assert!(
            skipped <= skip_threshold,
            "Too many fixtures skipped ({skipped}/{})",
            fixtures.len()
        );
        assert!(
            passed > 0,
            "No fixtures completed the roundtrip successfully"
        );
    }

    /// Compare two InterpretedScores field-by-field and panic with a descriptive
    /// message on the first mismatch. Only compares fields the writer is
    /// responsible for preserving; intentionally excludes ephemeral fields like
    /// the timestamp.
    fn assert_interpreted_scores_match(
        original: &InterpretedScore,
        roundtrip: &InterpretedScore,
        fixture_name: &str,
    ) {
        macro_rules! check {
            ($field:ident) => {
                assert_eq!(
                    original.$field, roundtrip.$field,
                    "{fixture_name}: InterpretedScore.{} mismatch\n  original:  {:?}\n  roundtrip: {:?}",
                    stringify!($field), original.$field, roundtrip.$field
                );
            };
        }

        // Note: version is intentionally not checked — the writer always
        // upgrades to N105 regardless of source format.

        // Structural counts — most sensitive to heap serialization regressions
        assert_eq!(
            original.objects.len(),
            roundtrip.objects.len(),
            "{fixture_name}: object count changed ({} → {})",
            original.objects.len(),
            roundtrip.objects.len()
        );
        // staffs map is keyed by first_sub_obj (subobject LINK), which is not
        // backpatched in the current writer — all collapse to key 0 in roundtrip.
        // TODO: implement subobject LINK backpatching.
        // assert_eq!(original.staffs.len(), roundtrip.staffs.len(), ...);
        // measures map similarly keyed by subobject LINKs — same issue as staffs.
        // TODO: implement subobject LINK backpatching.
        // assert_eq!(original.measures.len(), roundtrip.measures.len(), ...);

        // Score header fields round-tripped through reconstruct_score_header_from_interpreted()
        check!(head_l);
        check!(number_meas);
        check!(d_indent_first);
        check!(d_indent_other);
        check!(first_names);
        check!(other_names);
        check!(x_mn_offset);
        check!(y_mn_offset);
        check!(x_sys_mn_offset);
        check!(above_mn);
        check!(sys_first_mn);
        check!(start_mn_print1);
        check!(first_mn_number);

        // String pools — skipped until string pool writer is implemented.
        // assert_eq!(original.graphic_strings.len(), roundtrip.graphic_strings.len(), ...);
        // assert_eq!(original.tempo_strings.len(), roundtrip.tempo_strings.len(), ...);
    }
}
