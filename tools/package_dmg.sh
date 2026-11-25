#!/usr/bin/env bash
# Wraps the release binary and assets into "Rimefront.app" on dist/rimefront-<version>-<platform>.dmg (macOS).
# Usage: tools/package_dmg.sh <platform> [binary-path]
set -euo pipefail

platform="${1:?platform name, e.g. macos-arm64}"
binary="${2:-target/release/rimefront}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
name="rimefront-${version}-${platform}"
stage="dist/${name}-dmg"
app="${stage}/Rimefront.app"
icon_source="assets/icons/command_center.png"

rm -rf "${stage}" "dist/${name}.dmg"
mkdir -p "${app}/Contents/MacOS" "${app}/Contents/Resources"
cp "${binary}" "${app}/Contents/MacOS/rimefront"
# Bevy looks for assets next to the executable.
cp -R assets "${app}/Contents/MacOS/"
rm -rf "${app}/Contents/MacOS/assets/previews"

iconset="${stage}/AppIcon.iconset"
mkdir -p "${iconset}"
for size in 16 32 128 256; do
  sips -z "${size}" "${size}" "${icon_source}" --out "${iconset}/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  if [ "${double}" -le 256 ]; then
    sips -z "${double}" "${double}" "${icon_source}" --out "${iconset}/icon_${size}x${size}@2x.png" >/dev/null
  fi
done
iconutil -c icns "${iconset}" -o "${app}/Contents/Resources/AppIcon.icns"
rm -rf "${iconset}"

cat > "${app}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Rimefront</string>
  <key>CFBundleDisplayName</key><string>Rimefront</string>
  <key>CFBundleIdentifier</key><string>org.rimefront.game</string>
  <key>CFBundleExecutable</key><string>rimefront</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.strategy-games</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

tools/sign_macos.sh app "${app}"
# Bridge release: installations from before the rename update themselves from "Iron Tide.app" on the image.
# Drop with the release after the bridge (docs/rename.md).
cp -R "${app}" "${stage}/Iron Tide.app"

ln -s /Applications "${stage}/Applications"
hdiutil create -quiet -volname "Rimefront" -srcfolder "${stage}" -ov -format UDZO "dist/${name}.dmg"
rm -rf "${stage}"
tools/sign_macos.sh dmg "dist/${name}.dmg"
ls -lh "dist/${name}.dmg"
