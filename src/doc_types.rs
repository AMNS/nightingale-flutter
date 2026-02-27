//! Document and Score header structures for Nightingale .ngl files.
//!
//! Ported from:
//! - `Nightingale/src/Precomps/NDocAndCnfgTypes.h` (DOCUMENTHDR, NIGHTSCOREHEADER)
//! - `Nightingale/src/Precomps/NObjTypesN105.h` (N105 bitfield layouts)
//! - `Nightingale/doc/Notes/Ngale5ProgQuickRef-TN1.txt` (byte offsets)
//!
//! ## File Structure (N105 Format)
//!
//! Every N105 Nightingale file begins:
//! ```text
//! Offset  Length  Item
//! ------  ------  ----
//! 0       4       "N105" magic bytes
//! 4       4       Date/time file was written (u32)
//! 8       72      Document header (DOCUMENTHDR)
//! 80      2148    Score header (NIGHTSCOREHEADER)
//! 2228    2       LASTtype (always 25)
//! 2230    4       String pool length
//! 2234    var     String pool
//! ...     var     Heaps (object data)
//! ```
//!
//! ## N105 Bitfield Handling
//!
//! The C++ code used PowerPC MSB-first bitfields for TextStyle and other fields.
//! In Rust, we store unpacked values and provide functions to decode/encode them.

use binrw::{BinRead, BinWrite};

use crate::basic_types::{Ddist, Link, Point, Rect, StringOffset, VoiceInfo};
use crate::limits::{MAXVOICES, MAX_COMMENT_LEN, MAX_SCOREFONTS};

// ============================================================================
// Constants (from tech note and source files)
// ============================================================================

/// Size of N105 document header in bytes.
///
/// Source: `Ngale5ProgQuickRef-TN1.txt:97`
pub const DOC_HDR_SIZE: usize = 72;

/// Size of N105 score header in bytes.
///
/// Source: `Ngale5ProgQuickRef-TN1.txt:98`
pub const SCORE_HDR_SIZE_N105: usize = 2148;

/// Offset of document header in N105 files.
///
/// Source: `Ngale5ProgQuickRef-TN1.txt:97`
pub const DOC_HDR_OFFSET: usize = 8;

/// Offset of score header in N105 files.
///
/// Source: `Ngale5ProgQuickRef-TN1.txt:98`
pub const SCORE_HDR_OFFSET: usize = 80;

/// Number of TEXTSTYLE records in score header.
///
/// Source: `NDocAndCnfgTypes.h:167`
pub const N_FONT_RECORDS: usize = 15;

/// Size of each TEXTSTYLE record in N105 format.
///
/// TextStyle has: fontName[32] + filler(2) + 4 u16 fields + 1 i16 = 44 bytes.
/// But in N105, it's stored with bitfields that pack differently.
/// The actual on-disk size is 36 bytes per record.
///
/// Source: `Ngale5ProgQuickRef-TN1.txt:120` (540 bytes / 15 records = 36 bytes each)
pub const TEXTSTYLE_SIZE_N105: usize = 36;

/// Size of FONTITEM record.
///
/// FONTITEM is: fontID (2 bytes) + fontName (32 bytes Pascal string) = 34 bytes.
///
/// Source: `NDocAndCnfgTypes.h:115-117`
pub const FONTITEM_SIZE: usize = 34;

/// Maximum duration code for spacing map.
///
/// Source: `NLimits.h:24` (MAX_L_DUR)
pub const MAX_L_DUR: usize = 9;

// ============================================================================
// DOCUMENTHEADER (72 bytes at offset 8)
// ============================================================================

/// Document header: generic fields for any document program.
///
/// This is a fixed 72-byte structure at offset 8 in N105 files.
///
/// Source: `NDocAndCnfgTypes.h:86-111`
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct DocumentHeader {
    /// Current origin of document window
    pub origin: Point,

    /// Size of virtual paper sheet in current magnification (pixels)
    pub paper_rect: Rect,

    /// Size of paper sheet (points)
    pub orig_paper_rect: Rect,

    /// (unused) Holds position during pick mode
    pub hold_origin: Point,

    /// Size of area within margins on sheet (points)
    pub margin_rect: Rect,

    /// Where in QuickDraw space to place sheet array
    pub sheet_origin: Point,

    /// Internal sheet [0, ..., numSheets)
    pub current_sheet: i16,

    /// Number of sheets in document (visible or not)
    pub num_sheets: i16,

    /// To be shown in upper left of sheet array
    pub first_sheet: i16,

    /// Page number of zero'th sheet
    pub first_page_number: i16,

    /// First printed page number
    pub start_page_number: i16,

    /// Size of sheet array (rows)
    pub num_rows: i16,

    /// Size of sheet array (columns)
    pub num_cols: i16,

    /// Current standard/custom page size from popup menu
    pub page_type: i16,

    /// Code for measurement system (from popup menu)
    pub meas_system: i16,

    /// Header/footer/pagenum margins
    pub header_footer_margins: Rect,

    /// Paper rect in window coords for current sheet (pixels)
    pub current_paper: Rect,

    /// Unused; was "Paper wider than it is high"
    pub landscape: i8,

    /// nonzero = created on a little Endian CPU
    pub little_endian: u8,
}

