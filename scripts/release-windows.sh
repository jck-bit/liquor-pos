#!/usr/bin/env bash
# Build the Windows installer on this Mac and publish it as a GitHub Release,
# including the latest.json that installed copies poll for updates.
#
# Usage: npm run release:windows
# Needs: brew install nsis llvm; cargo install cargo-xwin; rustup target add x86_64-pc-windows-msvc;
#        gh auth login; ~/.tauri/liquorpos.key
set -euo pipefail
cd "$(dirname "$0")/.."

export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/llvm/bin:$PATH"
export TAURI_SIGNING_PRIVATE_KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/liquorpos.key}"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
PKG_VERSION=$(node -p "require('./package.json').version")
if [ "$VERSION" != "$PKG_VERSION" ]; then
  echo "package.json ($PKG_VERSION) and tauri.conf.json ($VERSION) versions differ. Make them match first." >&2
  exit 1
fi
REPO=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
TAG="v$VERSION"

echo "==> Building Liquor POS $VERSION for Windows"
npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis

BUNDLE_DIR="src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis"
EXE=$(ls "$BUNDLE_DIR"/*-setup.exe | head -1)
SIG="$EXE.sig"
[ -f "$SIG" ] || { echo "Missing signature $SIG (is bundle.createUpdaterArtifacts true?)" >&2; exit 1; }
EXE_NAME=$(basename "$EXE")

echo "==> Writing latest.json"
OUT=$(mktemp -d)
node -e '
const [version, sig, url] = process.argv.slice(1);
process.stdout.write(JSON.stringify({
  version,
  notes: "Liquor POS " + version,
  pub_date: new Date().toISOString(),
  platforms: { "windows-x86_64": { signature: sig, url } },
}, null, 2));
' "$VERSION" "$(cat "$SIG")" "https://github.com/$REPO/releases/download/$TAG/${EXE_NAME// /.}" > "$OUT/latest.json"

echo "==> Publishing release $TAG on $REPO"
git tag -f "$TAG" >/dev/null
git push -q origin "$TAG" --force
if gh release view "$TAG" >/dev/null 2>&1; then
  gh release upload "$TAG" "$EXE" "$SIG" "$OUT/latest.json" --clobber
else
  gh release create "$TAG" "$EXE" "$SIG" "$OUT/latest.json" \
    --title "Liquor POS $TAG" \
    --notes "Download the .exe below to install. Installed copies update themselves on next launch."
fi
echo "==> Done: https://github.com/$REPO/releases/tag/$TAG"
