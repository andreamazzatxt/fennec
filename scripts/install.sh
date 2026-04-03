#!/bin/bash
set -euo pipefail

echo "Installing Fennec..."

# Get latest version from GitHub
RELEASE_URL=$(curl -sI https://github.com/andreamazzatxt/fennec/releases/latest | grep -i "^location:" | tr -d '\r' | awk '{print $2}')
VERSION=$(basename "$RELEASE_URL")
DMG_URL="https://github.com/andreamazzatxt/fennec/releases/download/${VERSION}/Fennec_${VERSION#v}_aarch64.dmg"

echo "Downloading Fennec ${VERSION}..."
curl -L "$DMG_URL" -o /tmp/Fennec.dmg

echo "Mounting DMG..."
hdiutil attach /tmp/Fennec.dmg -nobrowse -quiet

echo "Installing to /Applications..."
# Remove old version if present
rm -rf /Applications/Fennec.app
cp -R "/Volumes/Fennec/Fennec.app" /Applications/
xattr -cr /Applications/Fennec.app 2>/dev/null || true

echo "Cleaning up..."
hdiutil detach "/Volumes/Fennec" -quiet
rm -f /tmp/Fennec.dmg

echo "Done! Opening Fennec..."
open /Applications/Fennec.app
