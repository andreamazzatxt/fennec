#!/bin/bash
set -euo pipefail

# Release script for Fennec
# Creates signed updater artifacts and uploads to GitHub release

VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
BUNDLE_DIR="src-tauri/target/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/Fennec.app"
DMG_PATH="$BUNDLE_DIR/dmg/Fennec_${VERSION}_aarch64.dmg"
TAR_PATH="$BUNDLE_DIR/macos/Fennec.app.tar.gz"
SIG_PATH="$TAR_PATH.sig"
KEY_PATH="$HOME/.tauri/fennec.key"

echo "==> Releasing Fennec v${VERSION}"

# Check prerequisites
if [ ! -f "$APP_PATH/Contents/MacOS/fennec" ]; then
    echo "ERROR: App bundle not found. Run 'bun run tauri build' first."
    exit 1
fi

if [ ! -f "$KEY_PATH" ]; then
    echo "ERROR: Signing key not found at $KEY_PATH"
    exit 1
fi

# Create .tar.gz of the .app bundle
echo "==> Creating tar.gz..."
cd "$BUNDLE_DIR/macos"
COPYFILE_DISABLE=1 tar -czf Fennec.app.tar.gz Fennec.app
cd - > /dev/null

# Sign with Tauri CLI (uses rsign2 format)
echo "==> Signing with Tauri signer..."
TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_PATH")" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" bun run tauri signer sign "$TAR_PATH" 2>&1
if [ ! -f "$SIG_PATH" ]; then
    echo "ERROR: Signature file not created"
    exit 1
fi
SIGNATURE=$(cat "$SIG_PATH")
echo "==> Signed OK"

# Generate latest.json
echo "==> Generating latest.json..."
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")
cat > "$BUNDLE_DIR/latest.json" << EOF
{
  "version": "${VERSION}",
  "notes": "Fennec v${VERSION}",
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "darwin-aarch64": {
      "signature": "${SIGNATURE}",
      "url": "https://github.com/andreamazzatxt/fennec/releases/download/v${VERSION}/Fennec.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "${SIGNATURE}",
      "url": "https://github.com/andreamazzatxt/fennec/releases/download/v${VERSION}/Fennec.app.tar.gz"
    }
  }
}
EOF

echo "==> Uploading to GitHub release v${VERSION}..."

# Delete existing release if it exists, then recreate
gh release delete "v${VERSION}" --repo andreamazzatxt/fennec --yes 2>/dev/null || true
gh release create "v${VERSION}" \
    "$DMG_PATH" \
    "$TAR_PATH" \
    "$BUNDLE_DIR/latest.json" \
    --repo andreamazzatxt/fennec \
    --title "Fennec v${VERSION}" \
    --notes "Fennec v${VERSION}"

echo "==> Done! Release v${VERSION} published."
