//! Heap interpreter: decodes N105 binary format to typed Rust structs.
//!
//! This module provides the critical layer between raw .ngl file bytes and the
//! high-level Rust data model. It handles:
//!
//! - **N105 object header unpacking** (23-byte common prefix + type-specific data)
//! - **PowerPC bitfield unpacking** (MSB-first within bytes)
//! - **Subobject decoding** (30-byte ANOTE records, etc.)
//! - **Sequential subobject walking** for BeamSets and Slurs
//!
//! ## Critical N105 Details
//!
//! - **1-based heap indexing**: Slot 0 is unused, NILINK=0 means "no link"
//! - **Big-endian encoding**: All multi-byte values are big-endian
//! - **Bitfield packing**: PowerPC convention = MSB first within bytes
//!
//! ## Example: OBJECTHEADER_5 (23 bytes)
//!
//! ```text
//! Offset  Type      Field
//! ------  --------  -----
//! 0-1     u16       right
//! 2-3     u16       left
//! 4-5     u16       firstSubObj
//! 6-7     i16       xd
//! 8-9     i16       yd
//! 10      i8        type
//! 11      byte      flags: selected:1 | visible:1 | soft:1 | valid:1 | tweaked:1 | spareFlag:1 | filler:2
//! 12-19   Rect      objRect (4 x i16)
//! 20      i8        relSize
//! 21      i8        filler
//! 22      u8        nEntries
//! ```
//!
//! Source: NObjTypesN105.h lines 12-27, Ngale5ProgQuickRef-TN1.txt lines 214-229

use crate::basic_types::{DRect, Link, Rect, NILINK};
use crate::obj_types::{
    AClef, AConnect, ADynamic, AGraphic, AKeySig, AMeasure, AModNr, ANote, ANoteBeam, ANoteOttava,
    ANoteTuple, APsMeas, ARptEnd, ASlur, AStaff, ATimeSig, BeamSet, Clef, Connect, Dynamic, Ending,
    GrSync, Graphic, Header, KeySig, Measure, ObjectHeader, Ottava, Page, PsMeas, RptEnd, Slur,
    Spacer, Staff, Sync, System, Tail, Tempo, TimeSig, Tuplet,
};
use std::collections::HashMap;

// Re-export all unpacking functions from submodules for backward compatibility
pub use super::unpack_headers::{
    unpack_ksinfo_n105, unpack_object_header_n105, unpack_subobj_header_n105,
};
pub use super::unpack_notation::{unpack_aclef_n105, unpack_akeysig_n105, unpack_atimesig_n105};
pub use super::unpack_notes::{unpack_anote_n105, unpack_anotebeam_n105, unpack_anotetuple_n105};
pub use super::unpack_slur::unpack_aslur_n105;
pub use super::unpack_structural::{unpack_ameasure_n105, unpack_astaff_n105};
pub use super::unpack_stubs::{
    unpack_aconnect_n105, unpack_adynamic_n105, unpack_agraphic_n105, unpack_amodnr_n105,
    unpack_anoteottava_n105, unpack_apsmeas_n105, unpack_arptend_n105, unpack_partinfo,
};

/// InterpretedScore: all decoded objects and subobjects from a .ngl file.
///
/// This struct holds the fully interpreted score data, organized by type for efficient access.
#[derive(Debug, Clone)]
pub struct InterpretedScore {
    /// Version of the NGL format (N103, N105, N106, etc.)
    /// Used to preserve format version when round-tripping through write_to_bytes()
    pub version: crate::ngl::reader::NglVersion,

    /// Link to the score HEADER object (start of score linked list).
    /// For NGL files, this comes from ScoreHeader.head_l (same as OG doc->headL).
    /// For Notelist-generated scores, this is the first HEADER link.
    /// The walk() method starts from this object's `right` pointer.
    pub head_l: Link,

    /// All objects in heap order (1-based indexing: slot 0 unused)
    pub objects: Vec<InterpretedObject>,

    // Score header fields needed for rendering
    /// Code for drawing part names on first system (0=none, 1=abbrev, 2=full)
    pub first_names: i8,
    /// Code for drawing part names on other systems
    pub other_names: i8,
    /// Amount to indent first System (DDIST)
    pub d_indent_first: i16,
    /// Amount to indent Systems other than first (DDIST)
    pub d_indent_other: i16,
    /// Show measure nos.: -=every system, 0=never, n=every nth meas.
    pub number_meas: i8,
    /// True=measure numbers above staff, else below
    pub above_mn: bool,
    /// Number of first measure
    pub first_mn_number: i16,
    /// Horiz. pos. offset for meas. nos. (half-spaces)
    pub x_mn_offset: i8,
    /// Vert. pos. offset for meas. nos. (half-spaces)
    pub y_mn_offset: i8,
    /// Horiz. pos. offset for meas. nos. if 1st meas. in system (half-spaces)
    pub x_sys_mn_offset: i8,
    /// True=indent 1st meas. of system by xSysMNOffset
    pub sys_first_mn: bool,
    /// True=First meas. number to print is 1, else 2
    pub start_mn_print1: bool,

    // Page geometry
    /// Page width in points (from orig_paper_rect or layout config)
    pub page_width_pt: f32,
    /// Page height in points (from orig_paper_rect or layout config)
    pub page_height_pt: f32,
    /// Page number of zero'th sheet (OG doc->firstPageNumber, default 1)
    pub first_page_number: i16,

    // Subobject storage by type
    /// Type 0 subobjects: decoded PARTINFO structs
    pub part_infos: Vec<crate::obj_types::PartInfo>,
    /// Type 2 subobjects: ANOTE (30 bytes)
    pub notes: HashMap<Link, Vec<ANote>>,
    /// Type 3 subobjects: ARPTEND (6 bytes)
    pub rptend_subs: HashMap<Link, Vec<ARptEnd>>,
    /// Type 6 subobjects: ASTAFF (50 bytes)
    pub staffs: HashMap<Link, Vec<AStaff>>,
    /// Type 7 subobjects: AMEASURE (40 bytes)
    pub measures: HashMap<Link, Vec<AMeasure>>,
    /// Type 8 subobjects: ACLEF (10 bytes)
    pub clefs: HashMap<Link, Vec<AClef>>,
    /// Type 9 subobjects: AKEYSIG (24 bytes)
    pub keysigs: HashMap<Link, Vec<AKeySig>>,
    /// Type 10 subobjects: ATIMESIG (12 bytes)
    pub timesigs: HashMap<Link, Vec<ATimeSig>>,
    /// Type 11 subobjects: ANOTEBEAM (6 bytes)
    pub notebeams: HashMap<Link, Vec<ANoteBeam>>,
    /// Type 12 subobjects: ACONNECT (12 bytes)
    pub connects: HashMap<Link, Vec<AConnect>>,
    /// Type 13 subobjects: ADYNAMIC (12 bytes)
    pub dynamics: HashMap<Link, Vec<ADynamic>>,
    /// Type 14 subobjects: AMODNR (6 bytes)
    pub modnrs: HashMap<Link, Vec<AModNr>>,
    /// Type 15 subobjects: AGRAPHIC (6 bytes)
    pub graphics: HashMap<Link, Vec<AGraphic>>,
    /// Type 16 subobjects: ANOTEOTTAVA (4 bytes)
    pub ottavas: HashMap<Link, Vec<ANoteOttava>>,
    /// Type 17 subobjects: ASLUR (42 bytes)
    pub slurs: HashMap<Link, Vec<ASlur>>,
    /// Type 18 subobjects: ANOTETUPLE (4 bytes)
    pub tuplets: HashMap<Link, Vec<ANoteTuple>>,
    /// Type 19 subobjects: AGRNOTE (30 bytes, same as ANOTE)
    pub grnotes: HashMap<Link, Vec<ANote>>,
    /// Type 23 subobjects: APSMEAS (6 bytes)
    pub psmeas_subs: HashMap<Link, Vec<APsMeas>>,

    /// Resolved text strings for GRAPHIC objects.
    /// Key is the GRAPHIC object's first_sub_obj link, value is the decoded string.
    /// Resolved at interpretation time from AGRAPHIC.strOffset + string pool.
    pub graphic_strings: HashMap<Link, String>,

    /// Resolved tempo strings for TEMPO objects.
    /// Key is the object's 1-based index (Link), value is (verbal_string, metro_string).
    /// verbal_string: e.g. "Allegro", metro_string: e.g. "120"
    /// Resolved at interpretation time from Tempo.str_offset + Tempo.metro_str_offset.
    pub tempo_strings: HashMap<Link, (String, String)>,

    /// Text styles parsed from the score header (15 entries, indexed by FONT_* constants).
    /// Each entry: (font_name, font_size, font_style, is_lyric, enclosure, rel_f_size).
    pub text_styles: Vec<TextStyle>,

    /// Font table from the score header (up to 20 entries, indexed by fontInd).
    /// Each entry is a font name string. Used to resolve GRAPHIC objects with
    /// info=0 (FONT_THISITEMONLY) where fontInd indexes into this table.
    /// Reference: NDocAndCnfgTypes.h FONTITEM, DrawUtils.cp GetGraphicFontInfo()
    pub font_names: Vec<String>,

    // === Header/footer text rendering fields ===
    // Reference: DrawObject.cp DrawHeaderFooter() lines 60-177,
    //            HeaderFooterDialog.cp GetHeaderFooterStrings() lines 64-118
    /// True=use header/footer text, False=simple page number only.
    /// Reference: NDocAndCnfgTypes.h doc->useHeaderFooter
    pub use_header_footer: bool,

    /// True=page numbers at top of page, else bottom.
    /// Reference: NDocAndCnfgTypes.h doc->topPGN
    pub top_pgn: bool,

    /// Page number horizontal position: 1=left, 2=center, 3=right.
    /// Reference: NDocAndCnfgTypes.h doc->hPosPGN
    pub h_pos_pgn: u8,

    /// True=page numbers alternately left and right on even/odd pages.
    /// Reference: NDocAndCnfgTypes.h doc->alternatePGN
    pub alternate_pgn: bool,

    /// First printed page number (pages before this are not numbered).
    /// Reference: NDocAndCnfgTypes.h doc->startPageNumber
    pub start_page_number: i16,

