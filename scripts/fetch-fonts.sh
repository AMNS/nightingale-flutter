#!/bin/sh
# Download required fonts for Nightingale rendering.
#
# Fonts:
#   - Bravura (SMuFL music font) — from the Bravura GitHub releases
#   - Liberation Sans/Serif — metric-compatible replacements for
#     Times New Roman and Helvetica (fonts used in OG Nightingale files)
#
# All fonts are SIL Open Font License (OFL).
#
# Usage:
#   ./scripts/fetch-fonts.sh
#
# Output: assets/fonts/

set -e

FONT_DIR="assets/fonts"
mkdir -p "$FONT_DIR"

echo "=== Fetching Nightingale fonts ==="

# ── Bravura (SMuFL music font) ──────────────────────────────────────────
BRAVURA_VER="1.392"
BRAVURA_URL="https://github.com/steinbergmedia/bravura/releases/download/bravura-${BRAVURA_VER}/bravura-${BRAVURA_VER}.zip"

if [ ! -f "$FONT_DIR/Bravura.otf" ]; then
    echo "Downloading Bravura ${BRAVURA_VER}..."
    TMPDIR=$(mktemp -d)
    curl -sL -o "$TMPDIR/bravura.zip" "$BRAVURA_URL"
    unzip -q -o "$TMPDIR/bravura.zip" -d "$TMPDIR/bravura"
    cp "$TMPDIR/bravura/redist/otf/Bravura.otf" "$FONT_DIR/"
    cp "$TMPDIR/bravura/redist/otf/BravuraText.otf" "$FONT_DIR/"
    cp "$TMPDIR/bravura/redist/LICENSE.txt" "$FONT_DIR/OFL-Bravura.txt"
    rm -rf "$TMPDIR"
    echo "  Bravura.otf + BravuraText.otf installed."
else
    echo "  Bravura.otf already present."
fi

# ── Liberation Sans + Serif (text fonts) ─────────────────────────────────
LIBERATION_VER="2.1.5"
LIBERATION_URL="https://github.com/liberationfonts/liberation-fonts/files/7261482/liberation-fonts-ttf-${LIBERATION_VER}.tar.gz"

if [ ! -f "$FONT_DIR/LiberationSerif-Regular.ttf" ]; then
    echo "Downloading Liberation Fonts ${LIBERATION_VER}..."
    TMPDIR=$(mktemp -d)
    curl -sL -o "$TMPDIR/liberation.tar.gz" "$LIBERATION_URL"
    tar xzf "$TMPDIR/liberation.tar.gz" -C "$TMPDIR"
    cp "$TMPDIR/liberation-fonts-ttf-${LIBERATION_VER}"/LiberationSerif-*.ttf "$FONT_DIR/"
    cp "$TMPDIR/liberation-fonts-ttf-${LIBERATION_VER}"/LiberationSans-*.ttf "$FONT_DIR/"
    cp "$TMPDIR/liberation-fonts-ttf-${LIBERATION_VER}/LICENSE" "$FONT_DIR/LICENSE-Liberation.txt"
    rm -rf "$TMPDIR"
    echo "  Liberation Sans + Serif (8 variants) installed."
else
    echo "  Liberation fonts already present."
fi

echo ""
echo "All fonts ready in $FONT_DIR/"
ls "$FONT_DIR/"
