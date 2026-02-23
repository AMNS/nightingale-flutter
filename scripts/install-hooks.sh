#!/bin/sh
# Install git hooks for this repository.
# Run once after cloning: ./scripts/install-hooks.sh

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOK_DIR="$REPO_ROOT/.git/hooks"

cp "$REPO_ROOT/scripts/pre-commit" "$HOOK_DIR/pre-commit"
chmod +x "$HOOK_DIR/pre-commit"

echo "Git hooks installed."
