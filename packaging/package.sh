#!/bin/zsh
# Builds "Didia Keeper.app" and a zip to share: dist/Didia-Keeper-<version>.zip
# Usage: packaging/package.sh [signing identity]   (default: ad-hoc "-")
#   With a Developer ID identity it also notarizes and staples, using the notarytool
#   keychain profile in $NOTARY_PROFILE (default "didia"; create it once with
#   xcrun notarytool store-credentials didia --key AuthKey_X.p8 --key-id X --issuer <issuer>).
set -e
cd "$(dirname "$0")/.."
ID=${1:--}
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build --release
[[ -f packaging/AppIcon.icns ]] || uv run -q --with pillow python packaging/make_icon.py
APP="dist/Didia Keeper.app"
rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/didia-keeper "$APP/Contents/MacOS/didia-keeper"
cp packaging/AppIcon.icns "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Didia Keeper</string>
<key>CFBundleDisplayName</key><string>Didia Keeper</string>
<key>CFBundleIdentifier</key><string>io.github.fais649.didia-keeper</string>
<key>CFBundleExecutable</key><string>didia-keeper</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$VER</string>
<key>CFBundleVersion</key><string>$VER</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>LSArchitecturePriority</key><array><string>arm64</string></array>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PL
if [[ $ID == - ]]; then
  codesign --force --sign - "$APP"
else
  codesign --force --options runtime --timestamp --sign "$ID" "$APP"
fi
codesign --verify --strict "$APP"
(cd dist && ditto -c -k --keepParent "Didia Keeper.app" "Didia-Keeper-$VER.zip")
if [[ $ID == "Developer ID Application:"* ]]; then
  xcrun notarytool submit "dist/Didia-Keeper-$VER.zip" --keychain-profile "${NOTARY_PROFILE:-didia}" --wait
  xcrun stapler staple "$APP"
  rm "dist/Didia-Keeper-$VER.zip"
  (cd dist && ditto -c -k --keepParent "Didia Keeper.app" "Didia-Keeper-$VER.zip")
  spctl -a -t exec "$APP"
fi
echo "dist/Didia-Keeper-$VER.zip ($(du -h "dist/Didia-Keeper-$VER.zip" | cut -f1))"