    /// Header template string (decoded from string pool).
    /// Format: "leftText\x01centerText\x01rightText" with 0x01 delimiters.
    /// '#' is the page number placeholder character.
    /// Reference: HeaderFooterDialog.cp HEADERFOOTER_DELIM_CHAR
    pub header_str: String,

    /// Footer template string (decoded from string pool).
    /// Same format as header_str.
    pub footer_str: String,

    /// Header/footer margins (top, left, bottom, right) in points.
    /// Reference: NDocAndCnfgTypes.h doc->headerFooterMargins
    pub hf_margin_top: f32,
    pub hf_margin_left: f32,
    pub hf_margin_bottom: f32,
    pub hf_margin_right: f32,

    /// Page header/footer font name.
    /// Reference: NDocAndCnfgTypes.h doc->fontNamePG
    pub pg_font_name: String,

    /// Page header/footer font size (points).
    /// Reference: NDocAndCnfgTypes.h doc->fontSizePG
    pub pg_font_size: f32,

    /// Page header/footer font style (Mac TextFace bitfield: 1=bold, 2=italic, etc.).
    /// Reference: NDocAndCnfgTypes.h doc->fontStylePG
    pub pg_font_style: i16,

    // === Score metadata (title, composer, etc.) ===
    // Populated from MusicXML <work-title>, <movement-title>, <identification>/<creator>
    // elements, or from GrString GRAPHIC objects in NGL files.
    /// Score title (from MusicXML movement-title or work-title, or NGL GrString).
    pub title: String,

    /// Composer name (from MusicXML <creator type="composer">, or NGL GrString).
    pub composer: String,
}

/// Text style record parsed from the N105 score header.
///
/// 15 of these are stored at offset 390 (0x186) in the score header, each 36 bytes.
/// Indexed by FONT_THISITEMONLY=0, FONT_MN=1, FONT_PN=2, FONT_RM=3, FONT_R1=4, etc.
///
/// Source: NBasicTypesN105.h lines 53-61
#[derive(Debug, Clone)]
pub struct TextStyle {
    /// Font name (Pascal string, up to 31 chars)
    pub font_name: String,
    /// True if font size is relative to staff size
    pub rel_f_size: bool,
    /// Font size (absolute pt or relative code)
    pub font_size: u8,
    /// Font style (bold, italic, etc. — Mac TextFace bitfield)
    pub font_style: i16,
    /// True if lyric spacing
    pub lyric: bool,
    /// Enclosure type (0=none, 1=box, 2=circle)
    pub enclosure: u8,
}

/// InterpretedObject: a single object with its header and type-specific data.
#[derive(Debug, Clone)]
pub struct InterpretedObject {
    /// 1-based index in heap (matches LINK values)
    pub index: Link,
    /// Common object header
    pub header: ObjectHeader,
    /// Type-specific data
    pub data: ObjData,
}

/// ObjData: type-specific object data (enum of all object types).
#[derive(Debug, Clone)]
pub enum ObjData {
    Header(Header),
    Tail(Tail),
    Sync(Sync),
    RptEnd(RptEnd),
    Page(Page),
    System(System),
    Staff(Staff),
    Measure(Measure),
    Clef(Clef),
    KeySig(KeySig),
    TimeSig(TimeSig),
    BeamSet(BeamSet),
    Connect(Connect),
    Dynamic(Dynamic),
    Graphic(Graphic),
    Ottava(Ottava),
    Slur(Slur),
    Tuplet(Tuplet),
    GrSync(GrSync),
    Tempo(Tempo),
    Spacer(Spacer),
    Ending(Ending),
    PsMeas(PsMeas),
}

impl Default for InterpretedScore {
    fn default() -> Self {
        Self::new()
    }
}

impl InterpretedScore {
    /// Create a new empty InterpretedScore.
    pub fn new() -> Self {
        Self {
            head_l: NILINK,
            objects: Vec::new(),
            first_names: 0,
            other_names: 0,
            d_indent_first: 0,
            d_indent_other: 0,
            number_meas: 0,
            above_mn: true,
            first_mn_number: 1,
            x_mn_offset: 0,
            y_mn_offset: 0,
            x_sys_mn_offset: 0,
            sys_first_mn: false,
            start_mn_print1: true,
            page_width_pt: 612.0,
            page_height_pt: 792.0,
            first_page_number: 1,
            part_infos: Vec::new(),
            notes: HashMap::new(),
            rptend_subs: HashMap::new(),
            staffs: HashMap::new(),
            measures: HashMap::new(),
            clefs: HashMap::new(),
            keysigs: HashMap::new(),
            timesigs: HashMap::new(),
            notebeams: HashMap::new(),
            connects: HashMap::new(),
            dynamics: HashMap::new(),
            modnrs: HashMap::new(),
            graphics: HashMap::new(),
            ottavas: HashMap::new(),
            slurs: HashMap::new(),
            tuplets: HashMap::new(),
            grnotes: HashMap::new(),
            psmeas_subs: HashMap::new(),
            graphic_strings: HashMap::new(),
            tempo_strings: HashMap::new(),
            text_styles: Vec::new(),
            font_names: Vec::new(),
            // Header/footer defaults
            use_header_footer: false,
            top_pgn: false,
            h_pos_pgn: 2, // CENTER
            alternate_pgn: false,
            start_page_number: 1,
            header_str: String::new(),
            footer_str: String::new(),
            hf_margin_top: 36.0,
            hf_margin_left: 36.0,
            hf_margin_bottom: 36.0,
            hf_margin_right: 36.0,
            pg_font_name: "Helvetica".to_string(),
            pg_font_size: 10.0,
            pg_font_style: 0,
            // Score metadata
            title: String::new(),
            composer: String::new(),
            // Default to N105 format
            version: crate::ngl::reader::NglVersion::N105,
        }
    }

    /// Get an object by link.
    ///
    /// First tries fast index-based lookup (link == index + 1, true for NGL-parsed scores).
    /// Falls back to linear search for synthesized scores where links may not match indices.
    ///
    /// Returns `None` if link is NILINK or not found.
    pub fn get(&self, link: Link) -> Option<&InterpretedObject> {
        if link == NILINK || link == 0 {
            return None;
        }
        // Fast path: check if link == index + 1 (true for NGL binary files)
        let idx = (link - 1) as usize;
        if let Some(obj) = self.objects.get(idx) {
            if obj.index == link {
                return Some(obj);
            }
        }
        // Slow path: linear search (for synthesized scores with non-sequential links)
        self.objects.iter().find(|obj| obj.index == link)
    }

    /// Walk objects in linked-list order (following `right` links).
    ///
    /// Starts from the HEADER object at `head_l` (equivalent to OG `doc->headL`)
    /// and follows `right` pointers through to TAIL. This correctly skips
    /// the master page object list which shares the same heap.
    ///
    /// Reference: HeapFileIO.cp, WriteHeap() — score list starts at headL,
    /// master page list starts at masterHeadL.
    pub fn walk(&self) -> impl Iterator<Item = &InterpretedObject> {
        // Start from the HEADER object identified by head_l.
        // The first object yielded is the one HEADER.right points to
        // (usually the first PAGE object).
        let start = if self.head_l != NILINK {
            self.get(self.head_l).map(|obj| obj.header.right)
        } else {
            // Fallback: if head_l not set, use first object (legacy behavior)
            self.objects.first().map(|obj| obj.header.right)
        };
        ObjectWalker {
            score: self,
            current: start,
        }
    }

    /// Get notes for a Sync (or GrSync).
    ///
    /// Returns the notes starting at `first_sub` in the notes HashMap.
    pub fn get_notes(&self, first_sub: Link) -> Vec<ANote> {
        self.notes.get(&first_sub).cloned().unwrap_or_default()
    }

    /// Get notebeam subobjects for a BeamSet (sequential, not chained).
    ///
    /// BeamSets use **sequential storage**: subobjects are stored consecutively
    /// starting at `first_sub`, NOT linked by next-pointers.
    pub fn get_notebeam_subs(&self, first_sub: Link, count: u8) -> Vec<ANoteBeam> {
        self.notebeams
            .get(&first_sub)
            .map(|beams| beams.iter().take(count as usize).cloned().collect())
            .unwrap_or_default()
    }

    /// Get slur subobjects for a Slur (sequential, not chained).
    ///
    /// Slurs use **sequential storage**: subobjects are stored consecutively
    /// starting at `first_sub`, NOT linked by next-pointers.
    pub fn get_slur_subs(&self, first_sub: Link, count: u8) -> Vec<ASlur> {
        self.slurs
            .get(&first_sub)
            .map(|slurs| slurs.iter().take(count as usize).cloned().collect())
            .unwrap_or_default()
    }

    /// Get ottava subobjects for an Ottava (sequential, not chained).
    ///
    /// Each ANOTEOTTAVA contains an opSync link pointing to the Sync
    /// under the ottava bracket.
    pub fn get_ottava_subs(&self, first_sub: Link, count: u8) -> Vec<ANoteOttava> {
        self.ottavas
            .get(&first_sub)
            .map(|octs| octs.iter().take(count as usize).cloned().collect())
            .unwrap_or_default()
    }

    /// Get the first Sync link under an Ottava.
    ///
    /// Equivalent to C++ FirstInOttava(): returns the opSync of the
    /// first ANOTEOTTAVA subobject.
    ///
    /// Reference: Ottava.cp lines 871-877
    pub fn first_in_ottava(&self, first_sub: Link, count: u8) -> Option<Link> {
        let subs = self.get_ottava_subs(first_sub, count);
        subs.first().map(|s| s.op_sync)
    }

    /// Get the last Sync link under an Ottava.
    ///
    /// Equivalent to C++ LastInOttava(): returns the opSync of the
    /// last ANOTEOTTAVA subobject.
    ///
    /// Reference: Ottava.cp lines 879-892
    pub fn last_in_ottava(&self, first_sub: Link, count: u8) -> Option<Link> {
        let subs = self.get_ottava_subs(first_sub, count);
        subs.last().map(|s| s.op_sync)
    }

