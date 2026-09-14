#!/usr/bin/env bash
# Build the Windows installer on this Mac and publish it as a GitHub Release,
# including the latest.json that installed copies poll for updates.
#
# Usage: npm run release:windows -- 0.2.0   (bumps both version files, commits, builds, publishes)
#        npm run release:windows            (uses the version already in the config files)
# Needs: brew install nsis llvm; cargo install cargo-xwin; rustup target add x86_64-pc-windows-msvc;
#        gh auth login; ~/.tauri/liquorpos.key
set -euo pipefail
cd "$(dirname "$0")/.."

# Always push with the GitHub CLI's active login, never an older account cached in the keychain.
gitpush() { git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -q "$@"; }

export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/llvm/bin:$PATH"
KEY_FILE="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/liquorpos.key}"
[ -f "$KEY_FILE" ] || { echo "Signing key not found at $KEY_FILE" >&2; exit 1; }
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

# Optional: pass the new version as the first argument to bump both config files and commit.
if [ -n "${1:-}" ]; then
  NEW="$1"
  [[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Version must look like 0.2.0" >&2; exit 1; }
  node -e '
const fs = require("fs"); const v = process.argv[1];
for (const f of ["package.json", "src-tauri/tauri.conf.json"]) {
  const j = JSON.parse(fs.readFileSync(f)); j.version = v; fs.writeFileSync(f, JSON.stringify(j, null, 2) + "\n");
}' "$NEW"
  git add package.json src-tauri/tauri.conf.json
  git commit -q -m "Release v$NEW" || true
  gitpush origin HEAD
fi

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
gitpush origin "$TAG" --force
if gh release view "$TAG" >/dev/null 2>&1; then
  gh release upload "$TAG" "$EXE" "$SIG" "$OUT/latest.json" --clobber
else
  gh release create "$TAG" "$EXE" "$SIG" "$OUT/latest.json" \
    --title "Liquor POS $TAG" \
    --notes "Download the .exe below to install. Installed copies update themselves on next launch."
fi
echo "==> Done: https://github.com/$REPO/releases/tag/$TAG"
