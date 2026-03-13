#!/bin/bash
# QA Compare: Smart Before/After PDF Deltas
#
# Efficiently identifies which fixtures have rendering changes by checking
# command stream hash deltas, then shows detailed PDF comparisons only for
# changed fixtures (avoid full re-render of 26+ fixtures).
#
# Usage:
#   ./scripts/qa-compare-smart.sh [--all]
#
# With --all: Re-render all fixtures (slower but comprehensive)
# Default:   Smart mode - only show changed fixtures based on hashes

set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/test-output/qa-compare"
SMART_MODE=true

if [[ "$1" == "--all" ]]; then
    SMART_MODE=false
fi

mkdir -p "$OUTPUT_DIR"

echo "=== QA Compare: PDF Rendering Deltas ==="
echo ""

if [[ "$SMART_MODE" == "true" ]]; then
    echo "Smart Mode: Identifying changed fixtures via command stream hashes..."
    echo ""

    # Get current commit
    CURRENT_COMMIT=$(git rev-parse --short HEAD)
    PARENT_COMMIT=$(git rev-parse --short HEAD~1)

    # Run tests and capture current hashes
    cd "$REPO_ROOT"
    CURRENT_HASHES=$(cargo test --test ngl_all --test notelist_all --quiet 2>&1 | grep "hash" || echo "")

    # Checkout parent and get those hashes
    git stash >/dev/null 2>&1 || true
    git checkout HEAD~1 >/dev/null 2>&1

    PARENT_HASHES=$(cargo test --test ngl_all --test notelist_all --quiet 2>&1 | grep "hash" || echo "")

    # Back to current
    git checkout - >/dev/null 2>&1
    git stash pop >/dev/null 2>&1 || true

    # Compare hashes to find changes
    CHANGED_FIXTURES=$(comm -13 <(echo "$PARENT_HASHES" | sort) <(echo "$CURRENT_HASHES" | sort) || echo "")

    if [[ -z "$CHANGED_FIXTURES" ]]; then
        echo "✓ No rendering changes detected (all fixtures match)"
        exit 0
    fi

    echo "Changed fixtures: $(echo "$CHANGED_FIXTURES" | wc -l)"
    echo ""
    echo "Generating before/after PDFs for changed fixtures only..."
else
    echo "Full Mode: Re-rendering all fixtures for comprehensive comparison..."
    echo ""
fi

# Generate before PDFs
echo "1. Generating PDFs for HEAD~1..."
mkdir -p "$OUTPUT_DIR/before"
git stash >/dev/null 2>&1 || true
git checkout HEAD~1 >/dev/null 2>&1

cd "$REPO_ROOT"
cargo test --test ngl_all --test notelist_all --quiet 2>&1 | tail -3

# Copy PDFs
cp test-output/ngl/*.pdf "$OUTPUT_DIR/before/" 2>/dev/null || true
cp test-output/notelist/*.pdf "$OUTPUT_DIR/before/" 2>/dev/null || true

# Return to current
git checkout - >/dev/null 2>&1
git stash pop >/dev/null 2>&1 || true

# Generate after PDFs
echo ""
echo "2. Generating PDFs for HEAD..."
mkdir -p "$OUTPUT_DIR/after"

cd "$REPO_ROOT"
cargo test --test ngl_all --test notelist_all --quiet 2>&1 | tail -3

# Copy PDFs
cp test-output/ngl/*.pdf "$OUTPUT_DIR/after/" 2>/dev/null || true
cp test-output/notelist/*.pdf "$OUTPUT_DIR/after/" 2>/dev/null || true

# Convert to PNG
echo ""
echo "3. Converting to PNG (150 DPI) for pixel comparison..."
for pdf in "$OUTPUT_DIR/before"/*.pdf; do
    base=$(basename "$pdf" .pdf)
    sips -s format png -s dpiWidth 150 -s dpiHeight 150 "$pdf" --out "$OUTPUT_DIR/before/${base}.png" 2>/dev/null
done

for pdf in "$OUTPUT_DIR/after"/*.pdf; do
    base=$(basename "$pdf" .pdf)
    sips -s format png -s dpiWidth 150 -s dpiHeight 150 "$pdf" --out "$OUTPUT_DIR/after/${base}.png" 2>/dev/null
done

# Generate report
echo ""
echo "4. Analyzing deltas..."

cat > "$OUTPUT_DIR/report.txt" << EOF
QA Compare: PDF Rendering Before/After Deltas
==============================================

Commits:
  Before: $(git rev-parse --short HEAD~1 2>/dev/null || echo "unknown")
  After:  $(git rev-parse --short HEAD)

Results:
--------
EOF

CHANGED_COUNT=0
TOTAL_COUNT=0

for after_png in "$OUTPUT_DIR/after"/*.png; do
    base=$(basename "$after_png")
    name="${base%.png}"
    before_png="$OUTPUT_DIR/before/$base"

    ((TOTAL_COUNT++))

    if [[ ! -f "$before_png" ]]; then
        echo "NEW      $name" >> "$OUTPUT_DIR/report.txt"
        ((CHANGED_COUNT++))
    elif cmp -s "$before_png" "$after_png"; then
        echo "✓ SAME   $name" >> "$OUTPUT_DIR/report.txt"
    else
        echo "⚠ MODIFIED $name" >> "$OUTPUT_DIR/report.txt"
        ((CHANGED_COUNT++))
    fi
done

echo "" >> "$OUTPUT_DIR/report.txt"
echo "Summary:" >> "$OUTPUT_DIR/report.txt"
echo "  Total:    $TOTAL_COUNT fixtures" >> "$OUTPUT_DIR/report.txt"
echo "  Changed:  $CHANGED_COUNT fixtures" >> "$OUTPUT_DIR/report.txt"
echo "  Unchanged: $((TOTAL_COUNT - CHANGED_COUNT)) fixtures" >> "$OUTPUT_DIR/report.txt"
echo "" >> "$OUTPUT_DIR/report.txt"

if [[ $CHANGED_COUNT -eq 0 ]]; then
    echo "✓ No visual changes detected" >> "$OUTPUT_DIR/report.txt"
else
    echo "⚠ Visual changes detected in $CHANGED_COUNT fixture(s)" >> "$OUTPUT_DIR/report.txt"
fi

# Print to stdout
cat "$OUTPUT_DIR/report.txt"

echo ""
echo "Artifacts:"
echo "  Report:  $OUTPUT_DIR/report.txt"
echo "  Befores: $OUTPUT_DIR/before/"
echo "  Afters:  $OUTPUT_DIR/after/"
echo ""

if [[ $CHANGED_COUNT -gt 0 ]]; then
    echo "To review visual changes:"
    echo "  open $OUTPUT_DIR/"
    echo ""
    exit 1  # Signal that changes were found
else
    exit 0  # No changes
fi
