//! Document header parsing for N105 .ngl files.
//!
//! This module re-exports the DocumentHeader from doc_types.rs which already
//! provides complete parsing support via binrw.
//!
//! The critical fields for score navigation are headL and tailL which are
//! found in the ScoreHeader, not the DocumentHeader.

// Re-export types from doc_types for convenience
pub use crate::doc_types::{DocumentHeader, ScoreHeader};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ngl::NglFile;

    #[test]
    fn test_parse_document_header_from_file() {
        // Test parsing DocumentHeader from a real file
        let path = "tests/fixtures/01_me_and_lucy.ngl";
        let ngl = NglFile::read_from_file(path).expect("Failed to read file");

        let doc_hdr = DocumentHeader::from_n105_bytes(&ngl.doc_header_raw)
            .expect("Failed to parse document header");

        // Basic sanity checks
        let page_w = doc_hdr.orig_paper_rect.right - doc_hdr.orig_paper_rect.left;
        let page_h = doc_hdr.orig_paper_rect.bottom - doc_hdr.orig_paper_rect.top;

        assert!(page_w > 0, "Page width should be positive");
        assert!(page_h > 0, "Page height should be positive");
        assert!(doc_hdr.num_sheets > 0, "Should have at least one sheet");
    }
}
