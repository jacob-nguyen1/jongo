#!/usr/bin/env bash
# Build wasm and assemble Chrome / Firefox store zips under release/.
# Requires: wasm-pack, bash. Prefer jq + zip; falls back to python3 for JSON/zip.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GECKO_ID="${GECKO_ID:-jongo@jacob-nguyen1.github.io}"

die() {
  echo "error: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found on PATH"
}

need_cmd wasm-pack
need_cmd python3

# --- version helpers (jq preferred, python3 fallback) ---
manifest_version() {
  if command -v jq >/dev/null 2>&1; then
    jq -r '.version' manifest.json
  else
    python3 -c 'import json; print(json.load(open("manifest.json"))["version"])'
  fi
}

cargo_version() {
  # First non-comment version = line under [package]
  grep -m1 '^version[[:space:]]*=' Cargo.toml | sed -E 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/'
}

apply_firefox_manifest() {
  local src="$1"
  local dest="$2"
  if command -v jq >/dev/null 2>&1; then
    jq --arg id "$GECKO_ID" \
      '. + {browser_specific_settings: {gecko: {id: $id}}}' \
      "$src" >"$dest"
  else
    GECKO_ID="$GECKO_ID" python3 - "$src" "$dest" <<'PY'
import json, os, sys
src, dest = sys.argv[1], sys.argv[2]
with open(src) as f:
    data = json.load(f)
data["browser_specific_settings"] = {"gecko": {"id": os.environ["GECKO_ID"]}}
with open(dest, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
  fi
}

zip_dir_contents() {
  local dir="$1"
  local out_zip="$2"
  rm -f "$out_zip"
  if command -v zip >/dev/null 2>&1; then
    (cd "$dir" && zip -r -q "$out_zip" .)
  else
    python3 - "$dir" "$out_zip" <<'PY'
import os, sys, zipfile
src, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
    for root, _dirs, files in os.walk(src):
        for name in files:
            path = os.path.join(root, name)
            arc = os.path.relpath(path, src)
            zf.write(path, arc)
PY
  fi
}

VERSION="$(manifest_version)"
CARGO_VER="$(cargo_version)"

if [[ -z "$VERSION" || "$VERSION" == "null" ]]; then
  die "could not read version from manifest.json"
fi
if [[ -z "$CARGO_VER" ]]; then
  die "could not read version from Cargo.toml"
fi
if [[ "$VERSION" != "$CARGO_VER" ]]; then
  die "version mismatch: manifest.json=$VERSION Cargo.toml=$CARGO_VER (keep them in sync; manifest.json is the release source of truth)"
fi

echo "==> Building wasm (release)"
wasm-pack build --target web --release

for f in jongo.js jongo_bg.wasm; do
  [[ -f "pkg/$f" ]] || die "missing pkg/$f after wasm-pack build"
done

echo "==> Assembling dist/chrome and dist/firefox"
rm -rf dist/chrome dist/firefox
mkdir -p dist/chrome/pkg dist/firefox/pkg release

RUNTIME_ROOT=(manifest.json background.js content_shim.js popup.html popup.js)
for f in "${RUNTIME_ROOT[@]}"; do
  [[ -f "$f" ]] || die "missing runtime file: $f"
  cp "$f" dist/chrome/
  cp "$f" dist/firefox/
done

cp pkg/jongo.js pkg/jongo_bg.wasm dist/chrome/pkg/
cp pkg/jongo.js pkg/jongo_bg.wasm dist/firefox/pkg/

# Firefox overlay: gecko id (Chrome copy stays as the source manifest)
apply_firefox_manifest dist/firefox/manifest.json dist/firefox/manifest.json.tmp
mv dist/firefox/manifest.json.tmp dist/firefox/manifest.json

CHROME_ZIP="$ROOT/release/jongo-${VERSION}-chrome.zip"
FIREFOX_ZIP="$ROOT/release/jongo-${VERSION}-firefox.zip"

echo "==> Writing $CHROME_ZIP"
zip_dir_contents "$ROOT/dist/chrome" "$CHROME_ZIP"

echo "==> Writing $FIREFOX_ZIP"
zip_dir_contents "$ROOT/dist/firefox" "$FIREFOX_ZIP"

echo
echo "Done."
echo "  Chrome unpacked:  $ROOT/dist/chrome"
echo "  Firefox unpacked: $ROOT/dist/firefox"
echo "  Chrome zip:       $CHROME_ZIP"
echo "  Firefox zip:      $FIREFOX_ZIP"
echo
echo "Test: load unpacked dist/chrome (Chrome) or dist/firefox/manifest.json (Firefox)."
echo "Check zip roots: unzip -l release/jongo-${VERSION}-chrome.zip | head"
