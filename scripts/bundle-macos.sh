#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE="debug"
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --release) PROFILE="release" ;;
    --install) INSTALL=1; PROFILE="release" ;;
    *) echo "uso: $0 [--release] [--install]" >&2; exit 1 ;;
  esac
done

if [[ "$PROFILE" == "release" ]]; then
  cargo build --release
else
  cargo build
fi

APP="target/$PROFILE/Kirei.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/$PROFILE/kirei" "$APP/Contents/MacOS/Kirei"

ICON="target/AppIcon.icns"
if [[ ! -f "$ICON" || assets/icon.png -nt "$ICON" ]]; then
  ICONSET="target/AppIcon.iconset"
  mkdir -p "$ICONSET"
  for s in 16 32 128 256 512; do
    sips -z $s $s assets/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z $d $d assets/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$ICON"
fi
cp "$ICON" "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>com.shiddo.kirei</string>
  <key>CFBundleName</key><string>Kirei</string>
  <key>CFBundleDisplayName</key><string>Kirei</string>
  <key>CFBundleExecutable</key><string>Kirei</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSScreenCaptureUsageDescription</key><string>O Kirei captura a tela para criar suas imagens.</string>
</dict>
</plist>
PLIST

# Com o mesmo certificado e bundle id, a permissão de Gravação de Tela sobrevive a recompilações.
SIGN_ID="Kirei Local Signing"
if security find-identity -p codesigning | grep -q "$SIGN_ID"; then
  codesign --force --sign "$SIGN_ID" "$APP"
else
  echo "aviso: sem o certificado \"$SIGN_ID\", assinando ad-hoc (o macOS pede a permissão a cada build)" >&2
  codesign --force --sign - "$APP"
fi

if [[ "$INSTALL" == 1 ]]; then
  pkill -x Kirei 2>/dev/null || true
  rm -rf /Applications/Kirei.app
  cp -R "$APP" /Applications/Kirei.app
  echo "/Applications/Kirei.app"
else
  echo "$APP"
fi
