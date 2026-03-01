#!/bin/sh
# OG vs Modern rendering comparison.
#
# Renders our BitmapRenderer output alongside the OG Nightingale reference PDFs
# (via CoreGraphics), generates pixel diffs, and opens an interactive HTML report
# with side-by-side/blink/slider comparison modes.
#
# Usage:
#   ./scripts/og-compare.sh            # run + open report
#   ./scripts/og-compare.sh --no-open  # run without opening (CI-friendly)
#
# Output: test-output/og-comparison/
#   report.html                    — interactive comparison report
#   {name}_ours_page{N}.png       — our rendering
#   {name}_og_page{N}.png         — OG reference (CoreGraphics render of PDF)
#   {name}_diff_page{N}.png       — visual diff

set -e

echo "=== OG vs Modern: Rendering Comparison ==="
echo ""

cargo test --test og_comparison -- --nocapture 2>&1

REPORT="test-output/og-comparison/report.html"

if [ -f "$REPORT" ]; then
    if [ "$1" != "--no-open" ]; then
        echo ""
        echo "Opening comparison report..."
        open "$REPORT" 2>/dev/null || xdg-open "$REPORT" 2>/dev/null || echo "(open manually: $REPORT)"
    else
        echo ""
        echo "Report: $REPORT"
    fi
else
    echo ""
    echo "No report generated (check for errors above)."
fi
