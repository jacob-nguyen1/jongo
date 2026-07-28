#!/usr/bin/env bash
# Produce the final submission zip: source/ (buildable) + prebuilt/ (load-unpacked fallback).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')"
STAGE="$ROOT/release/jongo-${VERSION}-submission"
ARCHIVE="$ROOT/release/jongo-${VERSION}-submission.zip"

echo "==> Step 1: build wasm and assemble dist/ via package.sh"
bash ./scripts/package.sh

echo "==> Step 2: staging submission directory at $STAGE"
rm -rf "$STAGE" "$ARCHIVE"
mkdir -p "$STAGE/source" "$STAGE/prebuilt"

# Copy source tree, excluding local/dev/generated junk.
rsync -a \
  --exclude='.git' \
  --exclude='.agents' \
  --exclude='target' \
  --exclude='dist' \
  --exclude='release' \
  --exclude='pkg' \
  --exclude='scratch' \
  --exclude='tests' \
  --exclude='src/bin' \
  --exclude='README.md' \
  --exclude='scripts/submission.sh' \
  --exclude='*.orig' \
  --exclude='*.rej' \
  --exclude='*.patch' \
  --exclude='todo.md' \
  --exclude='temp_input.txt' \
  --exclude='Cargo.lock' \
  ./ "$STAGE/source/"

# Copy prebuilt Chrome extension as fallback.
cp -r dist/chrome/. "$STAGE/prebuilt/"

# Put grader-facing README at zip root.
cp README.md "$STAGE/README.md"

echo "==> Step 3: writing archive $ARCHIVE"
(cd "$ROOT/release" && zip -r -q "jongo-${VERSION}-submission.zip" "jongo-${VERSION}-submission")

echo
echo "Done."
echo "  Staged at:  $STAGE"
echo "  Archive:    $ARCHIVE"
echo "  Size:       $(du -h "$ARCHIVE" | cut -f1)"
echo
echo "Top-level contents:"
find "$STAGE" -maxdepth 2 | sort