    /// Compute an object's x-offset relative to its System.
    ///
    /// Mirrors C++ `SysRelxd()` from DSUtils.cp:568-578.
    /// For a Measure: returns measure.xd.
    /// For other objects inside a measure: returns measure.xd + obj.xd.
    /// For objects before the first measure: returns obj.xd.
    pub fn sys_rel_xd(&self, link: Link) -> i32 {
        let obj = match self.get(link) {
            Some(o) => o,
            None => return 0,
        };
        let obj_xd = obj.header.xd as i32;

        // If it IS a Measure, just return its xd
        if obj.header.obj_type == MEASURE_TYPE as i8 {
            return obj_xd;
        }

        // Walk left to find the enclosing Measure
        let mut cur = obj.header.left;
        let mut steps = 0;
        while cur != NILINK && steps < 5000 {
            if let Some(left_obj) = self.get(cur) {
                if left_obj.header.obj_type == MEASURE_TYPE as i8 {
                    // Found enclosing measure
                    return left_obj.header.xd as i32 + obj_xd;
                }
                cur = left_obj.header.left;
            } else {
                break;
            }
            steps += 1;
        }

        // Before first measure of system — just return obj.xd
        obj_xd
    }

    /// Get the head (first) object of the score list.
    ///
    /// Returns the first HEADER object if present (index 1 in typical files).
    pub fn head(&self) -> Option<&InterpretedObject> {
        self.objects.first()
    }

    /// Get the tail (last) object of the score list.
    ///
    /// Returns the last object following the linked list from head.
    /// In practice, this walks the `right` links until finding NILINK.
    pub fn tail(&self) -> Option<&InterpretedObject> {
        if self.objects.is_empty() {
            return None;
        }

        // Walk from head following right links until we find the tail
        let mut current = self.objects.first()?;
        while current.header.right != NILINK {
            current = self.get(current.header.right)?;
        }
        Some(current)
    }

    /// Count the number of staves in the score.
    ///
    /// This counts AStaff subobjects in the first Staff object found in the score.
    /// All Staff objects in a Nightingale score have the same number of staves.
    pub fn num_staves(&self) -> usize {
        // Find the first Staff object (type 6)
        for obj in &self.objects {
            if obj.header.obj_type == STAFF_TYPE as i8 {
                return obj.header.n_entries as usize;
            }
        }
        0
    }

    /// Get the score object list (HEADER→...→TAIL) as a Vec.
    ///
    /// Returns all objects in the main score list by walking the `right` links.
    pub fn score_list(&self) -> Vec<&InterpretedObject> {
        let mut result = Vec::new();
        if self.objects.is_empty() {
            return result;
        }

        // Start from the first object (should be HEADER)
        let mut current_link = 1;
        while let Some(obj) = self.get(current_link) {
            result.push(obj);
            if obj.header.right == NILINK {
                break;
            }
            current_link = obj.header.right;
        }
        result
    }

    /// Get the master page list (second HEADER→...→TAIL) as a Vec.
    ///
    /// The master page list typically starts after the main score list.
    /// We identify it by finding a second HEADER object.
    pub fn master_page_list(&self) -> Vec<&InterpretedObject> {
        let mut result = Vec::new();

        // Find the second HEADER (master page list head)
        let mut header_count = 0;
        let mut start_link = NILINK;

        for obj in &self.objects {
            if obj.header.obj_type == HEADER_TYPE as i8 {
                header_count += 1;
                if header_count == 2 {
                    start_link = obj.index;
                    break;
                }
            }
        }

        if start_link == NILINK {
            return result;
        }

        // Walk the master page list
        let mut current_link = start_link;
        while let Some(obj) = self.get(current_link) {
            result.push(obj);
            if obj.header.right == NILINK {
                break;
            }
            current_link = obj.header.right;
        }
        result
    }

    /// Check if a link references a PAGE object.
    ///
    /// Used to detect page-relative GRAPHICs/TEMPOs whose firstObj is a PAGE.
    /// Reference: InfoDialog.cp:672-680 (PageRelGraphic)
    pub fn is_page_type(&self, link: Link) -> bool {
        self.get(link)
            .is_some_and(|obj| obj.header.obj_type == PAGE_TYPE as i8)
    }

