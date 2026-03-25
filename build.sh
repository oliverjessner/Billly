#!/bin/sh

set -eu

RELEASE_REPO="oliverjessner/Billly-Release"
CHANGELOG_URL="https://raw.githubusercontent.com/$RELEASE_REPO/main/changelog.md"
BUNDLE_DIR="src-tauri/target/release/bundle"
APP="$BUNDLE_DIR/macos/Billly.app"
DMG_DIR="$BUNDLE_DIR/dmg"
TMP_CHANGELOG=$(mktemp)
TMP_RELEASE_NOTES=$(mktemp)

cleanup() {
    rm -f "$TMP_CHANGELOG" "$TMP_RELEASE_NOTES"
}

trap cleanup EXIT

echo "Cleaning previous builds..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

echo "Building the app..."
npm run tauri build -- --bundles app

if [ ! -d "$APP" ]; then
    echo "App not found at $APP"
    exit 1
fi

node scripts/bundle-macos-dylibs.mjs "$APP"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

mkdir -p "$DMG_DIR"
VERSION=$(node -p 'require("./package.json").version || "0.0.0"')
TAG="v$VERSION"
OUT="$DMG_DIR/Billly_${VERSION}_aarch64_adhoc.dmg"
hdiutil create -volname "Billly" -srcfolder "$APP" -ov -format UDZO "$OUT"
echo "Created $OUT"

echo "Fetching changelog from $CHANGELOG_URL..."
curl -L --fail --silent --show-error "$CHANGELOG_URL" -o "$TMP_CHANGELOG"
awk -v version="$VERSION" '
    $0 == "# " version { capture=1 }
    capture && $0 ~ /^# / && $0 != "# " version { exit }
    capture { print }
' "$TMP_CHANGELOG" > "$TMP_RELEASE_NOTES"

if [ ! -s "$TMP_RELEASE_NOTES" ]; then
    cp "$TMP_CHANGELOG" "$TMP_RELEASE_NOTES"
fi

echo "Creating GitHub release $TAG on $RELEASE_REPO..."
gh release create "$TAG" "$OUT" \
    --repo "$RELEASE_REPO" \
    --title "Billly $TAG" \
    --notes-file "$TMP_RELEASE_NOTES"

echo "Build completed successfully."

echo "Opening the Billly release page"
open -a "Google Chrome" "https://github.com/$RELEASE_REPO/releases/tag/$TAG"