// ============================================================================
// FONTITEM (34 bytes)
// ============================================================================

/// Font item: maps internal font ID to system font name.
///
/// Source: `NDocAndCnfgTypes.h:115-117`
#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct FontItem {
    /// Font ID number for TextFont
    pub font_id: i16,

    /// Font name: Pascal string (length byte + up to 31 chars)
    pub font_name: [u8; 32],
}

// ============================================================================
// NIGHTSCOREHEADER (2148 bytes at offset 80)
// ============================================================================

/// Score header: all Nightingale-specific document information.
///
/// This is a complex 2148-byte structure at offset 80 in N105 files.
/// It contains links, metadata, 15 TextStyle records, font table, spacing map,
/// and voice information.
///
/// NOTE: This struct is manually parsed due to:
/// 1. Variable-size font table (nfontsUsed * FONTITEM_SIZE)
/// 2. N105 bitfield packing in TextStyle records
/// 3. Complex layout with multiple sub-arrays
///
/// Source: `NDocAndCnfgTypes.h:120-335`, `Ngale5ProgQuickRef-TN1.txt:112-124`
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreHeader {
    // ========================================================================
    // Links and basic metadata (12 bytes at offset 80)
    // ========================================================================
    /// Link to header object
    pub head_l: Link,

    /// Link to tail object
    pub tail_l: Link,

    /// Currently selected range start
    pub sel_start_l: Link,

    /// Currently selected range end
    pub sel_end_l: Link,

    /// Number of staves in a system
    pub nstaves: i16,

    /// Number of systems in score
    pub nsystems: i16,

    // ========================================================================
    // Comment and config fields (42 bytes at offset 92)
    // ========================================================================
    /// User comment on score (C string, 256 bytes)
    pub comment: [u8; MAX_COMMENT_LEN + 1],

    /// True for audio feedback on note insert
    pub note_ins_feedback: u8,

    /// 0 = when playing, send patch changes for channels
    pub dont_send_patches: u8,

    /// True if score has been saved
    pub saved: u8,

    /// True if file has been named
    pub named: u8,

    /// True if score contains any nonstructural info
    pub used: u8,

    /// True if transposed score, else C score
    pub transposed: u8,

    /// (no longer used) True if last text entered was lyric
    pub filler_sc1: u8,

    /// True for one part per MIDI channel
    pub poly_timbral: u8,

    /// (no longer used)
    pub filler_sc2: u8,

    /// Percentage of normal horizontal spacing used
    pub space_percent: i16,

    /// Standard staff size rastral no.
    pub srastral: i16,

    /// (not yet used but ready) Alternate staff size rastral no.
    pub altsrastral: i16,

    /// (unused?) playback speed in beats per minute
    pub tempo: i16,

    /// Basic MIDI channel number
    pub channel: i16,

    /// Global MIDI playback velocity offset
    pub vel_offset: i16,

    /// Index returned by String Manager
    pub header_str_offset: StringOffset,

    /// Index returned by String Manager
    pub footer_str_offset: StringOffset,

    /// True=page numbers at top of page, else bottom
    pub top_pgn: u8,

    /// 1=page numbers at left, 2=center, 3=at right
    pub h_pos_pgn: u8,

    /// True=page numbers alternately left and right
    pub alternate_pgn: u8,

    /// True=use header/footer text, not simple pagenum
    pub use_header_footer: u8,

    /// unused
    pub filler_pgn: u8,

    /// unused
    pub filler_mb: i8,

    /// unused
    pub filler2: Ddist,

    /// Amount to indent Systems other than first
    pub d_indent_other: Ddist,

    /// Code for drawing part names on first system
    pub first_names: i8,

    /// Code for drawing part names on other systems
    pub other_names: i8,

    /// Header index of most recent text style used
    pub last_global_font: i8,

    /// Horiz. pos. offset for meas. nos. (half-spaces)
    pub x_mn_offset: i8,

    /// Vert. pos. offset for meas. nos. (half-spaces)
    pub y_mn_offset: i8,

    /// Horiz. pos. offset for meas.nos. if 1st meas. in system
    pub x_sys_mn_offset: i8,

    /// True=measure numbers above staff, else below
    pub above_mn: i16,

    /// True=indent 1st meas. of system by xMNOffset
    pub sys_first_mn: i16,

    /// True=First meas. number to print is 1, else 2
    pub start_mn_print1: i16,

    /// Number of first measure
    pub first_mn_number: i16,

    /// Head of Master Page object list
    pub master_head_l: Link,

    /// Tail of Master Page object list
    pub master_tail_l: Link,

    /// unused
    pub filler1: i8,

    /// Always 15 for now
    pub n_font_records: i8,

    // ========================================================================
    // 15 TEXTSTYLE records (540 bytes in N105, 15 * 36 each)
    // ========================================================================
    /// MEASURE NO. FONT: default name, size and style
    pub font_name_mn: [u8; 32],
    pub filler_mn: u16,
    pub lyric_mn: u16,
    pub enclosure_mn: u16,
    pub rel_f_size_mn: u16,
    pub font_size_mn: u16,
    pub font_style_mn: i16,

    /// PART NAME FONT: default name, size and style
    pub font_name_pn: [u8; 32],
    pub filler_pn: u16,
    pub lyric_pn: u16,
    pub enclosure_pn: u16,
    pub rel_f_size_pn: u16,
    pub font_size_pn: u16,
    pub font_style_pn: i16,

    /// REHEARSAL MARK FONT: default name, size and style
    pub font_name_rm: [u8; 32],
    pub filler_rm: u16,
    pub lyric_rm: u16,
    pub enclosure_rm: u16,
    pub rel_f_size_rm: u16,
    pub font_size_rm: u16,
    pub font_style_rm: i16,

    /// REGULAR FONT 1: default name, size and style
    pub font_name1: [u8; 32],
    pub filler_r1: u16,
    pub lyric1: u16,
    pub enclosure1: u16,
    pub rel_f_size1: u16,
    pub font_size1: u16,
    pub font_style1: i16,

    /// REGULAR FONT 2: default name, size and style
    pub font_name2: [u8; 32],
    pub filler_r2: u16,
    pub lyric2: u16,
    pub enclosure2: u16,
    pub rel_f_size2: u16,
    pub font_size2: u16,
    pub font_style2: i16,

    /// REGULAR FONT 3: default name, size and style
    pub font_name3: [u8; 32],
    pub filler_r3: u16,
    pub lyric3: u16,
    pub enclosure3: u16,
    pub rel_f_size3: u16,
    pub font_size3: u16,
    pub font_style3: i16,

    /// REGULAR FONT 4: default name, size and style
    pub font_name4: [u8; 32],
    pub filler_r4: u16,
    pub lyric4: u16,
    pub enclosure4: u16,
    pub rel_f_size4: u16,
    pub font_size4: u16,
    pub font_style4: i16,

    /// TEMPO MARK FONT: default name, size and style
    pub font_name_tm: [u8; 32],
    pub filler_tm: u16,
    pub lyric_tm: u16,
    pub enclosure_tm: u16,
    pub rel_f_size_tm: u16,
    pub font_size_tm: u16,
    pub font_style_tm: i16,

    /// CHORD SYMBOL FONT: default name, size and style
    pub font_name_cs: [u8; 32],
    pub filler_cs: u16,
    pub lyric_cs: u16,
    pub enclosure_cs: u16,
    pub rel_f_size_cs: u16,
    pub font_size_cs: u16,
    pub font_style_cs: i16,

    /// PAGE HEADER/FOOTER/NO. FONT: default name, size and style
    pub font_name_pg: [u8; 32],
    pub filler_pg: u16,
    pub lyric_pg: u16,
    pub enclosure_pg: u16,
    pub rel_f_size_pg: u16,
    pub font_size_pg: u16,
    pub font_style_pg: i16,

    /// REGULAR FONT 5: default name, size and style
    pub font_name5: [u8; 32],
    pub filler_r5: u16,
    pub lyric5: u16,
    pub enclosure5: u16,
    pub rel_f_size5: u16,
    pub font_size5: u16,
    pub font_style5: i16,

    /// REGULAR FONT 6: default name, size and style
    pub font_name6: [u8; 32],
    pub filler_r6: u16,
    pub lyric6: u16,
    pub enclosure6: u16,
    pub rel_f_size6: u16,
    pub font_size6: u16,
    pub font_style6: i16,

    /// REGULAR FONT 7: default name, size and style
    pub font_name7: [u8; 32],
    pub filler_r7: u16,
    pub lyric7: u16,
    pub enclosure7: u16,
    pub rel_f_size7: u16,
    pub font_size7: u16,
    pub font_style7: i16,

    /// REGULAR FONT 8: default name, size and style
    pub font_name8: [u8; 32],
    pub filler_r8: u16,
    pub lyric8: u16,
    pub enclosure8: u16,
    pub rel_f_size8: u16,
    pub font_size8: u16,
    pub font_style8: i16,

    /// REGULAR FONT 9: default name, size and style
    pub font_name9: [u8; 32],
    pub filler_r9: u16,
    pub lyric9: u16,
    pub enclosure9: u16,
    pub rel_f_size9: u16,
    pub font_size9: u16,
    pub font_style9: i16,

    // ========================================================================
    // Font table (714 bytes: 2 + MAX_SCOREFONTS*34 + 32)
    // ========================================================================
    /// Number of entries in fontTable
    pub nfonts_used: i16,

    /// To convert stored to system font nos. (20 * 34 = 680 bytes)
    pub font_table: [FontItem; MAX_SCOREFONTS],

    /// Name of this document's music font (Pascal string)
    pub mus_font_name: [u8; 32],

    // ========================================================================
    // Display and editing state (32 bytes)
    // ========================================================================
    /// Current reduce/enlarge magnification, 0=none
    pub magnify: i16,

    /// If sel. is empty, insertion pt's staff, else undefined
    pub sel_staff: i16,

    /// (not yet used) staff no. for meas. nos. besides stf 1
    pub other_mn_staff: i8,

    /// Show measure nos.: -=every system, 0=never, n=every nth meas.
    pub number_meas: i8,

    /// systemNum of system which contains caret (currently not defined)
    pub current_system: i16,

    /// ID of 'SPTB' resource having spacing table to use
    pub space_table: i16,

    /// Percent tightness
    pub htight: i16,

    /// unused
    pub filler_int: i16,

    /// Voice to look at, or ANYONE for all voices
    pub look_voice: i16,

    /// (unused)
    pub filler_hp: i16,

    /// (unused)
    pub filler_lp: i16,

    /// Extra space above/below staff for max. ledger lines (half-spaces)
    pub ledger_y_sp: i16,

    /// Maximum time between consec. attacks in a chord (millisec.)
    pub deflam_time: i16,

    // ========================================================================
    // Boolean flags (7 bytes)
    // ========================================================================
    /// Respace on symbol insert, or leave things alone?
    pub auto_respace: u8,

    /// Graphic insertion logic (else temporal)?
    pub insert_mode: u8,

    /// In beam handling, treat rests like notes?
    pub beam_rests: u8,

    /// Display with notehead graphs or in pianoroll form?
    pub graph_mode: u8,

    /// Show (with InvertSymbolHilite) lines on every Sync?
    pub show_syncs: u8,

    /// Frame systemRects (for debugging)?
    pub frame_systems: u8,

    /// unused
    pub filler_em: u8,

    /// See enum in Defs.h for values
    pub color_voices: u8,

    /// Display invisible objects?
    pub show_invis: u8,

    /// Show measures with duration/time sig. problems?
    pub show_dur_prob: u8,

    /// True if black-key notes recorded should use flats
    pub record_flats: u8,

    // ========================================================================
    // Spacing map (MAX_L_DUR=9 entries, 4 bytes each = 36 bytes)
    // ========================================================================
    /// Ideal spacing of basic (undotted, non-tuplet) durations
    pub space_map: [i32; MAX_L_DUR],

    // ========================================================================
    // System indentation (4 bytes)
    // ========================================================================
    /// Amount to indent first System
    pub d_indent_first: Ddist,

    /// obsolete, was vert. "dead" space btwn Systems
    pub y_between_sys: Ddist,

    // ========================================================================
    // Voice table (MAXVOICES+1 = 101 entries, 3 bytes each = 303 bytes)
    // Plus expansion (255 entries, 3 bytes each = 765 bytes)
    // Total: 1068 bytes
    // ========================================================================
    /// Descriptions of voices in use
    pub voice_tab: [VoiceInfo; MAXVOICES + 1],

    /// Expansion space to reach 256 entries
    pub expansion: [VoiceInfo; 256 - (MAXVOICES + 1)],
}

