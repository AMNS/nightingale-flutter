#!/bin/sh
# Visual review of golden bitmap changes.
#
# Compares current golden bitmaps against git HEAD, generates diff images,
# and opens an HTML report with side-by-side before/after/diff views
# and per-file approval controls.
#
# Usage:
#   ./scripts/visual-review.sh            # show diff summary + open HTML report
#   ./scripts/visual-review.sh --no-open  # summary only (CI-friendly)
#
# Output: test-output/golden-diff/
#   review.html            — interactive HTML diff report
#   {name}_old.png         — committed version
#   {name}_new.png         — current version
#   {name}_diff.png        — visual diff

set -e

echo "=== Visual Review: Golden Bitmap Diffs ==="
echo ""

# Run the golden_diff test with output
cargo test --test golden_diff -- --nocapture 2>&1

DIFF_DIR="test-output/golden-diff"
REPORT="$DIFF_DIR/review.html"

# Open HTML report if it exists and has diffs
if [ -f "$REPORT" ]; then
    diff_count=$(find "$DIFF_DIR" -name '*_diff.png' 2>/dev/null | wc -l | tr -d ' ')
    if [ "$diff_count" -gt 0 ]; then
        echo ""
        echo "$diff_count bitmap(s) changed."
        if [ "$1" != "--no-open" ]; then
            echo "Opening visual review..."
            open "$REPORT" 2>/dev/null || xdg-open "$REPORT" 2>/dev/null || echo "(open manually: $REPORT)"
        else
            echo "Report: $REPORT"
        fi
    else
        echo ""
        echo "No bitmap changes detected."
    fi
else
    # Fallback: no HTML report means no changes
    echo ""
    echo "No bitmap changes detected."
fi

echo ""
echo "To update goldens after intentional changes:"
echo "  REGENERATE_REFS=1 cargo test test_all_ngl_bitmap_regression"
echo "  REGENERATE_REFS=1 cargo test test_all_notelists_bitmap_regression"
