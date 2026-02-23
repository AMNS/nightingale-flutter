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
//! - `interpret`: Decodes raw N105 bytes into typed Rust structs
//! - `writer`: (future) Writes .ngl files to disk
//!
//! Source: Nightingale/doc/Notes/NgaleFileFormatStatus.txt

pub mod error;
pub mod interpret;
pub mod reader;

// Future modules (stubs for now):
// pub mod writer;

// Re-export key types for convenience
pub use error::{NglError, Result};
pub use interpret::{
    unpack_anote_n105, unpack_anotebeam_n105, unpack_aslur_n105, unpack_object_header_n105,
    unpack_subobj_header_n105, InterpretedObject, InterpretedScore, ObjData,
};
pub use reader::{decode_string, HeapData, NglFile, NglVersion};