// ============================================================================
// Parsing Functions for N105 Format
// ============================================================================

impl DocumentHeader {
    /// Parse a DocumentHeader from N105 raw bytes.
    ///
    /// The DocumentHeader is a simple fixed-size struct that can be read
    /// directly with binrw.
    pub fn from_n105_bytes(data: &[u8]) -> Result<Self, binrw::Error> {
        use binrw::BinReaderExt;
        let mut cursor = std::io::Cursor::new(data);
        cursor.read_be()
    }
}

impl ScoreHeader {
    /// Parse a ScoreHeader from N105 raw bytes.
    ///
    /// This function manually parses the 2148-byte N105 score header,
    /// handling the complex layout with TextStyle records, font table,
    /// spacing map, and voice table.
    ///
    /// Source: `Ngale5ProgQuickRef-TN1.txt:112-124`
    pub fn from_n105_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < SCORE_HDR_SIZE_N105 {
            return Err(format!(
                "ScoreHeader requires >={} bytes, got {}",
                SCORE_HDR_SIZE_N105,
                data.len()
            ));
        }

        // Helper to read big-endian types
        fn read_u16_be(data: &[u8], offset: usize) -> u16 {
            u16::from_be_bytes([data[offset], data[offset + 1]])
        }

        fn read_i16_be(data: &[u8], offset: usize) -> i16 {
            i16::from_be_bytes([data[offset], data[offset + 1]])
        }

