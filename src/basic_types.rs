//! Basic types for Nightingale data model.
//!
//! Ported from `Nightingale/src/Precomps/NBasicTypes.h`.
//!
//! These types define the core coordinate systems and data structures used throughout
//! Nightingale. Many appear in .ngl score files, so their binary layout is fixed for
//! backward compatibility.
//!
//! # Coordinate Systems
//!
//! Nightingale uses three main coordinate systems:
//! - **DDIST** (Drawing Distance): 1/16 point resolution, range ±2048 points (±28 inches)
//! - **STDIST** (Staff Distance): 1/8 staff-line resolution, range ±4096 staff lines
//! - **QDIST** (Quarter-space Distance): 1/4 staff-space resolution, range ±8192 spaces
//!
//! The staff coordinate system uses `STD_LINEHT = 8` STDIST units per staff interline space.

use binrw::{BinRead, BinWrite};

// ============================================================================
// Distance and Coordinate Types (NBasicTypes.h:25-37)
// ============================================================================

/// Drawing distance: range ±2048 points (±28 inches), resolution 1/16 point.
///
/// Source: `NBasicTypes.h:31`
pub type Ddist = i16;

/// Long drawing distance: range ±134,217,728 points (±155,344 feet), resolution 1/16 point.
///
/// Source: `NBasicTypes.h:33`
pub type LongDdist = i32;

/// Staff distance: range ±4096 staff lines, resolution 1/8 staff line.
///
/// Source: `NBasicTypes.h:26`
pub type Stdist = i16;

/// Long staff distance: range ±268,435,456 spaces, resolution 1/8 space.
///
/// Source: `NBasicTypes.h:27`
pub type LongStdist = i32;

/// Quarter-space distance: range ±8192 spaces, resolution 1/4 space.
///
/// Source: `NBasicTypes.h:28`
pub type Qdist = i16;

/// Short staff distance: range ±16 spaces, resolution 1/8 space.
///
/// Source: `NBasicTypes.h:29`
pub type ShortStd = i8;

/// Short quarter-space distance: range ±32 spaces, resolution 1/4 space.
///
/// Source: `NBasicTypes.h:30`
pub type ShortQd = i8;

/// Short drawing distance: range ±8 points, resolution 1/16 point.
///
/// Source: `NBasicTypes.h:32`
pub type ShortDdist = i8;

/// STDIST scale: value for standard staff interline space.
///
/// This constant defines the relationship between STDIST units and staff spaces.
/// One staff interline space = 8 STDIST units.
///
/// Source: `NBasicTypes.h:34`
pub const STD_LINEHT: i16 = 8;

/// Fast floating-point type for calculations that don't need high precision.
///
/// Source: `NBasicTypes.h:36`
pub type FastFloat = f64;

/// String offset type for string pool references.
///
/// Source: `NBasicTypes.h:37`
pub type StringOffset = i32;

// ============================================================================
// Word Types (NBasicTypes.h:39-40)
// ============================================================================

/// Unsigned 16-bit word.
///
/// Source: `NBasicTypes.h:39`
pub type Word = u16;

/// Unsigned 32-bit double word.
///
/// Source: `NBasicTypes.h:40`
pub type DoubleWord = u32;

// ============================================================================
// Heap and Link Types (NBasicTypes.h:134-144)
// ============================================================================

/// Heap index: unsigned 16-bit index into object heap.
///
/// LINK values are used throughout Nightingale to reference objects in the heap-based
/// data structure. A LINK of 0 (NILINK) represents a null/invalid reference.
///
/// Source: `NBasicTypes.h:134`
pub type Link = u16;

/// Null/invalid LINK value.
pub const NILINK: Link = 0;

/// Heap descriptor for object storage.
///
/// Nightingale uses a heap-based memory system where objects of the same type are stored
/// in contiguous arrays (heaps). Each heap tracks its objects via a free list.
///
/// Source: `NBasicTypes.h:136-144`
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct Heap {
    /// Handle to floating array of objects (not directly portable; represents pointer on Mac)
    pub block: i32,
    /// Size in bytes of each object in array
    pub obj_size: i16,
    /// Type of object for this heap (from object type enum)
    pub obj_type: i16,
    /// Index of head of free list
    pub first_free: Link,
    /// Maximum number of objects in heap block
    pub n_objs: u16,
    /// Size of the free list
    pub n_free: u16,
    /// Nesting lock level: >0 means locked
    pub lock_level: i16,
}