    /// Compute the page-relative staff_left for a given anchor object and staff.
    ///
    /// Walks backward from `anchor_link` through the linked list to find
    /// the enclosing SYSTEM and STAFF objects, then computes:
    ///   staff_left = system.systemRect.left + astaff.staffLeft
    ///
    /// This mirrors OG `GetContext(doc, relObjL, staffn, &context)` which
    /// computes context at a specific object, rather than using the running
    /// ContextState (which may reflect a later system).
    ///
    /// Returns None if the enclosing system/staff can't be found.
    /// Reference: Context.cp:184-420 (GetContext)
    pub fn staff_left_at(&self, anchor_link: Link, staffn: i8) -> Option<i16> {
        // Walk left from anchor to find the enclosing STAFF, then SYSTEM.
        let mut cur = anchor_link;
        let mut staff_left_rel: Option<i16> = None; // aStaff.staffLeft (relative to system)
        let mut system_left: Option<i16> = None; // system.systemRect.left

        let mut steps = 0;
        while cur != NILINK && steps < 10000 {
            if let Some(obj) = self.get(cur) {
                match obj.header.obj_type as u8 {
                    STAFF_TYPE => {
                        // Look for the AStaff subobject for our staffn
                        if staff_left_rel.is_none() {
                            if let Some(astaff_list) = self.staffs.get(&obj.header.first_sub_obj) {
                                for astaff in astaff_list {
                                    if astaff.staffn == staffn {
                                        staff_left_rel = Some(astaff.staff_left);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    SYSTEM_TYPE => {
                        // Found the enclosing system
                        if let ObjData::System(sys) = &obj.data {
                            system_left = Some(sys.system_rect.left);
                        }
                        break; // System found — stop walking
                    }
                    _ => {}
                }
                cur = obj.header.left;
            } else {
                break;
            }
            steps += 1;
        }

        match (system_left, staff_left_rel) {
            (Some(sl), Some(stl)) => Some(stl.saturating_add(sl)),
            _ => None,
        }
    }

    /// Returns the number of objects with the given type byte.
    pub fn count_by_type(&self, obj_type: u8) -> usize {
        self.objects
            .iter()
            .filter(|obj| obj.header.obj_type == obj_type as i8)
            .count()
    }

    /// Get all SYNCs (note/rest containers) from the score list.
    ///
    /// Returns only objects with type SYNC_TYPE (2).
    pub fn syncs(&self) -> Vec<&InterpretedObject> {
        self.objects
            .iter()
            .filter(|obj| obj.header.obj_type == SYNC_TYPE as i8)
            .collect()
    }

    /// Get all MEASUREs from the score list.
    ///
    /// Returns only objects with type MEASURE_TYPE (7).
    pub fn measure_objects(&self) -> Vec<&InterpretedObject> {
        self.objects
            .iter()
            .filter(|obj| obj.header.obj_type == MEASURE_TYPE as i8)
            .collect()
    }

    /// Decode a string from the string pool at the given offset.
    ///
    /// This is a convenience wrapper around the reader's decode_string function.
    /// The string pool is typically from NglFile::string_pool.
    ///
    /// Returns Some(String) if successful, None if the offset is invalid.
    pub fn decode_string(pool: &[u8], offset: i32) -> Option<String> {
        reader_decode_string(pool, offset)
    }
}

struct ObjectWalker<'a> {
    score: &'a InterpretedScore,
    current: Option<Link>,
}

impl<'a> Iterator for ObjectWalker<'a> {
    type Item = &'a InterpretedObject;

    fn next(&mut self) -> Option<Self::Item> {
        let link = self.current?;
        let obj = self.score.get(link)?;
        self.current = if obj.header.right != NILINK {
            Some(obj.header.right)
        } else {
            None
        };
        Some(obj)
    }
}

// =============================================================================
// Heap Interpretation
// =============================================================================

// NOTE: All N105 unpacking functions have been moved to dedicated submodules:
//   unpack_headers.rs    — ObjectHeader, SubObjHeader, KsInfo
//   unpack_notes.rs      — ANote, ANoteBeam, ANoteTuple
//   unpack_structural.rs — AStaff, AMeasure
//   unpack_notation.rs   — AClef, AKeySig, ATimeSig
//   unpack_slur.rs       — ASlur
//   unpack_stubs.rs      — AConnect, ADynamic, AModNr, AGraphic, ANoteOttava, ARptEnd, APsMeas
// They are re-exported via `pub use` at the top of this file for backward compatibility.

use crate::defs::*;
use crate::ngl::reader::{decode_string as reader_decode_string, NglFile};

/// Interpret all heaps from an NGL file into an InterpretedScore.
///
/// This is the main entry point for converting raw .ngl binary data into
/// typed Rust structs. It:
/// 1. Walks the object heap (heap 24) and unpacks all objects
/// 2. For each object with subobjects, unpacks the subobject heap
/// 3. Stores everything in InterpretedScore for efficient access
///
/// **Critical**: The object heap (type 24) stores objects in **variable-length**
/// format in the file. Each object uses only its type-specific byte count
/// (from N105_OBJ_SIZES), NOT the uniform obj_size stride. The C++ reader
/// calls MoveObjSubobjs() to expand objects to uniform slots after reading.
/// We replicate this by walking the packed data and assigning sequential
/// 1-based indices.
///
/// Source: HeapFileIO.cp ReadObjHeap() (line 973), WriteObject() (line 659)
pub fn interpret_heap(ngl: &NglFile) -> Result<InterpretedScore, String> {
    let mut score = InterpretedScore::new();

    // Populate version from the original NGL file to preserve format
    score.version = ngl.version;

    // Parse head_l from score header — equivalent to OG doc->headL.
    // This is the first field of ScoreHeader (2 bytes, big-endian u16).
    // Reference: NObjTypesN105.h DOCUMENTHDR, ScoreHeader.head_l
    if ngl.score_header_raw.len() >= 2 {
        score.head_l = u16::from_be_bytes([ngl.score_header_raw[0], ngl.score_header_raw[1]]);
    }

    // === Parse score header fields for part names, measure numbers, etc. ===
    // Use the full ScoreHeader parser from doc_types which already handles all offsets.
    // The parser requires SCORE_HDR_SIZE_N105 bytes, so only attempt this for N103/N105 files.
    // N101/N102 files have smaller headers (1412 bytes) that don't contain the same fields.
    let score_header_ok = ngl.score_header_raw.len() >= crate::doc_types::SCORE_HDR_SIZE_N105;
    if score_header_ok {
        match crate::doc_types::ScoreHeader::from_n105_bytes(&ngl.score_header_raw) {
            Ok(hdr) => {
                score.first_names = hdr.first_names;
                score.other_names = hdr.other_names;
                score.d_indent_first = hdr.d_indent_first;
                score.d_indent_other = hdr.d_indent_other;
                score.number_meas = hdr.number_meas;
                score.above_mn = hdr.above_mn != 0;
                score.first_mn_number = hdr.first_mn_number;
                score.x_mn_offset = hdr.x_mn_offset;
                score.y_mn_offset = hdr.y_mn_offset;
                score.x_sys_mn_offset = hdr.x_sys_mn_offset;
                score.sys_first_mn = hdr.sys_first_mn != 0;
                score.start_mn_print1 = hdr.start_mn_print1 != 0;

                // === Extract font table (up to 20 entries) ===
                // Each FontItem has a Pascal string font_name[32] (length byte + chars).
                // GRAPHIC objects with info=0 (FONT_THISITEMONLY) use fontInd to index
                // into this table to determine their font.
                // Reference: NDocAndCnfgTypes.h FONTITEM, DrawUtils.cp GetGraphicFontInfo()
                for i in 0..hdr.nfonts_used.max(0) as usize {
                    if i >= hdr.font_table.len() {
                        break;
                    }
                    let fi = &hdr.font_table[i];
                    let name_len = (fi.font_name[0] as usize).min(31);
                    let name =
                        crate::ngl::reader::mac_roman_to_string(&fi.font_name[1..1 + name_len]);
                    score.font_names.push(name);
                }

                // === Extract header/footer fields from score header ===
                // Reference: NDocAndCnfgTypes.h, DrawObject.cp DrawHeaderFooter()
                score.use_header_footer = hdr.use_header_footer != 0;
                score.top_pgn = hdr.top_pgn != 0;
                score.h_pos_pgn = hdr.h_pos_pgn;
                score.alternate_pgn = hdr.alternate_pgn != 0;

                // Decode header/footer template strings from string pool
                if let Some(h) =
                    crate::ngl::reader::decode_string(&ngl.string_pool, hdr.header_str_offset)
                {
                    score.header_str = h;
                }
                if let Some(f) =
                    crate::ngl::reader::decode_string(&ngl.string_pool, hdr.footer_str_offset)
                {
                    score.footer_str = f;
                }

                // Extract PG font info (page header/footer/number font)
                let pg_name_len = (hdr.font_name_pg[0] as usize).min(31);
                if pg_name_len > 0 {
                    score.pg_font_name = crate::ngl::reader::mac_roman_to_string(
                        &hdr.font_name_pg[1..1 + pg_name_len],
                    );
                }
                if hdr.font_size_pg > 0 {
                    score.pg_font_size = hdr.font_size_pg as f32;
                }
                score.pg_font_style = hdr.font_style_pg;
            }
            Err(e) => {
                eprintln!("[interpret_heap] ScoreHeader parse failed: {}", e);
            }
        }
    }

    // === Parse page geometry from document header ===
    // Reference: NDocAndCnfgTypes.h, DOCUMENTHEADER fields
    if let Ok(hdr) = crate::doc_types::DocumentHeader::from_n105_bytes(&ngl.doc_header_raw) {
        let w = (hdr.orig_paper_rect.right - hdr.orig_paper_rect.left) as f32;
        let h = (hdr.orig_paper_rect.bottom - hdr.orig_paper_rect.top) as f32;
        if w > 0.0 {
            score.page_width_pt = w;
        }
        if h > 0.0 {
            score.page_height_pt = h;
        }
        score.first_page_number = hdr.first_page_number;
        score.start_page_number = hdr.start_page_number;

        // Extract header/footer margins
        // Reference: NDocAndCnfgTypes.h doc->headerFooterMargins (Rect: top, left, bottom, right)
        score.hf_margin_top = hdr.header_footer_margins.top as f32;
        score.hf_margin_left = hdr.header_footer_margins.left as f32;
        score.hf_margin_bottom = hdr.header_footer_margins.bottom as f32;
        score.hf_margin_right = hdr.header_footer_margins.right as f32;
    }
    // On Err: defaults already set (612x792, firstPageNumber=1)

    // === Parse text styles from score header ===
    // N105: 15 TEXTSTYLE records at file offset 390 (0x186), each 36 bytes.
    // score_header_raw starts at file offset 80, so offset within raw = 390 - 80 = 310.
    // TEXTSTYLEN105: fontName[32] + bitfield(2 bytes) + fontStyle(2 bytes) = 36 bytes
    // Bitfield: filler2:5 | lyric:1 | enclosure:2 | relFSize:1 | fontSize:7
    // Source: NBasicTypesN105.h lines 53-61, Ngale5ProgQuickRef-TN1.txt
    const TEXT_STYLE_OFFSET: usize = 310; // file offset 390 - 80 (header start)
    const TEXT_STYLE_SIZE: usize = 36;
    const NUM_TEXT_STYLES: usize = 15;
    if ngl.score_header_raw.len() >= TEXT_STYLE_OFFSET + NUM_TEXT_STYLES * TEXT_STYLE_SIZE {
        for i in 0..NUM_TEXT_STYLES {
            let base = TEXT_STYLE_OFFSET + i * TEXT_STYLE_SIZE;
            let style_bytes = &ngl.score_header_raw[base..base + TEXT_STYLE_SIZE];
            // fontName[32]: Pascal string (byte 0 = length, bytes 1..len = chars)
            let name_len = style_bytes[0] as usize;
            let name_len = name_len.min(31); // cap at 31 chars
            let font_name = crate::ngl::reader::mac_roman_to_string(&style_bytes[1..1 + name_len]);
            // Bitfield at offset 32-33 (big-endian u16):
            // bits 15-11: filler2 (5 bits)
            // bit 10: lyric (1 bit)
            // bits 9-8: enclosure (2 bits)
            // bit 7: relFSize (1 bit)
            // bits 6-0: fontSize (7 bits)
            let bf = u16::from_be_bytes([style_bytes[32], style_bytes[33]]);
            let lyric = (bf >> 10) & 1 != 0;
            let enclosure = ((bf >> 8) & 0x03) as u8;
            let rel_f_size = (bf >> 7) & 1 != 0;
            let font_size = (bf & 0x7F) as u8;
            let font_style = i16::from_be_bytes([style_bytes[34], style_bytes[35]]);
            score.text_styles.push(TextStyle {
                font_name,
                rel_f_size,
                font_size,
                font_style,
                lyric,
                enclosure,
            });
        }
    }

    // Get the object heap (type 24)
    let obj_heap = &ngl.heaps[OBJ_TYPE as usize];
    let obj_size = obj_heap.obj_size as usize; // uniform in-memory size (e.g. 46)
    let obj_data = &obj_heap.obj_data;

    // The reader prepends obj_size bytes of zeros for slot 0 (NILINK),
    // then the rest is sizeAllObjsFile bytes of variable-length packed objects.
    // We walk the packed region, reading each object's type byte at offset 10
    // to determine its actual file size from N105_OBJ_SIZES.

    let data_start = obj_size; // skip slot 0 padding
    let data_end = obj_data.len();
    let mut cursor = data_start;
    let mut obj_idx: u16 = 1; // 1-based index matching C++ LINK values

    while cursor < data_end && obj_idx <= obj_heap.obj_count {
        // Need at least 23 bytes for the object header
        if cursor + 23 > data_end {
            break;
        }

        // Read the type byte at offset 10 within the object header
        let obj_type = obj_data[cursor + 10];

        // Look up the actual file size for this object type
        let file_obj_size = if (obj_type as usize) < crate::obj_types::N105_OBJ_SIZES.len() {
            crate::obj_types::N105_OBJ_SIZES[obj_type as usize] as usize
        } else {
            // Invalid type — bail out since data is corrupt
            eprintln!(
                "Warning: Object {} at offset {} has invalid type {}, stopping",
                obj_idx, cursor, obj_type as i8
            );
            break;
        };

        if file_obj_size == 0 {
            // Type 14 (MODNR) has 0 object size — no MODNR objects exist
            eprintln!(
                "Warning: Object {} has zero-length type {}, stopping",
                obj_idx, obj_type
            );
            break;
        }

        if cursor + file_obj_size > data_end {
            eprintln!(
                "Warning: Object {} at offset {} truncated (need {} bytes, have {})",
                obj_idx,
                cursor,
                file_obj_size,
                data_end - cursor
            );
            break;
        }

        // Pad to uniform obj_size for header unpacking (some unpackers check len >= obj_size)
        let mut obj_bytes_padded = vec![0u8; obj_size.max(file_obj_size)];
        obj_bytes_padded[..file_obj_size]
            .copy_from_slice(&obj_data[cursor..cursor + file_obj_size]);
        let obj_bytes = &obj_bytes_padded[..];

        // Unpack the 23-byte object header
        let header = unpack_object_header_n105(obj_bytes)?;

        // Based on obj_type, unpack the type-specific data after byte 23
        let data = match header.obj_type as u8 {
            HEADER_TYPE => ObjData::Header(Header {
                header: header.clone(),
            }),
            TAIL_TYPE => ObjData::Tail(Tail {
                header: header.clone(),
            }),

            SYNC_TYPE => {
                // SYNC_5 layout: OBJECTHEADER_5 (23 bytes) + 1 byte mac68k padding
                // + timeStamp (unsigned short, 2 bytes) = 26 bytes total.
                // timeStamp is at offset 24-25 (not 23-24) due to mac68k alignment
                // padding before the unsigned short.
                let time_stamp = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    0
                };
                ObjData::Sync(Sync {
                    header: header.clone(),
                    time_stamp,
                })
            }

            MEASURE_TYPE => {
                // Measure: 46 bytes total, 23 bytes after header
                let filler_m = if obj_bytes.len() > 23 {
                    obj_bytes[23] as i8
                } else {
                    0
                };
                let l_measure = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_measure = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let system_l = if obj_bytes.len() >= 30 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                let staff_l = if obj_bytes.len() >= 32 {
                    u16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                } else {
                    NILINK
                };
                let fake_meas = if obj_bytes.len() >= 34 {
                    i16::from_be_bytes([obj_bytes[32], obj_bytes[33]])
                } else {
                    0
                };
                let space_percent = if obj_bytes.len() >= 36 {
                    i16::from_be_bytes([obj_bytes[34], obj_bytes[35]])
                } else {
                    100
                };
                let measure_b_box = if obj_bytes.len() >= 44 {
                    Rect {
                        top: i16::from_be_bytes([obj_bytes[36], obj_bytes[37]]),
                        left: i16::from_be_bytes([obj_bytes[38], obj_bytes[39]]),
                        bottom: i16::from_be_bytes([obj_bytes[40], obj_bytes[41]]),
                        right: i16::from_be_bytes([obj_bytes[42], obj_bytes[43]]),
                    }
                } else {
                    Rect {
                        top: 0,
                        left: 0,
                        bottom: 0,
                        right: 0,
                    }
                };
                let l_time_stamp = if obj_bytes.len() >= 48 {
                    i32::from_be_bytes([obj_bytes[44], obj_bytes[45], obj_bytes[46], obj_bytes[47]])
                } else {
                    0
                };
                ObjData::Measure(Measure {
                    header: header.clone(),
                    filler_m,
                    l_measure,
                    r_measure,
                    system_l,
                    staff_l,
                    fake_meas,
                    space_percent,
                    measure_b_box,
                    l_time_stamp,
                })
            }

            STAFF_TYPE => {
                let l_staff = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_staff = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let system_l = if obj_bytes.len() >= 30 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                ObjData::Staff(Staff {
                    header: header.clone(),
                    l_staff,
                    r_staff,
                    system_l,
                })
            }

            SYSTEM_TYPE => {
                let l_system = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_system = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let page_l = if obj_bytes.len() >= 30 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                let system_num = if obj_bytes.len() >= 32 {
                    i16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                } else {
                    0
                };
                let system_rect = if obj_bytes.len() >= 40 {
                    DRect {
                        top: i16::from_be_bytes([obj_bytes[32], obj_bytes[33]]),
                        left: i16::from_be_bytes([obj_bytes[34], obj_bytes[35]]),
                        bottom: i16::from_be_bytes([obj_bytes[36], obj_bytes[37]]),
                        right: i16::from_be_bytes([obj_bytes[38], obj_bytes[39]]),
                    }
                } else {
                    DRect {
                        top: 0,
                        left: 0,
                        bottom: 0,
                        right: 0,
                    }
                };
                ObjData::System(System {
                    header: header.clone(),
                    l_system,
                    r_system,
                    page_l,
                    system_num,
                    system_rect,
                    sys_desc_ptr: 0,
                })
            }

            PAGE_TYPE => {
                let l_page = if obj_bytes.len() >= 26 {
                    u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                } else {
                    NILINK
                };
                let r_page = if obj_bytes.len() >= 28 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let sheet_num = if obj_bytes.len() >= 30 {
                    i16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    0
                };
                let header_str_offset = if obj_bytes.len() >= 34 {
                    i32::from_be_bytes([obj_bytes[30], obj_bytes[31], obj_bytes[32], obj_bytes[33]])
                } else {
                    0
                };
                let footer_str_offset = if obj_bytes.len() >= 38 {
                    i32::from_be_bytes([obj_bytes[34], obj_bytes[35], obj_bytes[36], obj_bytes[37]])
                } else {
                    0
                };
                ObjData::Page(Page {
                    header: header.clone(),
                    l_page,
                    r_page,
                    sheet_num,
                    header_str_offset,
                    footer_str_offset,
                })
            }

            CLEF_TYPE => {
                let in_measure = if obj_bytes.len() > 23 {
                    obj_bytes[23] != 0
                } else {
                    false
                };
                ObjData::Clef(Clef {
                    header: header.clone(),
                    in_measure,
                })
            }

            KEYSIG_TYPE => {
                let in_measure = if obj_bytes.len() > 23 {
                    obj_bytes[23] != 0
                } else {
                    false
                };
                ObjData::KeySig(KeySig {
                    header: header.clone(),
                    in_measure,
                })
            }

            TIMESIG_TYPE => {
                let in_measure = if obj_bytes.len() > 23 {
                    obj_bytes[23] != 0
                } else {
                    false
                };
                ObjData::TimeSig(TimeSig {
                    header: header.clone(),
                    in_measure,
                })
            }

            BEAMSET_TYPE | SLUR_TYPE | TUPLET_TYPE | GRAPHIC_TYPE | OTTAVA_TYPE | SPACER_TYPE
            | ENDING_TYPE | TEMPO_TYPE => {
                // These all have ExtObjHeader (staffn byte at offset 23)
                // For now, create minimal objects - full implementation can be added later
                match header.obj_type as u8 {
                    BEAMSET_TYPE => {
                        // N105 BEAMSET_5 object: 26 bytes total
                        // 0-22:  OBJECTHEADER_5 (23 bytes)
                        // 23:    staffn (EXTOBJHEADER)
                        // 24:    voice (SignedByte)
                        // 25:    bitfield byte: thin:1|beamRests:1|feather:2|grace:1|firstSystem:1|crossStaff:1|crossSystem:1
                        // Source: NObjTypesN105.h lines 307-318
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };

                        // Extract bitfield byte at offset 25 if present
                        let bitfield = if obj_bytes.len() > 25 {
                            obj_bytes[25]
                        } else {
                            0
                        };

                        ObjData::BeamSet(BeamSet {
                            header: header.clone(),
                            ext_header,
                            voice: if obj_bytes.len() > 24 {
                                obj_bytes[24] as i8
                            } else {
                                1
                            },
                            thin: bitfield & 0x01,                // bit 0
                            beam_rests: (bitfield >> 1) & 0x01,   // bit 1
                            feather: (bitfield >> 2) & 0x03,      // bits 2-3 (2 bits)
                            grace: (bitfield >> 4) & 0x01,        // bit 4
                            first_system: (bitfield >> 5) & 0x01, // bit 5
                            cross_staff: (bitfield >> 6) & 0x01,  // bit 6
                            cross_system: (bitfield >> 7) & 0x01, // bit 7
                        })
                    }
                    SLUR_TYPE => {
                        // N105 SLUR_5 object: 30 bytes total
                        // 0-22:  OBJECTHEADER_5 (23 bytes)
                        // 23:    staffn (EXTOBJHEADER)
                        // 24:    voice (SignedByte)
                        // 25:    bitfield: filler:2|crossStaff:1|crossStfBack:1|crossSystem:1|tempFlag:1|used:1|tie:1
                        // 26-27: firstSyncL (LINK)
                        // 28-29: lastSyncL (LINK)
                        // Source: NObjTypesN105.h lines 472-485
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        let voice = if obj_bytes.len() > 24 {
                            obj_bytes[24] as i8
                        } else {
                            1
                        };
                        // Byte 25: bitfield (MSB-first on PowerPC)
                        let b25 = if obj_bytes.len() > 25 {
                            obj_bytes[25]
                        } else {
                            0
                        };
                        let cross_staff = (b25 >> 5) & 1;
                        let cross_stf_back = (b25 >> 4) & 1;
                        let cross_system = (b25 >> 3) & 1;
                        let temp_flag = (b25 >> 2) & 1 != 0;
                        let used = (b25 >> 1) & 1 != 0;
                        let tie = b25 & 1 != 0;
                        let first_sync_l = if obj_bytes.len() >= 28 {
                            u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                        } else {
                            NILINK
                        };
                        let last_sync_l = if obj_bytes.len() >= 30 {
                            u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                        } else {
                            NILINK
                        };
                        ObjData::Slur(Slur {
                            header: header.clone(),
                            ext_header,
                            voice,
                            philler: 0,
                            cross_staff,
                            cross_stf_back,
                            cross_system,
                            temp_flag,
                            used,
                            tie,
                            first_sync_l,
                            last_sync_l,
                        })
                    }
                    TUPLET_TYPE => {
                        // N105 TUPLET object: 40 bytes total
                        // 0-22: OBJECTHEADER_5 (23 bytes)
                        // 23:   staffn (EXTOBJHEADER)
                        // 24:   accNum
                        // 25:   accDenom
                        // 26:   voice (SignedByte)
                        // 27:   numVis
                        // 28:   denomVis
                        // 29:   brackVis
                        // 30:   small
                        // 31:   filler
                        // 32-33: xdFirst (DDIST)
                        // 34-35: ydFirst (DDIST)
                        // 36-37: xdLast (DDIST)
                        // 38-39: ydLast (DDIST)
                        // Note: acnxd/acnyd ("now unused") are NOT in N105 disk format
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        let acc_num = if obj_bytes.len() > 24 {
                            obj_bytes[24]
                        } else {
                            3
                        };
                        let acc_denom = if obj_bytes.len() > 25 {
                            obj_bytes[25]
                        } else {
                            2
                        };
                        let voice = if obj_bytes.len() > 26 {
                            obj_bytes[26] as i8
                        } else {
                            1
                        };
                        let num_vis = if obj_bytes.len() > 27 {
                            obj_bytes[27]
                        } else {
                            1
                        };
                        let denom_vis = if obj_bytes.len() > 28 {
                            obj_bytes[28]
                        } else {
                            0
                        };
                        let brack_vis = if obj_bytes.len() > 29 {
                            obj_bytes[29]
                        } else {
                            1
                        };
                        let small = if obj_bytes.len() > 30 {
                            obj_bytes[30]
                        } else {
                            0
                        };
                        let filler = if obj_bytes.len() > 31 {
                            obj_bytes[31]
                        } else {
                            0
                        };
                        let xd_first = if obj_bytes.len() > 33 {
                            i16::from_be_bytes([obj_bytes[32], obj_bytes[33]])
                        } else {
                            0
                        };
                        let yd_first = if obj_bytes.len() > 35 {
                            i16::from_be_bytes([obj_bytes[34], obj_bytes[35]])
                        } else {
                            0
                        };
                        let xd_last = if obj_bytes.len() > 37 {
                            i16::from_be_bytes([obj_bytes[36], obj_bytes[37]])
                        } else {
                            0
                        };
                        let yd_last = if obj_bytes.len() > 39 {
                            i16::from_be_bytes([obj_bytes[38], obj_bytes[39]])
                        } else {
                            0
                        };
                        ObjData::Tuplet(crate::obj_types::Tuplet {
                            header: header.clone(),
                            ext_header,
                            acc_num,
                            acc_denom,
                            voice,
                            num_vis,
                            denom_vis,
                            brack_vis,
                            small,
                            filler,
                            acnxd: 0, // not stored in N105
                            acnyd: 0, // not stored in N105
                            xd_first,
                            yd_first,
                            xd_last,
                            yd_last,
                        })
                    }
                    GRAPHIC_TYPE => {
                        // N105 GRAPHIC_5 object: 44 bytes total
                        // 0-22:  OBJECTHEADER_5 (23 bytes)
                        // 23:    staffn (EXTOBJHEADER)
                        // 24:    graphicType (SignedByte)
                        // 25:    voice (SignedByte)
                        // 26:    bitfield: enclosure:2|justify:3|vConstrain:1|hConstrain:1|multiLine:1
                        // 27:    [mac68k padding]
                        // 28-29: info (short) — text style index for GRString/GRLyric
                        // 30-33: gu union (4 bytes: Handle/thickness)
                        // 34:    fontInd (SignedByte)
                        // 35:    relFSize:1|fontSize:7
                        // 36-37: fontStyle (short)
                        // 38-39: info2 (short)
                        // 40-41: firstObj (LINK)
                        // 42-43: lastObj (LINK)
                        // Source: NObjTypesN105.h lines 401-425
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                0 // NB: staffn can be 0 for GRAPHICs
                            },
                        };
                        let graphic_type = if obj_bytes.len() > 24 {
                            obj_bytes[24] as i8
                        } else {
                            3 // default: GRString
                        };
                        let voice = if obj_bytes.len() > 25 {
                            obj_bytes[25] as i8
                        } else {
                            1
                        };
                        let b26 = if obj_bytes.len() > 26 {
                            obj_bytes[26]
                        } else {
                            0
                        };
                        let enclosure = (b26 >> 6) & 0x03;
                        let justify = (b26 >> 3) & 0x07;
                        let v_constrain = (b26 >> 2) & 1 != 0;
                        let h_constrain = (b26 >> 1) & 1 != 0;
                        let multi_line = b26 & 1;
                        // Byte 27 is padding
                        let info = if obj_bytes.len() > 29 {
                            i16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                        } else {
                            0
                        };
                        let gu_thickness = if obj_bytes.len() > 33 {
                            i16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                        } else {
                            0
                        };
                        let font_ind = if obj_bytes.len() > 34 {
                            obj_bytes[34] as i8
                        } else {
                            0
                        };
                        let b35 = if obj_bytes.len() > 35 {
                            obj_bytes[35]
                        } else {
                            0
                        };
                        let rel_f_size = (b35 >> 7) & 1;
                        let font_size = b35 & 0x7F;
                        let font_style = if obj_bytes.len() > 37 {
                            i16::from_be_bytes([obj_bytes[36], obj_bytes[37]])
                        } else {
                            0
                        };
                        let info2 = if obj_bytes.len() > 39 {
                            i16::from_be_bytes([obj_bytes[38], obj_bytes[39]])
                        } else {
                            0
                        };
                        let first_obj = if obj_bytes.len() > 41 {
                            u16::from_be_bytes([obj_bytes[40], obj_bytes[41]])
                        } else {
                            NILINK
                        };
                        let last_obj = if obj_bytes.len() > 43 {
                            u16::from_be_bytes([obj_bytes[42], obj_bytes[43]])
                        } else {
                            NILINK
                        };
                        ObjData::Graphic(Graphic {
                            header: header.clone(),
                            ext_header,
                            graphic_type,
                            voice,
                            enclosure,
                            justify,
                            v_constrain,
                            h_constrain,
                            multi_line,
                            info,
                            gu_handle: 0, // Not meaningful in modern context
                            gu_thickness,
                            font_ind,
                            rel_f_size,
                            font_size,
                            font_style,
                            info2,
                            first_obj,
                            last_obj,
                        })
                    }
                    TEMPO_TYPE => {
                        // N105 TEMPO_5 object: 38 bytes total
                        // 0-22:  OBJECTHEADER_5 (23 bytes)
                        // 23:    staffn (EXTOBJHEADER)
                        // 24:    subType (SignedByte) — beat duration code (same as l_dur)
                        // 25:    bitfield: expanded:1|noMM:1|filler:4|dotted:1|hideMM:1 (MSB-first)
                        // 26-27: tempoMM (short) — BPM
                        // 28-31: strOffset (STRINGOFFSET/long) — verbal tempo string
                        // 32-33: firstObjL (LINK) — object tempo is attached to
                        // 34-37: metroStrOffset (STRINGOFFSET/long) — metronome number string
                        // Source: NObjTypesN105.h lines 519-532
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        let sub_type = if obj_bytes.len() > 24 {
                            obj_bytes[24] as i8
                        } else {
                            4 // QTR_L_DUR default
                        };
                        let b25 = if obj_bytes.len() > 25 {
                            obj_bytes[25]
                        } else {
                            0
                        };
                        // MSB-first bitfield: expanded:1|noMM:1|filler:4|dotted:1|hideMM:1
                        let expanded = (b25 >> 7) & 1 != 0;
                        let no_mm = (b25 >> 6) & 1 != 0;
                        let filler = (b25 >> 2) & 0x0F;
                        let dotted = (b25 >> 1) & 1 != 0;
                        let hide_mm = b25 & 1 != 0;
                        let tempo_mm = if obj_bytes.len() > 27 {
                            i16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                        } else {
                            120
                        };
                        let str_offset = if obj_bytes.len() > 31 {
                            i32::from_be_bytes([
                                obj_bytes[28],
                                obj_bytes[29],
                                obj_bytes[30],
                                obj_bytes[31],
                            ])
                        } else {
                            0
                        };
                        let first_obj_l = if obj_bytes.len() > 33 {
                            u16::from_be_bytes([obj_bytes[32], obj_bytes[33]])
                        } else {
                            NILINK
                        };
                        let metro_str_offset = if obj_bytes.len() > 37 {
                            i32::from_be_bytes([
                                obj_bytes[34],
                                obj_bytes[35],
                                obj_bytes[36],
                                obj_bytes[37],
                            ])
                        } else {
                            0
                        };
                        ObjData::Tempo(Tempo {
                            header: header.clone(),
                            ext_header,
                            sub_type,
                            expanded,
                            no_mm,
                            filler,
                            dotted,
                            hide_mm,
                            tempo_mm,
                            str_offset,
                            first_obj_l,
                            metro_str_offset,
                        })
                    }
                    ENDING_TYPE => {
                        // N105 ENDING_5 object: 32 bytes total
                        // 0-22:  OBJECTHEADER_5 (23 bytes)
                        // 23:    staffn (EXTOBJHEADER)
                        // 24-25: firstObjL (LINK) — left end attachment
                        // 26-27: lastObjL (LINK) — right end attachment or NILINK
                        // 28:    bitfield: noLCutoff:1|noRCutoff:1|endNum:6 (MSB-first)
                        // 29:    [mac68k padding]
                        // 30-31: endxd (DDIST) — position offset from lastObjL
                        // Source: NObjTypesN105.h lines 547-556
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        let first_obj_l = if obj_bytes.len() > 25 {
                            u16::from_be_bytes([obj_bytes[24], obj_bytes[25]])
                        } else {
                            NILINK
                        };
                        let last_obj_l = if obj_bytes.len() > 27 {
                            u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                        } else {
                            NILINK
                        };
                        let b28 = if obj_bytes.len() > 28 {
                            obj_bytes[28]
                        } else {
                            0
                        };
                        let no_l_cutoff = (b28 >> 7) & 1;
                        let no_r_cutoff = (b28 >> 6) & 1;
                        let end_num = b28 & 0x3F;
                        // Byte 29 is mac68k padding
                        let endxd = if obj_bytes.len() > 31 {
                            i16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                        } else {
                            0
                        };
                        ObjData::Ending(Ending {
                            header: header.clone(),
                            ext_header,
                            first_obj_l,
                            last_obj_l,
                            no_l_cutoff,
                            no_r_cutoff,
                            end_num,
                            endxd,
                        })
                    }
                    OTTAVA_TYPE => {
                        // N105 OTTAVA_5 object: 40 bytes total
                        // 0-22:  OBJECTHEADER_5 (23 bytes)
                        // 23:    staffn (EXTOBJHEADER)
                        // 24:    bitfield: noCutoff:1|crossStaff:1|crossSystem:1|octSignType:5 (MSB-first)
                        // 25:    filler (SignedByte)
                        // 26:    bitfield2: numberVis:1|unused1:1|brackVis:1|unused2:5 (MSB-first)
                        // 27:    [mac68k padding — aligns DDIST to even offset]
                        // 28-29: nxd (DDIST)
                        // 30-31: nyd (DDIST)
                        // 32-33: xdFirst (DDIST)
                        // 34-35: ydFirst (DDIST)
                        // 36-37: xdLast (DDIST)
                        // 38-39: ydLast (DDIST)
                        // Source: NObjTypesN105.h lines 436-451
                        let ext_header = crate::obj_types::ExtObjHeader {
                            staffn: if obj_bytes.len() > 23 {
                                obj_bytes[23] as i8
                            } else {
                                1
                            },
                        };
                        let b24 = if obj_bytes.len() > 24 {
                            obj_bytes[24]
                        } else {
                            0
                        };
                        let no_cutoff = (b24 >> 7) & 1;
                        let cross_staff = (b24 >> 6) & 1;
                        let cross_system = (b24 >> 5) & 1;
                        let oct_sign_type = b24 & 0x1F;
                        let filler = if obj_bytes.len() > 25 {
                            obj_bytes[25] as i8
                        } else {
                            0
                        };
                        let b26 = if obj_bytes.len() > 26 {
                            obj_bytes[26]
                        } else {
                            0
                        };
                        let number_vis = (b26 >> 7) & 1 != 0;
                        let brack_vis = (b26 >> 5) & 1 != 0;
                        // Byte 27 is mac68k padding
                        let nxd = if obj_bytes.len() > 29 {
                            i16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                        } else {
                            0
                        };
                        let nyd = if obj_bytes.len() > 31 {
                            i16::from_be_bytes([obj_bytes[30], obj_bytes[31]])
                        } else {
                            0
                        };
                        let xd_first = if obj_bytes.len() > 33 {
                            i16::from_be_bytes([obj_bytes[32], obj_bytes[33]])
                        } else {
                            0
                        };
                        let yd_first = if obj_bytes.len() > 35 {
                            i16::from_be_bytes([obj_bytes[34], obj_bytes[35]])
                        } else {
                            0
                        };
                        let xd_last = if obj_bytes.len() > 37 {
                            i16::from_be_bytes([obj_bytes[36], obj_bytes[37]])
                        } else {
                            0
                        };
                        let yd_last = if obj_bytes.len() > 39 {
                            i16::from_be_bytes([obj_bytes[38], obj_bytes[39]])
                        } else {
                            0
                        };
                        ObjData::Ottava(Ottava {
                            header: header.clone(),
                            ext_header,
                            no_cutoff,
                            cross_staff,
                            cross_system,
                            oct_sign_type,
                            filler,
                            number_vis,
                            unused1: false,
                            brack_vis,
                            unused2: false,
                            nxd,
                            nyd,
                            xd_first,
                            yd_first,
                            xd_last,
                            yd_last,
                        })
                    }
                    _ => ObjData::GrSync(GrSync {
                        header: header.clone(),
                    }),
                }
            }

            GRSYNC_TYPE => ObjData::GrSync(GrSync {
                header: header.clone(),
            }),
            PSMEAS_TYPE => ObjData::PsMeas(PsMeas {
                header: header.clone(),
                filler: 0,
            }),

            DYNAMIC_TYPE => {
                // N105 DYNAMIC_5 object: 30 bytes on disk (mac68k padded)
                // 0-22:  OBJECTHEADER_5 (23 bytes)
                // 23:    dynamicType (SignedByte) — 1-21=text, 22=dim hairpin, 23=cresc hairpin
                // 24:    filler:7 | crossSys:1 (bitfield)
                // 25:    [mac68k padding — aligns LINK to even offset]
                // 26-27: firstSyncL (LINK) — sync the dynamic/hairpin start is attached to
                // 28-29: lastSyncL (LINK) — sync hairpin end is attached to, or NILINK
                // Total: 30 bytes (N105_OBJ_SIZES[13])
                // Source: NObjTypesN105.h lines 370-377, DrawObject.cp:1226-1324
                let dynamic_type = if obj_bytes.len() > 23 {
                    obj_bytes[23] as i8
                } else {
                    0
                };
                let b24 = if obj_bytes.len() > 24 {
                    obj_bytes[24]
                } else {
                    0
                };
                let cross_sys = (b24 & 0x01) != 0;
                // Byte 25 is mac68k padding (LINK must be at even offset)
                let first_sync_l = if obj_bytes.len() > 27 {
                    u16::from_be_bytes([obj_bytes[26], obj_bytes[27]])
                } else {
                    NILINK
                };
                let last_sync_l = if obj_bytes.len() > 29 {
                    u16::from_be_bytes([obj_bytes[28], obj_bytes[29]])
                } else {
                    NILINK
                };
                ObjData::Dynamic(Dynamic {
                    header: header.clone(),
                    dynamic_type,
                    filler: false,
                    cross_sys,
                    first_sync_l,
                    last_sync_l,
                })
            }

            RPTEND_TYPE => {
                // Unpack RPTEND main object (8 bytes after OBJECTHEADER)
                // Source: NObjTypes.h lines 147-157, NObjTypesN105.h lines 445-454
                // Layout: first_obj(2) | start_rpt(2) | end_rpt(2) | sub_type(1) | count(1)
                let first_obj = if obj_data.len() >= 2 {
                    u16::from_be_bytes([obj_data[0], obj_data[1]])
                } else {
                    NILINK
                };
                let start_rpt = if obj_data.len() >= 4 {
                    u16::from_be_bytes([obj_data[2], obj_data[3]])
                } else {
                    NILINK
                };
                let end_rpt = if obj_data.len() >= 6 {
                    u16::from_be_bytes([obj_data[4], obj_data[5]])
                } else {
                    NILINK
                };
                let sub_type = if obj_data.len() >= 7 {
                    obj_data[6] as i8
                } else {
                    0
                };
                let count = if obj_data.len() >= 8 { obj_data[7] } else { 0 };
                ObjData::RptEnd(RptEnd {
                    header: header.clone(),
                    first_obj,
                    start_rpt,
                    end_rpt,
                    sub_type,
                    count,
                })
            }

            CONNECT_TYPE => {
                // Connect (brace/bracket) — connFiller is after OBJECTHEADER
                // Source: NObjTypesN105.h lines 351-354
                let conn_filler = if obj_data.len() >= 2 {
                    u16::from_be_bytes([obj_data[0], obj_data[1]])
                } else {
                    0
                };
                ObjData::Connect(Connect {
                    header: header.clone(),
                    conn_filler,
                })
            }

            _ => {
                // Should not happen — we already validated the type above
                eprintln!(
                    "Warning: Skipping object {} with unhandled type: {}",
                    obj_idx, header.obj_type
                );
                // Still advance past this object (we know its size from the type lookup)
                cursor += file_obj_size;
                obj_idx += 1;
                continue;
            }
        };

        // Resolve TEMPO strings from the string pool immediately.
        // TEMPO objects have no subobjects, so we do this in the main object loop.
        // Source: DrawObject.cp:2320-2346 (DrawTEMPO string handling)
        if let ObjData::Tempo(ref tempo) = data {
            let verbal =
                reader_decode_string(&ngl.string_pool, tempo.str_offset).unwrap_or_default();
            let metro =
                reader_decode_string(&ngl.string_pool, tempo.metro_str_offset).unwrap_or_default();
            score.tempo_strings.insert(obj_idx, (verbal, metro));
        }

        score.objects.push(InterpretedObject {
            index: obj_idx,
            header,
            data,
        });

        // Advance cursor past this variable-length object
        cursor += file_obj_size;
        obj_idx += 1;
    }