        fn read_i32_be(data: &[u8], offset: usize) -> i32 {
            i32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        }

        fn read_array<const N: usize>(data: &[u8], offset: usize) -> [u8; N] {
            let mut arr = [0u8; N];
            arr.copy_from_slice(&data[offset..offset + N]);
            arr
        }

        let mut offset = 0;

        // Links and basic metadata (12 bytes)
        let head_l = read_u16_be(data, offset);
        offset += 2;
        let tail_l = read_u16_be(data, offset);
        offset += 2;
        let sel_start_l = read_u16_be(data, offset);
        offset += 2;
        let sel_end_l = read_u16_be(data, offset);
        offset += 2;
        let nstaves = read_i16_be(data, offset);
        offset += 2;
        let nsystems = read_i16_be(data, offset);
        offset += 2;

        // Comment (256 bytes)
        let comment = read_array::<{ MAX_COMMENT_LEN + 1 }>(data, offset);
        offset += MAX_COMMENT_LEN + 1;

        // Config bitfield byte: 8 single-bit fields packed into 1 byte (mac68k alignment)
        // C++: feedback:1, dontSendPatches:1, saved:1, named:1, used:1, transposed:1,
        //      lyricText:1, polyTimbral:1
        // Reference: NDocAndCnfgTypesN105.h lines 53-60
        let config_bits = data[offset];
        offset += 1;
        let note_ins_feedback = (config_bits >> 7) & 1;
        let dont_send_patches = (config_bits >> 6) & 1;
        let saved = (config_bits >> 5) & 1;
        let named = (config_bits >> 4) & 1;
        let used = (config_bits >> 3) & 1;
        let transposed = (config_bits >> 2) & 1;
        let filler_sc1 = (config_bits >> 1) & 1; // lyricText
        let poly_timbral = config_bits & 1;
        // currentPage (Byte) — no longer used
        let filler_sc2 = data[offset]; // currentPage
        offset += 1;

