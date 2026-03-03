#!/usr/bin/env bash
# bootstrap-og-source.sh — clone the OG Nightingale C source alongside this repo.
#
# The OG source lives at ../OGNGale_source/ (a sibling of nightingale-modernize/).
# It is excluded from this repo via .gitignore — clone it once and keep it updated.
#
# Usage:
#   scripts/bootstrap-og-source.sh          # clone if absent, report if present
#   scripts/bootstrap-og-source.sh --update # also pull latest if already cloned

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET="$REPO_ROOT/../OGNGale_source"
REMOTE="https://github.com/AMNS/Nightingale.git"
BRANCH="develop"

if [ -d "$TARGET/.git" ]; then
    echo "OGNGale_source already present at: $TARGET"
    if [ "${1:-}" = "--update" ]; then
        echo "Pulling latest $BRANCH..."
        git -C "$TARGET" fetch origin
        git -C "$TARGET" checkout "$BRANCH"
        git -C "$TARGET" pull --ff-only
        echo "Up to date."
    else
        echo "Run with --update to pull latest, or cd into it and git pull."
    fi
else
    echo "Cloning OG Nightingale C source → $TARGET"
    git clone "$REMOTE" "$TARGET"
    git -C "$TARGET" checkout "$BRANCH"
    echo ""
    echo "Done. Key paths:"
    echo "  $TARGET/src/CFilesBoth/   — core drawing/layout/engraving"
    echo "  $TARGET/src/Utilities/    — utility functions"
    echo "  $TARGET/src/Precomps/     — headers / type definitions"
fi
