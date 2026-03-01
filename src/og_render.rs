//! Native macOS PDF rendering via CoreGraphics FFI.
//!
//! Uses the same rendering engine as Preview.app to produce pixel-perfect
//! reference bitmaps from OG Nightingale PDF output.
//!
//! This module is only compiled on macOS (`#[cfg(target_os = "macos")]`).
//! On other platforms it provides no-op stubs.

#[cfg(target_os = "macos")]
mod platform {
    use core_foundation::base::TCFType;
    use core_foundation::url::CFURL;
    use core_graphics::color_space::CGColorSpace;
    use core_graphics::context::CGContext;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};

    /// Information about a rendered PDF page.
    pub struct RenderedPage {
        /// RGBA pixel data (8 bits per component, premultiplied alpha).
        pub rgba: Vec<u8>,
        /// Width in pixels.
        pub width: u32,
        /// Height in pixels.
        pub height: u32,
    }

    // ── Raw CoreGraphics PDF FFI (not exposed by the `core-graphics` crate) ──

    #[allow(non_camel_case_types)]
    enum CGPDFDocumentRef_opaque {}
    #[allow(non_camel_case_types)]
    type CGPDFDocumentRef = *const CGPDFDocumentRef_opaque;

    #[allow(non_camel_case_types)]
    enum CGPDFPageRef_opaque {}
    #[allow(non_camel_case_types)]
    type CGPDFPageRef = *const CGPDFPageRef_opaque;

    const K_CGPDF_MEDIA_BOX: i32 = 0;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPDFDocumentCreateWithURL(url: core_foundation::url::CFURLRef) -> CGPDFDocumentRef;
        fn CGPDFDocumentRelease(document: CGPDFDocumentRef);
        fn CGPDFDocumentGetNumberOfPages(document: CGPDFDocumentRef) -> usize;
        fn CGPDFDocumentGetPage(document: CGPDFDocumentRef, page_number: usize) -> CGPDFPageRef;
        fn CGPDFPageGetBoxRect(page: CGPDFPageRef, box_type: i32) -> CGRect;
        fn CGContextDrawPDFPage(context: core_graphics::sys::CGContextRef, page: CGPDFPageRef);
    }

    /// Extract the raw CGContextRef pointer from a CGContext.
    ///
    /// CGContext is a foreign_type! wrapper whose first (and only) field is
    /// `*mut core_graphics::sys::CGContext`. This function extracts it without
    /// requiring the ForeignType trait to be in scope.
    fn cg_context_ptr(ctx: &CGContext) -> core_graphics::sys::CGContextRef {
        // SAFETY: CGContext is repr(transparent) over a *mut sys::CGContext
        unsafe { *(ctx as *const CGContext as *const core_graphics::sys::CGContextRef) }
    }

    /// RAII wrapper for CGPDFDocument.
    struct PdfDocument {
        ptr: CGPDFDocumentRef,
    }

    impl PdfDocument {
        fn open(path: &std::path::Path) -> Option<Self> {
            let url = CFURL::from_path(path, false)?;
            let ptr = unsafe { CGPDFDocumentCreateWithURL(url.as_concrete_TypeRef()) };
            if ptr.is_null() {
                None
            } else {
                Some(PdfDocument { ptr })
            }
        }

        fn page_count(&self) -> usize {
            unsafe { CGPDFDocumentGetNumberOfPages(self.ptr) }
        }

        fn page(&self, page_num: usize) -> Option<CGPDFPageRef> {
            let p = unsafe { CGPDFDocumentGetPage(self.ptr, page_num) };
            if p.is_null() {
                None
            } else {
                Some(p)
            }
        }
    }

    impl Drop for PdfDocument {
        fn drop(&mut self) {
            unsafe {
                CGPDFDocumentRelease(self.ptr);
            }
        }
    }

    /// Render a single page from a PDF file to an RGBA bitmap.
    ///
    /// - `pdf_path`: absolute path to the PDF file
    /// - `page_num`: 1-based page number
    /// - `dpi`: target resolution (72 = 1pt per pixel, matching our BitmapRenderer)
    ///
    /// Returns `None` if the file cannot be opened, the page doesn't exist,
    /// or rendering fails.
    pub fn render_pdf_page(pdf_path: &str, page_num: usize, dpi: f64) -> Option<RenderedPage> {
        let doc = PdfDocument::open(std::path::Path::new(pdf_path))?;
        let page = doc.page(page_num)?;
        let media_box = unsafe { CGPDFPageGetBoxRect(page, K_CGPDF_MEDIA_BOX) };

        let scale = dpi / 72.0;
        let width = (media_box.size.width * scale).ceil() as u32;
        let height = (media_box.size.height * scale).ceil() as u32;

        if width == 0 || height == 0 {
            return None;
        }

        // Create a bitmap context — pass None so CoreGraphics allocates the buffer.
        // We'll copy data out via ctx.data() afterward.
        let color_space = CGColorSpace::create_device_rgb();
        let mut ctx = CGContext::create_bitmap_context(
            None,
            width as usize,
            height as usize,
            8,
            0, // let CG compute bytes_per_row
            &color_space,
            core_graphics::base::kCGImageAlphaPremultipliedLast,
        );

        // Fill with white background
        ctx.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0);
        ctx.fill_rect(CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(width as f64, height as f64),
        ));

        // Scale for DPI and draw the PDF page
        ctx.scale(scale, scale);
        unsafe {
            CGContextDrawPDFPage(cg_context_ptr(&ctx), page);
        }

        // Extract the pixel data.
        // ctx.data() returns a mutable slice referencing CG's internal buffer.
        // The buffer is (height * bytes_per_row) bytes, but bytes_per_row may
        // have padding beyond width*4. We need to repack to exactly width*4.
        let bytes_per_row = ctx.bytes_per_row();
        let raw = ctx.data();
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height as usize {
            let row_start = y * bytes_per_row;
            let row_end = row_start + (width as usize * 4);
            rgba.extend_from_slice(&raw[row_start..row_end]);
        }

        Some(RenderedPage {
            rgba,
            width,
            height,
        })
    }

    /// Return the number of pages in a PDF file.
    pub fn pdf_page_count(pdf_path: &str) -> Option<usize> {
        let doc = PdfDocument::open(std::path::Path::new(pdf_path))?;
        Some(doc.page_count())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    /// Information about a rendered PDF page.
    pub struct RenderedPage {
        pub rgba: Vec<u8>,
        pub width: u32,
        pub height: u32,
    }

    /// Stub: CoreGraphics PDF rendering is macOS-only.
    pub fn render_pdf_page(_pdf_path: &str, _page_num: usize, _dpi: f64) -> Option<RenderedPage> {
        None
    }

    /// Stub: CoreGraphics PDF rendering is macOS-only.
    pub fn pdf_page_count(_pdf_path: &str) -> Option<usize> {
        None
    }
}

pub use platform::{pdf_page_count, render_pdf_page, RenderedPage};