        let space_percent = read_i16_be(data, offset);
        offset += 2;
        let srastral = read_i16_be(data, offset);
        offset += 2;
        let altsrastral = read_i16_be(data, offset);
        offset += 2;
        let tempo = read_i16_be(data, offset);
        offset += 2;
        let channel = read_i16_be(data, offset);
        offset += 2;
        let vel_offset = read_i16_be(data, offset);
        offset += 2;

        let header_str_offset = read_i32_be(data, offset);
        offset += 4;
        let footer_str_offset = read_i32_be(data, offset);
        offset += 4;

        // PGN bitfield byte: packed into 1 byte
        // C++: topPGN:1, hPosPGN:3, alternatePGN:1, useHeaderFooter:1, fillerPGN:2
        // Reference: NDocAndCnfgTypesN105.h lines 72-77
        let pgn_bits = data[offset];
        offset += 1;
        let top_pgn = (pgn_bits >> 7) & 1;
        let h_pos_pgn = (pgn_bits >> 4) & 0x07;
        let alternate_pgn = (pgn_bits >> 3) & 1;
        let use_header_footer = (pgn_bits >> 2) & 1;
        let filler_pgn = pgn_bits & 0x03;
        let filler_mb = data[offset] as i8;
        offset += 1;

        let filler2 = read_i16_be(data, offset);
        offset += 2;
        let d_indent_other = read_i16_be(data, offset);
        offset += 2;

        let first_names = data[offset] as i8;
        offset += 1;
        let other_names = data[offset] as i8;
        offset += 1;
        let last_global_font = data[offset] as i8;
        offset += 1;
        let x_mn_offset = data[offset] as i8;
        offset += 1;
        let y_mn_offset = data[offset] as i8;
        offset += 1;
        let x_sys_mn_offset = data[offset] as i8;
        offset += 1;

