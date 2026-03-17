//! Nightingale file format (.ngl) support.
//!
//! This module provides reading, writing, and interpretation of Nightingale score files.
//! The .ngl format stores scores in a binary heap-based structure with:
//!
//! - **N105 format**: Nightingale 5.x (current standard, PowerPC bitfields)
//! - **N103 format**: Nightingale 3.x/4.x (legacy, smaller headers)
//! - **N106 format**: Nightingale 6.x (planned, expanded headers)
//!
//! ## Module Structure
//!
//! - `error`: Error types for NGL operations
//! - `reader`: Reads .ngl files from disk into raw heap data
//! - `interpret`: Decodes raw N105 bytes into typed Rust structs (core types + interpret_heap)
//! - `unpack_headers`: ObjectHeader, SubObjHeader, KsInfo unpackers
//! - `unpack_notes`: ANote, ANoteBeam, ANoteTuple unpackers
//! - `unpack_structural`: AStaff, AMeasure unpackers
//! - `unpack_notation`: AClef, AKeySig, ATimeSig unpackers
//! - `unpack_slur`: ASlur unpacker
//! - `unpack_stubs`: Stub unpackers for types not yet fully implemented
//! - `pack_subobjects`: Subobject serialization (inverse of unpack_* functions)
//! - `doc_header`: Parses the 72-byte document header
//! - `writer`: Writes .ngl files to disk (skeleton with helper functions)
//!
//! Source: Nightingale/doc/Notes/NgaleFileFormatStatus.txt

pub mod doc_header;
pub mod error;
pub mod interpret;
pub mod pack_objects;
pub mod pack_subobjects;
pub mod reader;
pub mod unpack_headers;
pub mod unpack_notation;
pub mod unpack_notes;
pub mod unpack_slur;
pub mod unpack_structural;
pub mod unpack_stubs;
pub mod writer;

// Re-export key types for convenience
pub use doc_header::DocumentHeader;
pub use error::{NglError, Result};
pub use interpret::{
    interpret_heap, unpack_anote_n105, unpack_anotebeam_n105, unpack_aslur_n105,
    unpack_object_header_n105, unpack_subobj_header_n105, InterpretedObject, InterpretedScore,
    ObjData,
};
pub use reader::{decode_string, mac_roman_to_string, HeapData, NglFile, NglVersion};
pub use writer::NglWriter;
