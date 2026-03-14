#!/bin/sh
# Visual review of golden bitmap changes (DEPRECATED).
#
# Golden bitmaps have been replaced with PDF-based visual review via Flutter.
# This script is kept for reference but should not be used in CI.
#
# For visual review of rendering changes, use:
#   ./scripts/qa-compare-smart.sh           # Before/after PDF comparison
#   cd nightingale && flutter run           # Review in Flutter QA Compare screen
#
# To regenerate PDF output after code changes:
#   cargo test --test ngl_all --test notelist_all
#   # PDFs appear in test-output/ngl/ and test-output/notelist/

echo "⚠️  WARNING: This script is deprecated."
echo ""
echo "Golden bitmap regression tests have been removed."
echo "Use Flutter-based visual review instead:"
echo ""
echo "  1. ./scripts/qa-compare-smart.sh"
echo "  2. cd nightingale && flutter run"
echo "  3. Navigate to: QA Compare (Before/After) screen"
echo ""
exit 1