        // aboveMN bitfield: 4 fields packed into 1 short (2 bytes)
        // C++: aboveMN:1, sysFirstMN:1, startMNPrint1:1, firstMNNumber:13
        // Reference: NDocAndCnfgTypesN105.h lines 86-89
        let mn_bits = read_u16_be(data, offset);
        offset += 2;
        let above_mn = ((mn_bits >> 15) & 1) as i16;
        let sys_first_mn = ((mn_bits >> 14) & 1) as i16;
        let start_mn_print1 = ((mn_bits >> 13) & 1) as i16;
        let first_mn_number = (mn_bits & 0x1FFF) as i16;

        let master_head_l = read_u16_be(data, offset);
        offset += 2;
        let master_tail_l = read_u16_be(data, offset);
        offset += 2;

        let filler1 = data[offset] as i8;
        offset += 1;
        let n_font_records = data[offset] as i8;
        offset += 1;

        // 15 TEXTSTYLE records (each 36 bytes in N105 = 540 bytes total)
        // Each record is: fontName[32] + filler(2) + lyric(2) + enclosure(2) + relFSize(2) + fontSize(2) + fontStyle(2) = 44 bytes
        // But stored as 36 bytes in N105 due to bitfield packing
        // Layout is fontName[32] + 4 bytes of packed data
        let font_name_mn = read_array::<32>(data, offset);
        offset += 32;
        // Bitfield u16: filler:5, lyric:1, enclosure:2, relFSize:1, fontSize:7
        let mn_bits = read_u16_be(data, offset);
        let filler_mn = (mn_bits >> 11) & 0x1F;
        let lyric_mn = (mn_bits >> 10) & 0x01;
        let enclosure_mn = (mn_bits >> 8) & 0x03;
        let rel_f_size_mn = (mn_bits >> 7) & 0x01;
        let font_size_mn = mn_bits & 0x7F;
        offset += 2;
        let font_style_mn = read_i16_be(data, offset);
        offset += 2;

        // Repeat for other 14 fonts (abbreviated here for brevity)
        // In a real implementation, this would be a loop or macro

        let font_name_pn = read_array::<32>(data, offset);
        offset += 32;
        let pn_bits = read_u16_be(data, offset);
        let filler_pn = (pn_bits >> 11) & 0x1F;
        let lyric_pn = (pn_bits >> 10) & 0x01;
        let enclosure_pn = (pn_bits >> 8) & 0x03;
        let rel_f_size_pn = (pn_bits >> 7) & 0x01;
        let font_size_pn = pn_bits & 0x7F;
        offset += 2;
        let font_style_pn = read_i16_be(data, offset);
        offset += 2;

        // Helper: parse a TEXTSTYLE record (36 bytes = fontName[32] + bitfield u16 + fontStyle i16)
        // Reference: NDocAndCnfgTypesN105.h:92-214
        macro_rules! read_textstyle {
            ($data:expr, $offset:expr) => {{
                let name = read_array::<32>($data, $offset);
                let bits = read_u16_be($data, $offset + 32);
                let filler = (bits >> 11) & 0x1F;
                let lyric = (bits >> 10) & 0x01;
                let enclosure = (bits >> 8) & 0x03;
                let rel_f_size = (bits >> 7) & 0x01;
                let font_size = bits & 0x7F;
                let font_style = read_i16_be($data, $offset + 34);
                $offset += 36; // TEXTSTYLE_SIZE_N105
                (
                    name, filler, lyric, enclosure, rel_f_size, font_size, font_style,
                )
            }};
        }

        let (
            font_name_rm,
            filler_rm,
            lyric_rm,
            enclosure_rm,
            rel_f_size_rm,
            font_size_rm,
            font_style_rm,
        ) = read_textstyle!(data, offset);
        let (font_name1, filler_r1, lyric1, enclosure1, rel_f_size1, font_size1, font_style1) =
            read_textstyle!(data, offset);
        let (font_name2, filler_r2, lyric2, enclosure2, rel_f_size2, font_size2, font_style2) =
            read_textstyle!(data, offset);
        let (font_name3, filler_r3, lyric3, enclosure3, rel_f_size3, font_size3, font_style3) =
            read_textstyle!(data, offset);
        let (font_name4, filler_r4, lyric4, enclosure4, rel_f_size4, font_size4, font_style4) =
            read_textstyle!(data, offset);
        let (
            font_name_tm,
            filler_tm,
            lyric_tm,
            enclosure_tm,
            rel_f_size_tm,
            font_size_tm,
            font_style_tm,
        ) = read_textstyle!(data, offset);
        let (
            font_name_cs,
            filler_cs,
            lyric_cs,
            enclosure_cs,
            rel_f_size_cs,
            font_size_cs,
            font_style_cs,
        ) = read_textstyle!(data, offset);
        let (
            font_name_pg,
            filler_pg,
            lyric_pg,
            enclosure_pg,
            rel_f_size_pg,
            font_size_pg,
            font_style_pg,
        ) = read_textstyle!(data, offset);
        let (font_name5, filler_r5, lyric5, enclosure5, rel_f_size5, font_size5, font_style5) =
            read_textstyle!(data, offset);
        let (font_name6, filler_r6, lyric6, enclosure6, rel_f_size6, font_size6, font_style6) =
            read_textstyle!(data, offset);
        let (font_name7, filler_r7, lyric7, enclosure7, rel_f_size7, font_size7, font_style7) =
            read_textstyle!(data, offset);
        let (font_name8, filler_r8, lyric8, enclosure8, rel_f_size8, font_size8, font_style8) =
            read_textstyle!(data, offset);
        let (font_name9, filler_r9, lyric9, enclosure9, rel_f_size9, font_size9, font_style9) =
            read_textstyle!(data, offset);