// ============================================================================
// Point and Rectangle Types (NBasicTypes.h:42-48)
// ============================================================================

/// Point in DDIST coordinates (vertical, horizontal).
///
/// Nightingale uses (v, h) ordering following QuickDraw conventions.
///
/// Source: `NBasicTypes.h:42-44`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct DPoint {
    /// Vertical coordinate
    pub v: Ddist,
    /// Horizontal coordinate
    pub h: Ddist,
}

/// Rectangle in DDIST coordinates.
///
/// Source: `NBasicTypes.h:46-48`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct DRect {
    /// Top edge coordinate
    pub top: Ddist,
    /// Left edge coordinate
    pub left: Ddist,
    /// Bottom edge coordinate
    pub bottom: Ddist,
    /// Right edge coordinate
    pub right: Ddist,
}

/// QuickDraw-style point with i16 coordinates.
///
/// Used for legacy compatibility where QuickDraw Point types were used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct Point {
    /// Vertical coordinate
    pub v: i16,
    /// Horizontal coordinate
    pub h: i16,
}

/// QuickDraw-style rectangle with i16 coordinates.
///
/// Used for legacy compatibility where QuickDraw Rect types were used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct Rect {
    /// Top edge coordinate
    pub top: i16,
    /// Left edge coordinate
    pub left: i16,
    /// Bottom edge coordinate
    pub bottom: i16,
    /// Right edge coordinate
    pub right: i16,
}

// ============================================================================
// Key Signature Types (NBasicTypes.h:50-67)
// ============================================================================

/// Maximum number of items in a key signature.
///
/// This constant is referenced in WHOLE_KSINFO but defined elsewhere.
/// Standard Western key signatures use at most 7 sharps or flats.
pub const MAX_KSITEMS: usize = 7;

/// Key signature item: one sharp or flat in a key signature.
///
/// Source: `NBasicTypes.h:50-54`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct KsItem {
    /// Letter code: A=5, B=4, C=3, D=2, E=1, F=0, G=6
    pub letcode: i8,
    /// True if sharp, false if flat
    pub sharp: u8, // Boolean in C++
}

/// Complete key signature without context.
///
/// Source: `NBasicTypes.h:64-67`
#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct KsInfo {
    /// The sharps and flats in the key signature
    pub ks_item: [KsItem; MAX_KSITEMS],
    /// Number of sharps and flats in key signature
    pub n_ks_items: i8,
}

impl Default for KsInfo {
    fn default() -> Self {
        Self {
            ks_item: [KsItem::default(); MAX_KSITEMS],
            n_ks_items: 0,
        }
    }
}

// ============================================================================
// Time Signature Types (NBasicTypes.h:69-74)
// ============================================================================

/// Time signature information.
///
/// Source: `NBasicTypes.h:69-74`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct TsInfo {
    /// Time signature type/context
    pub ts_type: i8,
    /// Numerator (beats per measure)
    pub numerator: i8,
    /// Denominator (note value getting the beat)
    pub denominator: i8,
}

// ============================================================================
// Object Type Enumeration (NBasicTypes.h:96-129)
// ============================================================================

