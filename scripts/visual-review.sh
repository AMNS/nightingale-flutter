#!/bin/sh
# Visual review of golden bitmap changes.
#
# Compares current golden bitmaps against git HEAD and generates
# diff images (matching pixels dimmed, differences in bright red).
#
# Usage:
#   ./scripts/visual-review.sh          # show diff summary + open diff dir
#   ./scripts/visual-review.sh --no-open  # summary only (CI-friendly)
#
# Output: /tmp/nightingale-test-output/golden-diff/
#   {name}_old.png   — committed version
#   {name}_new.png   — current version
#   {name}_diff.png  — visual diff

set -e

echo "=== Visual Review: Golden Bitmap Diffs ==="
echo ""

# Run the golden_diff test with output
cargo test --test golden_diff -- --nocapture 2>&1

DIFF_DIR="/tmp/nightingale-test-output/golden-diff"

# Count files in diff dir
if [ -d "$DIFF_DIR" ]; then
    diff_count=$(find "$DIFF_DIR" -name '*_diff.png' 2>/dev/null | wc -l | tr -d ' ')
    if [ "$diff_count" -gt 0 ]; then
        echo ""
        echo "$diff_count bitmap(s) changed. Diff images at:"
        echo "  $DIFF_DIR"
        if [ "$1" != "--no-open" ]; then
            echo ""
            echo "Opening diff directory..."
            open "$DIFF_DIR" 2>/dev/null || xdg-open "$DIFF_DIR" 2>/dev/null || echo "(open manually: $DIFF_DIR)"
        fi
    else
        echo ""
        echo "No bitmap changes detected."
    fi
else
    echo ""
    echo "No diff directory found — golden_diff test may have failed."
fi

echo ""
echo "To update goldens after intentional changes:"
echo "  REGENERATE_REFS=1 cargo test test_all_ngl_bitmap_regression"
echo "  REGENERATE_REFS=1 cargo test test_all_notelists_bitmap_regression"