        // Font table (714 bytes)
        let nfonts_used = read_i16_be(data, offset);
        offset += 2;

        let mut font_table = [FontItem {
            font_id: 0,
            font_name: [0u8; 32],
        }; MAX_SCOREFONTS];

        for item in font_table.iter_mut().take(MAX_SCOREFONTS) {
            item.font_id = read_i16_be(data, offset);
            offset += 2;
            item.font_name = read_array::<32>(data, offset);
            offset += 32;
        }

        let mus_font_name = read_array::<32>(data, offset);
        offset += 32;

        // Display and editing state (32 bytes)
        let magnify = read_i16_be(data, offset);
        offset += 2;
        let sel_staff = read_i16_be(data, offset);
        offset += 2;
        let other_mn_staff = data[offset] as i8;
        offset += 1;
        let number_meas = data[offset] as i8;
        offset += 1;
        let current_system = read_i16_be(data, offset);
        offset += 2;
        let space_table = read_i16_be(data, offset);
        offset += 2;
        let htight = read_i16_be(data, offset);
        offset += 2;
        let filler_int = read_i16_be(data, offset);
        offset += 2;
        let look_voice = read_i16_be(data, offset);
        offset += 2;
        let filler_hp = read_i16_be(data, offset);
        offset += 2;
        let filler_lp = read_i16_be(data, offset);
        offset += 2;
        let ledger_y_sp = read_i16_be(data, offset);
        offset += 2;
        let deflam_time = read_i16_be(data, offset);
        offset += 2;

        // Boolean flags (11 bytes)
        let auto_respace = data[offset];
        offset += 1;
        let insert_mode = data[offset];
        offset += 1;
        let beam_rests = data[offset];
        offset += 1;
        let graph_mode = data[offset];
        offset += 1;
        let show_syncs = data[offset];
        offset += 1;
        let frame_systems = data[offset];
        offset += 1;
        let filler_em = data[offset];
        offset += 1;
        let color_voices = data[offset];
        offset += 1;
        let show_invis = data[offset];
        offset += 1;
        let show_dur_prob = data[offset];
        offset += 1;
        let record_flats = data[offset];
        offset += 1;

        // Spacing map (36 bytes = 9 * 4)
        let mut space_map = [0i32; MAX_L_DUR];
        for item in space_map.iter_mut() {
            *item = read_i32_be(data, offset);
            offset += 4;
        }

        // System indentation (4 bytes)
        let d_indent_first = read_i16_be(data, offset);
        offset += 2;
        let y_between_sys = read_i16_be(data, offset);
        offset += 2;

        // Voice table (MAXVOICES+1 = 101 entries, 3 bytes each = 303 bytes)
        let mut voice_tab = [VoiceInfo::default(); MAXVOICES + 1];
        for item in voice_tab.iter_mut() {
            item.partn = data[offset];
            offset += 1;
            item.voice_role = data[offset];
            offset += 1;
            item.rel_voice = data[offset];
            offset += 1;
        }

        // Expansion (155 entries, 3 bytes each = 465 bytes)
        // Guard against buffer overrun: some N105 files have exactly 2148 bytes,
        // and with correct bitfield packing the voice+expansion table fits tightly.
        let mut expansion = [VoiceInfo::default(); 256 - (MAXVOICES + 1)];
        for item in expansion.iter_mut() {
            if offset + 3 > data.len() {
                break;
            }
            item.partn = data[offset];
            offset += 1;
            item.voice_role = data[offset];
            offset += 1;
            item.rel_voice = data[offset];
            offset += 1;
        }