/// Object types in the Nightingale data structure.
///
/// This enum defines all object types that can appear in a Nightingale score.
/// The order MUST match the HEAP array order in vars.h and is relied upon by
/// generic subobject functions in Objects.c.
///
/// Source: `NBasicTypes.h:96-129`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ObjectType {
    /// Document header object
    Header = 0,
    /// Tail marker object
    Tail = 1,
    /// Note/rest synchronization object
    Sync = 2,
    /// Repeat ending marker
    RptEnd = 3,
    /// Page break
    Page = 4,
    /// System break
    System = 5,
    /// Staff object
    Staff = 6,
    /// Measure bar
    Measure = 7,
    /// Clef symbol
    Clef = 8,
    /// Key signature
    KeySig = 9,
    /// Time signature
    TimeSig = 10,
    /// Beam group
    BeamSet = 11,
    /// Staff/system connector (brace, bracket, line)
    Connect = 12,
    /// Dynamic marking
    Dynamic = 13,
    /// Note modifier (articulation, etc.)
    ModNR = 14,
    /// Graphic object (text, line, etc.)
    Graphic = 15,
    /// Ottava (8va, 8vb, etc.)
    Ottava = 16,
    /// Slur or set of ties
    Slur = 17,
    /// Tuplet
    Tuplet = 18,
    /// Grace note sync
    GrSync = 19,
    /// Tempo marking
    Tempo = 20,
    /// Spacer object
    Spacer = 21,
    /// Ending bracket
    Ending = 22,
    /// Pseudo-measure (for spacing)
    PsMeas = 23,
    /// Generic object (must be last heap)
    Obj = 24,
}

/// First object type value.
pub const LOW_TYPE: u16 = ObjectType::Header as u16;

/// Last object type value (one past the highest valid type).
pub const HIGH_TYPE: u16 = 25;

impl ObjectType {
    /// Convert from u16 to ObjectType.
    ///
    /// Returns None if the value is out of range.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(ObjectType::Header),
            1 => Some(ObjectType::Tail),
            2 => Some(ObjectType::Sync),
            3 => Some(ObjectType::RptEnd),
            4 => Some(ObjectType::Page),
            5 => Some(ObjectType::System),
            6 => Some(ObjectType::Staff),
            7 => Some(ObjectType::Measure),
            8 => Some(ObjectType::Clef),
            9 => Some(ObjectType::KeySig),
            10 => Some(ObjectType::TimeSig),
            11 => Some(ObjectType::BeamSet),
            12 => Some(ObjectType::Connect),
            13 => Some(ObjectType::Dynamic),
            14 => Some(ObjectType::ModNR),
            15 => Some(ObjectType::Graphic),
            16 => Some(ObjectType::Ottava),
            17 => Some(ObjectType::Slur),
            18 => Some(ObjectType::Tuplet),
            19 => Some(ObjectType::GrSync),
            20 => Some(ObjectType::Tempo),
            21 => Some(ObjectType::Spacer),
            22 => Some(ObjectType::Ending),
            23 => Some(ObjectType::PsMeas),
            24 => Some(ObjectType::Obj),
            _ => None,
        }
    }
}

// ============================================================================
// Window and Name Types (NBasicTypes.h:76-87)
// ============================================================================

/// Window types.
///
/// Source: `NBasicTypes.h:76-80`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowType {
    /// Detail/score window
    Detail = 1,
    /// Palette window
    Palette = 2,
    /// Keyboard window
    Keyboard = 3,
}

/// Types of part names to label left ends of systems.
///
/// Source: `NBasicTypes.h:82-87`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NamesType {
    /// No names displayed
    NoNames = 0,
    /// Abbreviated names
    AbbrevNames = 1,
    /// Full names
    FullNames = 2,
}

pub const MAX_NAMES_TYPE: u8 = NamesType::FullNames as u8;

// ============================================================================
// Text Style (NBasicTypes.h:149-157)
// ============================================================================

/// Text style descriptor.
///
/// Source: `NBasicTypes.h:149-157`
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct TextStyle {
    /// Default font name: Pascal string (length byte + up to 31 chars)
    pub font_name: [u8; 32],
    /// Filler for alignment
    pub filler2: u16,
    /// True means lyric spacing
    pub lyric: u16,
    /// Enclosure type
    pub enclosure: u16,
    /// True if size is relative to staff size
    pub rel_f_size: u16,
    /// If rel_f_size, small..large code; else point size
    pub font_size: u16,
    /// Font style flags
    pub font_style: i16,
}

// ============================================================================
// Voice Information (NBasicTypes.h:162-166)
// ============================================================================

