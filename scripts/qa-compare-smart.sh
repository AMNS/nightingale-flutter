#!/bin/bash
# QA Compare: Smart Before/After PDF Deltas
#
# Efficiently identifies which fixtures have rendering changes by generating
# before/after PDFs, converting to PNG, and using the qa_compare.rs test to
# generate visual diffs (red highlights for changes) and an HTML report.
#
# Only fixtures with visual changes are shown in the report.
#
# Usage:
#   ./scripts/qa-compare-smart.sh
#
# Output:
#   test-output/qa-compare/
#     before/          — PDFs + PNGs from HEAD~1
#     after/           — PDFs + PNGs from HEAD
#     diff/            — Diff images (changed pixels in red)
#     report.html      — Interactive visual diff report

set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/test-output/qa-compare"

mkdir -p "$OUTPUT_DIR"/{before,after,diff}

echo "=== QA Compare: PDF Rendering Before/After Deltas ==="
echo ""

# Get current commit
CURRENT_COMMIT=$(git rev-parse --short HEAD)
PARENT_COMMIT=$(git rev-parse --short HEAD~1)

echo "Comparing:"
echo "  Before: $PARENT_COMMIT (HEAD~1)"
echo "  After:  $CURRENT_COMMIT (HEAD)"
echo ""

# Generate before PDFs (HEAD~1)
echo "1. Generating PDFs for HEAD~1..."
git stash >/dev/null 2>&1 || true
git checkout HEAD~1 >/dev/null 2>&1

cd "$REPO_ROOT"
cargo test --test ngl_all --test notelist_all --quiet 2>&1 | tail -3

# Copy PDFs and convert to PNG
cp test-output/ngl/*.pdf "$OUTPUT_DIR/before/" 2>/dev/null || true
cp test-output/notelist/*.pdf "$OUTPUT_DIR/before/" 2>/dev/null || true

echo "   Converting to PNG (72 DPI)..."
for pdf in "$OUTPUT_DIR/before"/*.pdf; do
    base=$(basename "$pdf" .pdf)
    sips -s format png -s dpiWidth 72 -s dpiHeight 72 "$pdf" --out "$OUTPUT_DIR/before/${base}.png" >/dev/null 2>&1
done

# Return to current commit
git checkout - >/dev/null 2>&1
git stash pop >/dev/null 2>&1 || true

# Generate after PDFs (HEAD)
echo ""
echo "2. Generating PDFs for HEAD..."

cd "$REPO_ROOT"
cargo test --test ngl_all --test notelist_all --quiet 2>&1 | tail -3

# Copy PDFs and convert to PNG
cp test-output/ngl/*.pdf "$OUTPUT_DIR/after/" 2>/dev/null || true
cp test-output/notelist/*.pdf "$OUTPUT_DIR/after/" 2>/dev/null || true

echo "   Converting to PNG (72 DPI)..."
for pdf in "$OUTPUT_DIR/after"/*.pdf; do
    base=$(basename "$pdf" .pdf)
    sips -s format png -s dpiWidth 72 -s dpiHeight 72 "$pdf" --out "$OUTPUT_DIR/after/${base}.png" >/dev/null 2>&1
done

# Run comparison test to generate diffs and changed.txt manifest
echo ""
echo "3. Analyzing deltas and generating diff images..."
echo ""

cd "$REPO_ROOT"
if cargo test --test qa_compare -- --nocapture 2>&1; then
    echo ""
    echo "✓ No visual changes detected."
    exit 0
else
    # Test failed (changes detected) — manifest and diffs were generated
    echo ""
    echo "⚠ Visual changes detected. Review in Flutter:"
    echo "   cd nightingale && flutter run"
    echo "   Navigate to: QA Compare (Before/After) screen"
    exit 1
fi