        Ok(ScoreHeader {
            head_l,
            tail_l,
            sel_start_l,
            sel_end_l,
            nstaves,
            nsystems,
            comment,
            note_ins_feedback,
            dont_send_patches,
            saved,
            named,
            used,
            transposed,
            filler_sc1,
            poly_timbral,
            filler_sc2,
            space_percent,
            srastral,
            altsrastral,
            tempo,
            channel,
            vel_offset,
            header_str_offset,
            footer_str_offset,
            top_pgn,
            h_pos_pgn,
            alternate_pgn,
            use_header_footer,
            filler_pgn,
            filler_mb,
            filler2,
            d_indent_other,
            first_names,
            other_names,
            last_global_font,
            x_mn_offset,
            y_mn_offset,
            x_sys_mn_offset,
            above_mn,
            sys_first_mn,
            start_mn_print1,
            first_mn_number,
            master_head_l,
            master_tail_l,
            filler1,
            n_font_records,
            font_name_mn,
            filler_mn,
            lyric_mn,
            enclosure_mn,
            rel_f_size_mn,
            font_size_mn,
            font_style_mn,
            font_name_pn,
            filler_pn,
            lyric_pn,
            enclosure_pn,
            rel_f_size_pn,
            font_size_pn,
            font_style_pn,
            font_name_rm,
            filler_rm,
            lyric_rm,
            enclosure_rm,
            rel_f_size_rm,
            font_size_rm,
            font_style_rm,
            font_name1,
            filler_r1,
            lyric1,
            enclosure1,
            rel_f_size1,
            font_size1,
            font_style1,
            font_name2,
            filler_r2,
            lyric2,
            enclosure2,
            rel_f_size2,
            font_size2,
            font_style2,
            font_name3,
            filler_r3,
            lyric3,
            enclosure3,
            rel_f_size3,
            font_size3,
            font_style3,
            font_name4,
            filler_r4,
            lyric4,
            enclosure4,
            rel_f_size4,
            font_size4,
            font_style4,
            font_name_tm,
            filler_tm,
            lyric_tm,
            enclosure_tm,
            rel_f_size_tm,
            font_size_tm,
            font_style_tm,
            font_name_cs,
            filler_cs,
            lyric_cs,
            enclosure_cs,
            rel_f_size_cs,
            font_size_cs,
            font_style_cs,
            font_name_pg,
            filler_pg,
            lyric_pg,
            enclosure_pg,
            rel_f_size_pg,
            font_size_pg,
            font_style_pg,
            font_name5,
            filler_r5,
            lyric5,
            enclosure5,
            rel_f_size5,
            font_size5,
            font_style5,
            font_name6,
            filler_r6,
            lyric6,
            enclosure6,
            rel_f_size6,
            font_size6,
            font_style6,
            font_name7,
            filler_r7,
            lyric7,
            enclosure7,
            rel_f_size7,
            font_size7,
            font_style7,
            font_name8,
            filler_r8,
            lyric8,
            enclosure8,
            rel_f_size8,
            font_size8,
            font_style8,
            font_name9,
            filler_r9,
            lyric9,
            enclosure9,
            rel_f_size9,
            font_size9,
            font_style9,
            nfonts_used,
            font_table,
            mus_font_name,
            magnify,
            sel_staff,
            other_mn_staff,
            number_meas,
            current_system,
            space_table,
            htight,
            filler_int,
            look_voice,
            filler_hp,
            filler_lp,
            ledger_y_sp,
            deflam_time,
            auto_respace,
            insert_mode,
            beam_rests,
            graph_mode,
            show_syncs,
            frame_systems,
            filler_em,
            color_voices,
            show_invis,
            show_dur_prob,
            record_flats,
            space_map,
            d_indent_first,
            y_between_sys,
            voice_tab,
            expansion,
        })
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DOC_HDR_SIZE, 72);
        assert_eq!(SCORE_HDR_SIZE_N105, 2148);
        assert_eq!(DOC_HDR_OFFSET, 8);
        assert_eq!(SCORE_HDR_OFFSET, 80);
        assert_eq!(N_FONT_RECORDS, 15);
        assert_eq!(TEXTSTYLE_SIZE_N105, 36);
    }

    #[test]
    fn test_document_header_size() {
        // DocumentHeader should be exactly 72 bytes when serialized
        // This test verifies the struct layout matches N105 format
        assert_eq!(
            std::mem::size_of::<DocumentHeader>(),
            DOC_HDR_SIZE,
            "DocumentHeader must be exactly 72 bytes"
        );
    }

    #[test]
    fn test_fontitem_size() {
        // FontItem should be exactly 34 bytes (2 + 32)
        assert_eq!(
            std::mem::size_of::<FontItem>(),
            FONTITEM_SIZE,
            "FontItem must be exactly 34 bytes"
        );
    }
}