/// Voice information descriptor.
///
/// Source: `NBasicTypes.h:162-166`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BinRead, BinWrite)]
#[br(big)]
pub struct VoiceInfo {
    /// Number of part using voice, or 0 if voice is unused
    pub partn: u8,
    /// Voice role: upper, lower, single, or cross-staff
    pub voice_role: u8,
    /// Voice number within the part, >= 1
    pub rel_voice: u8,
}

// ============================================================================
// Part Information (NBasicTypes.h:171-201)
// ============================================================================

/// FreeMIDI unique ID type (obsolete, kept for file compatibility).
///
/// FreeMIDI was a pre-OS X technology. These fields are no longer used but
/// remain in the struct for backward compatibility with old .ngl files.
pub type FmsUniqueId = [u8; 4];

/// FreeMIDI destination match (obsolete, kept for file compatibility).
///
/// This struct is approximately 280 bytes in the C++ code.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct FmsDestinationMatch {
    /// Placeholder for ~280 bytes of FreeMIDI data
    pub data: [u8; 280],
}

/// Part (instrument or voice) information.
///
/// Source: `NBasicTypes.h:171-201`
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
pub struct PartInfo {
    /// Index of next subobject
    pub next: Link,
    /// MIDI playback velocity offset
    pub part_velocity: i8,
    /// Index of first staff in the part
    pub first_staff: i8,
    /// MIDI program number
    pub patch_num: u8,
    /// Index of last staff in the part (>= first_staff)
    pub last_staff: i8,
    /// MIDI channel number
    pub channel: u8,
    /// Transposition in semitones (0 = none)
    pub transpose: i8,
    /// MIDI note number of lowest playable note
    pub lo_key_num: i16,
    /// MIDI note number of highest playable note
    pub hi_key_num: i16,
    /// Full name (e.g., to label 1st system), C string
    pub name: [u8; 32],
    /// Short name (e.g., for systems after 1st), C string
    pub short_name: [u8; 12],
    /// Name of highest playable note
    pub hi_key_name: i8,
    /// Accidental of highest playable note
    pub hi_key_acc: i8,
    /// Name of transposition
    pub tran_name: i8,
    /// Accidental of transposition
    pub tran_acc: i8,
    /// Name of lowest playable note
    pub lo_key_name: i8,
    /// Accidental of lowest playable note
    pub lo_key_acc: i8,
    /// Bank number for MIDI controller 0 (N103+ format)
    pub bank_number0: u8,
    /// Bank number for MIDI controller 32 (N103+ format)
    pub bank_number32: u8,
    /// FreeMIDI output device (obsolete, kept for compatibility)
    pub fms_output_device: FmsUniqueId,
    /// FreeMIDI output destination (obsolete, kept for compatibility)
    pub fms_output_destination: FmsDestinationMatch,
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_sizes() {
        // Verify that basic type aliases have the correct size
        assert_eq!(std::mem::size_of::<Ddist>(), 2, "DDIST should be 2 bytes");
        assert_eq!(
            std::mem::size_of::<LongDdist>(),
            4,
            "LONGDDIST should be 4 bytes"
        );
        assert_eq!(std::mem::size_of::<Stdist>(), 2, "STDIST should be 2 bytes");
        assert_eq!(
            std::mem::size_of::<LongStdist>(),
            4,
            "LONGSTDIST should be 4 bytes"
        );
        assert_eq!(std::mem::size_of::<Qdist>(), 2, "QDIST should be 2 bytes");
        assert_eq!(
            std::mem::size_of::<ShortStd>(),
            1,
            "SHORTSTD should be 1 byte"
        );
        assert_eq!(
            std::mem::size_of::<ShortQd>(),
            1,
            "SHORTQD should be 1 byte"
        );
        assert_eq!(
            std::mem::size_of::<ShortDdist>(),
            1,
            "SHORTDDIST should be 1 byte"
        );
        assert_eq!(std::mem::size_of::<Link>(), 2, "LINK should be 2 bytes");
        assert_eq!(
            std::mem::size_of::<StringOffset>(),
            4,
            "STRINGOFFSET should be 4 bytes"
        );
    }

    #[test]
    fn test_dpoint_default() {
        let p = DPoint::default();
        assert_eq!(p.v, 0);
        assert_eq!(p.h, 0);
    }