    // Now unpack subobject heaps for objects that have subobjects
    for obj in &score.objects {
        if obj.header.first_sub_obj == NILINK || obj.header.first_sub_obj == 0 {
            continue;
        }

        let heap_type = obj.header.obj_type as usize;
        if heap_type >= ngl.heaps.len() {
            continue;
        }

        let subobj_heap = &ngl.heaps[heap_type];
        if subobj_heap.obj_count == 0 {
            continue;
        }

        let sub_size = subobj_heap.obj_size as usize;
        let sub_data = &subobj_heap.obj_data;

        // Unpack subobjects based on type
        match obj.header.obj_type as u8 {
            HEADER_TYPE => {
                // Unpack PARTINFO subobjects from the HEADER heap.
                // Only process from the first HEADER (main score), not the master page.
                if score.part_infos.is_empty() {
                    let n_entries = obj.header.n_entries as usize;
                    for i in 0..n_entries {
                        let sub_idx = (obj.header.first_sub_obj as usize) + i;
                        let offset = sub_idx * sub_size;
                        if offset + sub_size <= sub_data.len() {
                            if let Ok(pi) = unpack_partinfo(&sub_data[offset..offset + sub_size]) {
                                score.part_infos.push(pi);
                            }
                        }
                    }
                }
            }

            SYNC_TYPE | GRSYNC_TYPE => {
                // Unpack ANOTE subobjects
                let mut notes = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(note) = unpack_anote_n105(&sub_data[offset..offset + sub_size]) {
                            notes.push(note);
                        }
                    }
                }
                if !notes.is_empty() {
                    if obj.header.obj_type as u8 == SYNC_TYPE {
                        score.notes.insert(obj.header.first_sub_obj, notes);
                    } else {
                        score.grnotes.insert(obj.header.first_sub_obj, notes);
                    }
                }
            }

