#!/bin/bash
# Builds Kirei.app. Usage: scripts/build.sh [--install] [--dmg]
#
# Distribution (optional, via environment):
#   DEVELOPER_ID="Developer ID Application: Name (TEAMID)"  signs with hardened runtime for Gatekeeper
#   NOTARY_PROFILE="kirei"  notarizes and staples the DMG (create it once with
#                           `xcrun notarytool store-credentials kirei --apple-id … --team-id …`)
set -euo pipefail
cd "$(dirname "$0")/.."

INSTALL=0; DMG=0
for arg in "$@"; do
  case "$arg" in
    --install) INSTALL=1 ;;
    --dmg) DMG=1 ;;
    *) echo "Opção desconhecida: $arg"; exit 1 ;;
  esac
done

APP="build/Kirei.app"
VERSION="$(cat VERSION 2>/dev/null || echo 1.0.0)"
BUILD="$(date +%Y%m%d%H%M)"

echo "→ Compilando (release)…"
swift build -c release 2>&1 | grep -vE "^\[" || true
BIN=".build/release/Kirei"

echo "→ Montando ${APP}…"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Kirei"

if [ ! -f build/AppIcon.icns ] || [ scripts/make-icon.swift -nt build/AppIcon.icns ]; then
  echo "→ Gerando ícone…"
  ICONSET="build/AppIcon.iconset"
  rm -rf "$ICONSET" && mkdir -p "$ICONSET"
  swift scripts/make-icon.swift build/icon-1024.png
  for s in 16 32 128 256 512; do
    sips -z $s $s build/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z $d $d build/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o build/AppIcon.icns
fi
cp build/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

# Translations (en, pt-BR, es). pt.lproj covers Portuguese outside Brazil.
cp -R Localization/*.lproj "$APP/Contents/Resources/"
cp -R Localization/pt-BR.lproj "$APP/Contents/Resources/pt.lproj"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Kirei</string>
  <key>CFBundleDisplayName</key><string>Kirei</string>
  <key>CFBundleIdentifier</key><string>com.shiddo.kirei</string>
  <key>CFBundleExecutable</key><string>Kirei</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${BUILD}</string>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleLocalizations</key>
  <array><string>en</string><string>pt-BR</string><string>pt</string><string>es</string></array>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>© 2026 Shiddo</string>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Image</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>public.image</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

# Developer ID + hardened runtime is what Gatekeeper accepts on other Macs. Without it, a stable
# local certificate keeps the Screen Recording permission across rebuilds (ad-hoc changes every build).
SIGN_ID="Kirei Local Signing"
if [ -n "${DEVELOPER_ID:-}" ]; then
  echo "→ Assinando com Developer ID (hardened runtime)…"
  codesign --force --options runtime --timestamp --sign "$DEVELOPER_ID" "$APP"
elif security find-identity -p codesigning | grep -q "$SIGN_ID"; then
  echo "→ Assinando com \"$SIGN_ID\"…"
  codesign --force --sign "$SIGN_ID" "$APP"
else
  echo "→ Assinando (ad-hoc; a permissão de Gravação de Tela será pedida a cada build)…"
  codesign --force --sign - "$APP"
fi
codesign --verify --strict "$APP"

if [ "$DMG" = 1 ]; then
  DMG_PATH="build/Kirei-${VERSION}.dmg"
  echo "→ Gerando ${DMG_PATH}…"
  STAGE="build/dmg"
  RW="build/Kirei-rw.dmg"
  VOL="Kirei ${VERSION}"
  rm -rf "$STAGE" "$DMG_PATH" "$RW"
  mkdir -p "$STAGE/.background"
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"

  # Window background: 1x + 2x in one TIFF so Finder picks the right one on Retina.
  if [ ! -f build/dmg-background.tiff ] || [ scripts/make-dmg-background.swift -nt build/dmg-background.tiff ]; then
    swift scripts/make-dmg-background.swift build/dmg-background@2x.png
    sips -z 400 660 -s dpiWidth 72 -s dpiHeight 72 build/dmg-background@2x.png --out build/dmg-background.png >/dev/null
    tiffutil -cathidpicheck build/dmg-background.png build/dmg-background@2x.png -out build/dmg-background.tiff >/dev/null
  fi
  cp build/dmg-background.tiff "$STAGE/.background/background.tiff"

  hdiutil create -volname "$VOL" -srcfolder "$STAGE" -ov -format UDRW -fs HFS+ "$RW" >/dev/null
  rm -rf "$STAGE"
  hdiutil detach "/Volumes/$VOL" -quiet 2>/dev/null || true
  DEV="$(hdiutil attach -readwrite -noverify -noautoopen "$RW" | grep -E '^/dev/' | head -1 | awk '{print $1}')"
  # Finder lays out the window (needs Automation permission for Finder once; without it the DMG still works).
  osascript <<APPLESCRIPT 2>&1 || echo "  (aviso: não deu para ajustar a janela do DMG no Finder; seguindo sem o layout)"
tell application "Finder"
  tell disk "$VOL"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {200, 120, 860, 520}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 112
    set text size of opts to 13
    set background picture of opts to file ".background:background.tiff"
    set position of item "Kirei.app" of container window to {170, 190}
    set position of item "Applications" of container window to {490, 190}
    update without registering applications
    delay 1
    close
  end tell
end tell
APPLESCRIPT
  sync
  hdiutil detach "$DEV" -quiet || hdiutil detach "$DEV" -force -quiet
  hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH" >/dev/null
  rm -f "$RW"
  if [ -n "${DEVELOPER_ID:-}" ]; then
    codesign --force --timestamp --sign "$DEVELOPER_ID" "$DMG_PATH"
  fi
  if [ -n "${NOTARY_PROFILE:-}" ]; then
    [ -n "${DEVELOPER_ID:-}" ] || { echo "✗ A notarização exige DEVELOPER_ID."; exit 1; }
    echo "→ Enviando para notarização (pode levar alguns minutos)…"
    xcrun notarytool submit "$DMG_PATH" --keychain-profile "$NOTARY_PROFILE" --wait
    xcrun stapler staple "$DMG_PATH"
  fi
  echo "✓ DMG: $DMG_PATH"
fi

if [ "$INSTALL" = 1 ]; then
  echo "→ Instalando em /Applications…"
  pkill -x Kirei 2>/dev/null || true
  rm -rf /Applications/Kirei.app
  cp -R "$APP" /Applications/
  echo "✓ Instalado: /Applications/Kirei.app (${VERSION} build ${BUILD})"
else
  echo "✓ Pronto: $APP (${VERSION} build ${BUILD})"
fi