    #[test]
    fn test_drect_default() {
        let r = DRect::default();
        assert_eq!(r.top, 0);
        assert_eq!(r.left, 0);
        assert_eq!(r.bottom, 0);
        assert_eq!(r.right, 0);
    }

    #[test]
    fn test_dpoint_copy() {
        let p1 = DPoint { v: 100, h: 200 };
        let p2 = p1; // Should be Copy
        assert_eq!(p1.v, p2.v);
        assert_eq!(p1.h, p2.h);
    }

    #[test]
    fn test_drect_equality() {
        let r1 = DRect {
            top: 10,
            left: 20,
            bottom: 30,
            right: 40,
        };
        let r2 = DRect {
            top: 10,
            left: 20,
            bottom: 30,
            right: 40,
        };
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_std_lineht_constant() {
        assert_eq!(STD_LINEHT, 8, "STD_LINEHT must be 8");
    }

    #[test]
    fn test_nilink_constant() {
        assert_eq!(NILINK, 0, "NILINK must be 0");
    }

    #[test]
    fn test_object_type_values() {
        // Verify the numeric values of key object types
        assert_eq!(ObjectType::Header as u16, 0);
        assert_eq!(ObjectType::Tail as u16, 1);
        assert_eq!(ObjectType::Sync as u16, 2);
        assert_eq!(ObjectType::BeamSet as u16, 11);
        assert_eq!(ObjectType::Slur as u16, 17);
        assert_eq!(ObjectType::Obj as u16, 24);
    }

    #[test]
    fn test_object_type_from_u16() {
        assert_eq!(ObjectType::from_u16(0), Some(ObjectType::Header));
        assert_eq!(ObjectType::from_u16(2), Some(ObjectType::Sync));
        assert_eq!(ObjectType::from_u16(17), Some(ObjectType::Slur));
        assert_eq!(ObjectType::from_u16(24), Some(ObjectType::Obj));
        assert_eq!(ObjectType::from_u16(99), None);
    }

    #[test]
    fn test_low_high_type_constants() {
        assert_eq!(LOW_TYPE, 0);
        assert_eq!(HIGH_TYPE, 25);
    }

    #[test]
    fn test_ksinfo_default() {
        let ks = KsInfo::default();
        assert_eq!(ks.n_ks_items, 0);
        assert_eq!(ks.ks_item.len(), MAX_KSITEMS);
    }

    #[test]
    fn test_tsinfo_default() {
        let ts = TsInfo::default();
        assert_eq!(ts.ts_type, 0);
        assert_eq!(ts.numerator, 0);
        assert_eq!(ts.denominator, 0);
    }

    #[test]
    fn test_voiceinfo_default() {
        let vi = VoiceInfo::default();
        assert_eq!(vi.partn, 0);
        assert_eq!(vi.voice_role, 0);
        assert_eq!(vi.rel_voice, 0);
    }

    #[test]
    fn test_heap_struct_fields() {
        // Verify that Heap struct has all expected fields
        let heap = Heap {
            block: 0,
            obj_size: 64,
            obj_type: 2,
            first_free: NILINK,
            n_objs: 100,
            n_free: 50,
            lock_level: 0,
        };
        assert_eq!(heap.obj_size, 64);
        assert_eq!(heap.obj_type, 2);
        assert_eq!(heap.n_objs, 100);
        assert_eq!(heap.n_free, 50);
    }

    #[test]
    fn test_window_type_values() {
        assert_eq!(WindowType::Detail as u8, 1);
        assert_eq!(WindowType::Palette as u8, 2);
        assert_eq!(WindowType::Keyboard as u8, 3);
    }

    #[test]
    fn test_names_type_values() {
        assert_eq!(NamesType::NoNames as u8, 0);
        assert_eq!(NamesType::AbbrevNames as u8, 1);
        assert_eq!(NamesType::FullNames as u8, 2);
        assert_eq!(MAX_NAMES_TYPE, 2);
    }

    #[test]
    fn test_max_ksitems_constant() {
        assert_eq!(
            MAX_KSITEMS, 7,
            "Standard key signatures use at most 7 sharps/flats"
        );
    }
}
