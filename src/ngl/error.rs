//! Error types for NGL file reading
//!
//! Defines the error types that can occur when reading Nightingale binary files.

use std::fmt;
use std::io;

/// Errors that can occur when reading NGL files
#[derive(Debug)]
pub enum NglError {
    /// Invalid file version tag (expected "N103" or "N105")
    InvalidVersion(String),

    /// I/O error reading the file
    IoError(io::Error),

    /// Heap reading error (invalid count, size mismatch, etc.)
    HeapError(String),

    /// String pool error (invalid format, bad offset, etc.)
    StringPoolError(String),

    /// LASTtype validation failed (expected 25)
    InvalidLastType(u16),

    /// File is too short or corrupt
    UnexpectedEof,

    /// Invalid object type encountered
    InvalidObjectType(u8),

    /// Feature not yet implemented
    NotImplemented(String),
}

impl fmt::Display for NglError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NglError::InvalidVersion(s) => write!(f, "Invalid NGL version: {}", s),
            NglError::IoError(e) => write!(f, "I/O error: {}", e),
            NglError::HeapError(s) => write!(f, "Heap error: {}", s),
            NglError::StringPoolError(s) => write!(f, "String pool error: {}", s),
            NglError::InvalidLastType(v) => write!(f, "Invalid LASTtype: {} (expected 25)", v),
            NglError::UnexpectedEof => write!(f, "Unexpected end of file"),
            NglError::InvalidObjectType(t) => write!(f, "Invalid object type: {}", t),
            NglError::NotImplemented(s) => write!(f, "Not implemented: {}", s),
        }
    }
}

impl std::error::Error for NglError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NglError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for NglError {
    fn from(err: io::Error) -> Self {
        NglError::IoError(err)
    }
}

pub type Result<T> = std::result::Result<T, NglError>;