            STAFF_TYPE => {
                // Unpack ASTAFF subobjects
                let mut staffs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(mut staff) =
                            unpack_astaff_n105(&sub_data[offset..offset + sub_size])
                        {
                            // FIXME: N101/N102 files may have show_lines=0 due to struct layout differences.
                            // Default to SHOW_ALL_LINES for legacy formats since showLines field might not
                            // be properly set or might be at a different offset in older file versions.
                            if (ngl.version == crate::ngl::reader::NglVersion::N101
                                || ngl.version == crate::ngl::reader::NglVersion::N102)
                                && staff.show_lines == 0
                            {
                                staff.show_lines = crate::obj_types::SHOW_ALL_LINES;
                            }
                            staffs.push(staff);
                        }
                    }
                }
                if !staffs.is_empty() {
                    score.staffs.insert(obj.header.first_sub_obj, staffs);
                }
            }

            MEASURE_TYPE => {
                // Unpack AMEASURE subobjects
                let mut measures = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(meas) = unpack_ameasure_n105(&sub_data[offset..offset + sub_size])
                        {
                            measures.push(meas);
                        }
                    }
                }
                if !measures.is_empty() {
                    score.measures.insert(obj.header.first_sub_obj, measures);
                }
            }

            CLEF_TYPE => {
                // Unpack ACLEF subobjects
                let mut clefs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(clef) = unpack_aclef_n105(&sub_data[offset..offset + sub_size]) {
                            clefs.push(clef);
                        }
                    }
                }
                if !clefs.is_empty() {
                    score.clefs.insert(obj.header.first_sub_obj, clefs);
                }
            }

            KEYSIG_TYPE => {
                // Unpack AKEYSIG subobjects
                let mut keysigs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(ks) = unpack_akeysig_n105(&sub_data[offset..offset + sub_size]) {
                            keysigs.push(ks);
                        }
                    }
                }
                if !keysigs.is_empty() {
                    score.keysigs.insert(obj.header.first_sub_obj, keysigs);
                }
            }

            TIMESIG_TYPE => {
                // Unpack ATIMESIG subobjects
                let mut timesigs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(ts) = unpack_atimesig_n105(&sub_data[offset..offset + sub_size]) {
                            timesigs.push(ts);
                        }
                    }
                }
                if !timesigs.is_empty() {
                    score.timesigs.insert(obj.header.first_sub_obj, timesigs);
                }
            }

            RPTEND_TYPE => {
                // Unpack ARPTEND subobjects (8 bytes each)
                // Source: NObjTypes.h lines 142-147
                let mut rptends = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(rptend) =
                            unpack_arptend_n105(&sub_data[offset..offset + sub_size])
                        {
                            rptends.push(rptend);
                        }
                    }
                }
                if !rptends.is_empty() {
                    score.rptend_subs.insert(obj.header.first_sub_obj, rptends);
                }
            }

            PSMEAS_TYPE => {
                // Unpack APSMEAS subobjects (8 bytes each, same layout as ARPTEND)
                // Source: NObjTypesN105.h APSMEAS_5
                let mut psmeas_items = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(psm) = unpack_apsmeas_n105(&sub_data[offset..offset + sub_size]) {
                            psmeas_items.push(psm);
                        }
                    }
                }
                if !psmeas_items.is_empty() {
                    score
                        .psmeas_subs
                        .insert(obj.header.first_sub_obj, psmeas_items);
                }
            }

            BEAMSET_TYPE => {
                // Unpack ANOTEBEAM subobjects
                let mut notebeams = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(beam) =
                            unpack_anotebeam_n105(&sub_data[offset..offset + sub_size])
                        {
                            notebeams.push(beam);
                        }
                    }
                }
                if !notebeams.is_empty() {
                    score.notebeams.insert(obj.header.first_sub_obj, notebeams);
                }
            }

            SLUR_TYPE => {
                // Unpack ASLUR subobjects
                let mut slurs = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(slur) = unpack_aslur_n105(&sub_data[offset..offset + sub_size]) {
                            slurs.push(slur);
                        }
                    }
                }
                if !slurs.is_empty() {
                    score.slurs.insert(obj.header.first_sub_obj, slurs);
                }
            }

            TUPLET_TYPE => {
                // Unpack ANOTETUPLE subobjects (4 bytes each: next + tpSync)
                let mut notetuples = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(nt) = unpack_anotetuple_n105(&sub_data[offset..offset + sub_size])
                        {
                            notetuples.push(nt);
                        }
                    }
                }
                if !notetuples.is_empty() {
                    score.tuplets.insert(obj.header.first_sub_obj, notetuples);
                }
            }

            DYNAMIC_TYPE => {
                // Unpack ADYNAMIC subobjects (14 bytes each)
                // Source: NObjTypesN105.h lines 359-368
                let mut dynamics = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(dyn_sub) =
                            unpack_adynamic_n105(&sub_data[offset..offset + sub_size])
                        {
                            dynamics.push(dyn_sub);
                        }
                    }
                }
                if !dynamics.is_empty() {
                    score.dynamics.insert(obj.header.first_sub_obj, dynamics);
                }
            }

            GRAPHIC_TYPE => {
                // Unpack AGRAPHIC subobjects (6 bytes each: next + strOffset)
                // Each GRAPHIC object has exactly 1 subobject.
                // Source: NObjTypesN105.h lines 396-399
                let mut graphics = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(gr) = unpack_agraphic_n105(&sub_data[offset..offset + sub_size]) {
                            graphics.push(gr);
                        }
                    }
                }
                // Resolve string from string pool for the first (and usually only) subobject
                if let Some(first_gr) = graphics.first() {
                    if let Some(text) = reader_decode_string(&ngl.string_pool, first_gr.str_offset)
                    {
                        if !text.is_empty() {
                            score.graphic_strings.insert(obj.header.first_sub_obj, text);
                        }
                    }
                }
                if !graphics.is_empty() {
                    score.graphics.insert(obj.header.first_sub_obj, graphics);
                }
            }

            OTTAVA_TYPE => {
                // Unpack ANOTEOTTAVA subobjects (4 bytes each: next + opSync)
                // Source: NObjTypesN105.h lines 431-434
                let mut noteottavas = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(no) =
                            unpack_anoteottava_n105(&sub_data[offset..offset + sub_size])
                        {
                            noteottavas.push(no);
                        }
                    }
                }
                if !noteottavas.is_empty() {
                    score.ottavas.insert(obj.header.first_sub_obj, noteottavas);
                }
            }

            CONNECT_TYPE => {
                // Unpack ACONNECT subobjects (12 bytes each)
                // Source: NObjTypesN105.h lines 338-349
                let mut connects = Vec::new();
                let n_entries = obj.header.n_entries as usize;
                for i in 0..n_entries {
                    let sub_idx = (obj.header.first_sub_obj as usize) + i;
                    let offset = sub_idx * sub_size;
                    if offset + sub_size <= sub_data.len() {
                        if let Ok(conn) = unpack_aconnect_n105(&sub_data[offset..offset + sub_size])
                        {
                            connects.push(conn);
                        }
                    }
                }
                if !connects.is_empty() {
                    score.connects.insert(obj.header.first_sub_obj, connects);
                }
            }

            _ => {
                // Other subobject types not yet implemented
            }
        }
    }

    // === Pass 2: Unpack MODNR subobjects ===
    // MODNRs live in heap type 14 (MODNR_TYPE) but are linked from individual
    // ANOTE subobjects via first_mod (not from MODNR main objects).
    // We walk each note's first_mod chain and unpack from the MODNR heap.
    // Source: NObjTypes.h line 107, DrawNRGR.cp DrawModNR() lines 195-245
    let modnr_heap_idx = MODNR_TYPE as usize;
    if modnr_heap_idx < ngl.heaps.len() && ngl.heaps[modnr_heap_idx].obj_count > 0 {
        let modnr_heap = &ngl.heaps[modnr_heap_idx];
        let modnr_size = modnr_heap.obj_size as usize;
        let modnr_data = &modnr_heap.obj_data;

        // Iterate over all unpacked notes, follow first_mod chains
        for notes_vec in score.notes.values() {
            for anote in notes_vec {
                if anote.first_mod == NILINK || anote.first_mod == 0 {
                    continue;
                }

                let mut mods = Vec::new();
                let mut mod_link = anote.first_mod;
                let mut safety = 0;

                while mod_link != NILINK && mod_link != 0 && safety < 100 {
                    safety += 1;
                    let offset = (mod_link as usize) * modnr_size;
                    if offset + modnr_size > modnr_data.len() {
                        break;
                    }
                    match unpack_amodnr_n105(&modnr_data[offset..offset + modnr_size]) {
                        Ok(modnr) => {
                            let next = modnr.next;
                            mods.push(modnr);
                            mod_link = next;
                        }
                        Err(_) => break,
                    }
                }

                if !mods.is_empty() {
                    score.modnrs.insert(anote.first_mod, mods);
                }
            }
        }
    }

    Ok(score)
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpack_object_header_basic() {
        // Minimal valid header: 23 bytes
        let data = vec![
            0x00, 0x02, // right = 2
            0x00, 0x01, // left = 1
            0x00, 0x03, // firstSubObj = 3
            0x00, 0x10, // xd = 16
            0x00, 0x20, // yd = 32
            0x02, // type = SYNC
            0xE0, // flags: selected=1, visible=1, soft=1, valid=0, tweaked=0, spare=0, filler=0
            0x00, 0x00, 0x01, 0x00, // objRect.top=0, left=256
            0x02, 0x00, 0x03, 0x00, // objRect.bottom=512, right=768
            0x00, // relSize = 0
            0x00, // filler = 0
            0x05, // nEntries = 5
        ];

        let hdr = unpack_object_header_n105(&data).unwrap();
        assert_eq!(hdr.right, 2);
        assert_eq!(hdr.left, 1);
        assert_eq!(hdr.first_sub_obj, 3);
        assert_eq!(hdr.xd, 16);
        assert_eq!(hdr.yd, 32);
        assert_eq!(hdr.obj_type, 2);
        assert!(hdr.selected);
        assert!(hdr.visible);
        assert!(hdr.soft);
        assert!(!hdr.valid);
        assert_eq!(hdr.n_entries, 5);
    }

    #[test]
    fn test_unpack_anote_minimal() {
        // Minimal ANOTE: 30 bytes (F#5 8th note from example in TN1)
        let data = vec![
            0x00, 0x00, // next = 0
            0x01, // staffn = 1
            0x05, // subType = EIGHTH_L_DUR
            0x40, // flags: selected=0, visible=1, soft=0, inChord=0, rest=0, unpitched=0, beamed=0, otherStemSide=0
            0xEC, // yqpit = -20 (0xEC as signed)
            0x00, 0x00, // xd = 0
            0x00, 0x00, // yd = 0
            0x01, 0x50, // ystem = 336
            0x00, 0x00, // playTimeDelta = 0
            0x00, 0xE4, // playDur = 228
            0x00, 0x00, // pTime = 0
            0x4E, // noteNum = 78 (F#5)
            0x4B, // onVelocity = 75
            0x40, // offVelocity = 64
            0x10, // tiedL=0, tiedR=0, ymovedots=1, ndots=0
            0x01, // voice = 1
            0x40, // rspIgnore=0, accident=4 (sharp), accSoft=0, playAsCue=0, micropitch=0
            0x28, // xmoveAcc=5, merged=0, courtesyAcc=0, doubleDur=0
            0x0B, // headShape=1 (NORMAL_VIS), xmovedots=3
            0x00, 0x00, // firstMod = 0
            0x00, // slurredL=0, slurredR=0, inTuplet=0, inOttava=0, small=0, tempFlag=0
            0x00, // fillerN = 0
        ];

        let note = unpack_anote_n105(&data).unwrap();
        assert_eq!(note.header.staffn, 1);
        assert_eq!(note.header.sub_type, 5); // EIGHTH_L_DUR
        assert_eq!(note.yqpit, -20);
        assert_eq!(note.note_num, 78); // F#5
        assert_eq!(note.on_velocity, 75);
        assert_eq!(note.accident, 4); // Sharp
        assert_eq!(note.voice, 1);
    }

    #[test]
    fn test_interpreted_score_get() {
        let mut score = InterpretedScore::new();

        // Add a dummy object at index 1
        let hdr = ObjectHeader {
            right: 2,
            left: 0,
            first_sub_obj: 0,
            xd: 0,
            yd: 0,
            obj_type: 0,
            selected: false,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ohdr_filler1: 0,
            obj_rect: Rect {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
            },
            rel_size: 0,
            ohdr_filler2: 0,
            n_entries: 0,
        };
        score.objects.push(InterpretedObject {
            index: 1,
            header: hdr.clone(),
            data: ObjData::Header(Header { header: hdr }),
        });

        // Test get() with valid link
        assert!(score.get(1).is_some());
        assert_eq!(score.get(1).unwrap().index, 1);

        // Test get() with NILINK
        assert!(score.get(NILINK).is_none());
        assert!(score.get(0).is_none());
    }
}
