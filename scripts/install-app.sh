#!/usr/bin/env bash
# Build Image Viewer.app locally and install to /Applications (personal use).
# Does not require a bundler — assembles a minimal .app from the release binary.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# Local Network privacy (macOS 15+) tracks identity by code signature (TN3179).
# Prefer an Apple-issued identity so the grant survives rebuilds; otherwise ad-hoc.
# The same signature is what allows WKWebView to use the network.
sign_app() {
  local app="$1"
  local identity=""
  identity="$(security find-identity -v -p codesigning 2>/dev/null | awk '/Developer ID Application/ { print $2; exit }')"
  if [[ -z "$identity" ]]; then
    identity="$(security find-identity -v -p codesigning 2>/dev/null | awk '/Apple Development/ { print $2; exit }')"
  fi
  if [[ -n "$identity" ]]; then
    echo "Signing with ${identity}..."
    if ! codesign --force --deep --sign "$identity" --identifier local.image-viewer "$app"; then
      echo "warning: codesign failed; network permission may not stick" >&2
    fi
    return
  fi
  echo "Ad-hoc signing (no Apple code-signing identity found)..."
  if ! codesign --force --deep --sign - --identifier local.image-viewer "$app"; then
    echo "warning: ad-hoc codesign failed; network permission may not stick" >&2
  fi
}

echo "Updating dependencies..."
cargo update

echo "Building release binary..."
cargo build --release

BIN="$CARGO_TARGET_DIR/release/image-viewer"
APP_DIR="$CARGO_TARGET_DIR/release/bundle/osx/Image Viewer.app"
CONTENTS="$APP_DIR/Contents"
MACOS="$CONTENTS/MacOS"
RES="$CONTENTS/Resources"

rm -rf "$APP_DIR"
mkdir -p "$MACOS" "$RES/models"
cp "$BIN" "$MACOS/image-viewer"
chmod +x "$MACOS/image-viewer"

"$ROOT/scripts/make-app-icon.sh"
ICON="$ROOT/assets/AppIcon.icns"
if [[ ! -f "$ICON" ]]; then
  echo "Missing app icon: $ICON" >&2
  exit 1
fi
cp "$ICON" "$RES/AppIcon.icns"

for asset in folder.png loading.png video.png; do
  if [[ ! -f "$ROOT/assets/$asset" ]]; then
    echo "Missing asset: assets/$asset" >&2
    exit 1
  fi
  cp "$ROOT/assets/$asset" "$RES/$asset"
done

# Models are optional at first. Copy whichever ONNX files exist.
if compgen -G "$ROOT/assets/models/*.onnx" > /dev/null; then
  cp "$ROOT/assets/models/"*.onnx "$RES/models/"
fi

# cargo pkgid is `path+file:///…/image-viewer#1.0.1` (or older `…@1.0.1`).
pkgid="$(cargo pkgid -p image-viewer)"
APP_VERSION="${pkgid##*#}"
APP_VERSION="${APP_VERSION##*@}"

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>image-viewer</string>
  <key>CFBundleIdentifier</key>
  <string>local.image-viewer</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Image Viewer</string>
  <key>CFBundleDisplayName</key>
  <string>Image Viewer</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${APP_VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${APP_VERSION}</string>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright © 2026 Steve</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSQuitAlwaysKeepsWindows</key>
  <false/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key>
      <string>JPEG image</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>jpeg</string>
        <string>jpg</string>
      </array>
      <key>LSItemContentTypes</key>
      <array>
        <string>public.jpeg</string>
      </array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key>
      <string>PNG image</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>png</string>
      </array>
      <key>LSItemContentTypes</key>
      <array>
        <string>public.png</string>
      </array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key>
      <string>GIF image</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>gif</string>
      </array>
      <key>LSItemContentTypes</key>
      <array>
        <string>com.compuserve.gif</string>
      </array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key>
      <string>WEBP image</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>webp</string>
      </array>
      <key>LSItemContentTypes</key>
      <array>
        <string>org.webmproject.webp</string>
      </array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key>
      <string>MPEG-4 video</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>mp4</string>
      </array>
      <key>LSItemContentTypes</key>
      <array>
        <string>public.mpeg-4</string>
      </array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key>
      <string>QuickTime movie</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>mov</string>
      </array>
      <key>LSItemContentTypes</key>
      <array>
        <string>com.apple.quicktime-movie</string>
      </array>
    </dict>
  </array>
</dict>
</plist>
PLIST

echo "Assembled: $APP_DIR"

DEST="/Applications/Image Viewer.app"
echo "Installing to ${DEST}..."
rm -rf "$DEST"
cp -R "$APP_DIR" "$DEST"
touch "$DEST"
sign_app "$DEST"
echo "Installed to ${DEST}"
