#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:?repo root is required}"
target_triple="${2:?target triple is required}"
arch_label="${3:?arch label is required}"

version="$(python -c "import pathlib,re; text=pathlib.Path(r'${repo_root}/Cargo.toml').read_text(); print(re.search(r'(?s)\[package\].*?version\s*=\s*\"([^\"]+)\"', text).group(1))")"
release_dir="${repo_root}/target/${target_triple}/release"
binary_path="${release_dir}/usb_mirror_sync"
generated_dir="${repo_root}/target/generated-assets/${target_triple}"
stage_root="${release_dir}/package-macos-${arch_label}"
bundle_name="USB Mirror Sync.app"
bundle_root="${stage_root}/${bundle_name}"
contents_root="${bundle_root}/Contents"
macos_root="${contents_root}/MacOS"
resources_root="${contents_root}/Resources"
iconset_root="${stage_root}/usb_mirror_sync.iconset"
tar_artifact="${release_dir}/usb_mirror_sync-macos-${arch_label}-v${version}.tar.gz"
dmg_artifact="${release_dir}/usb_mirror_sync-macos-${arch_label}-v${version}.dmg"

rm -rf "${stage_root}"
mkdir -p "${macos_root}" "${resources_root}" "${iconset_root}"

cp "${binary_path}" "${macos_root}/usb_mirror_sync"
chmod +x "${macos_root}/usb_mirror_sync"

cat > "${contents_root}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>USB Mirror Sync</string>
  <key>CFBundleExecutable</key>
  <string>usb_mirror_sync</string>
  <key>CFBundleIconFile</key>
  <string>usb_mirror_sync.icns</string>
  <key>CFBundleIdentifier</key>
  <string>com.rad.usbmirrorsync</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>USB Mirror Sync</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

cp "${generated_dir}/icon_16x16.png" "${iconset_root}/icon_16x16.png"
cp "${generated_dir}/icon_32x32.png" "${iconset_root}/icon_16x16@2x.png"
cp "${generated_dir}/icon_32x32.png" "${iconset_root}/icon_32x32.png"
cp "${generated_dir}/icon_64x64.png" "${iconset_root}/icon_32x32@2x.png"
cp "${generated_dir}/icon_128x128.png" "${iconset_root}/icon_128x128.png"
cp "${generated_dir}/icon_256x256.png" "${iconset_root}/icon_128x128@2x.png"
cp "${generated_dir}/icon_256x256.png" "${iconset_root}/icon_256x256.png"
cp "${generated_dir}/icon_512x512.png" "${iconset_root}/icon_256x256@2x.png"
cp "${generated_dir}/icon_512x512.png" "${iconset_root}/icon_512x512.png"
cp "${generated_dir}/icon_1024x1024.png" "${iconset_root}/icon_512x512@2x.png"
iconutil -c icns "${iconset_root}" -o "${resources_root}/usb_mirror_sync.icns"

if [[ -n "${MACOS_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --deep --options runtime --sign "${MACOS_SIGNING_IDENTITY}" "${bundle_root}"
fi

package_root="${stage_root}/package-root"
mkdir -p "${package_root}"
cp -R "${bundle_root}" "${package_root}/"
cp "${repo_root}/README.md" "${package_root}/README.md"
cp "${repo_root}/config.example.json" "${package_root}/config.example.json"
tar -czf "${tar_artifact}" -C "${package_root}" .

dmg_root="${stage_root}/dmg-root"
mkdir -p "${dmg_root}"
cp -R "${bundle_root}" "${dmg_root}/"
cp "${repo_root}/README.md" "${dmg_root}/README.md"
cp "${repo_root}/config.example.json" "${dmg_root}/config.example.json"
ln -s /Applications "${dmg_root}/Applications"

hdiutil create -volname "USB Mirror Sync" -srcfolder "${dmg_root}" -ov -format UDZO "${dmg_artifact}"
hdiutil verify "${dmg_artifact}"

if [[ -n "${MACOS_SIGNING_IDENTITY:-}" && -n "${MACOS_NOTARY_APPLE_ID:-}" && -n "${MACOS_NOTARY_TEAM_ID:-}" && -n "${MACOS_NOTARY_APP_PASSWORD:-}" ]]; then
  xcrun notarytool store-credentials "usb-mirror-sync-notary" \
    --apple-id "${MACOS_NOTARY_APPLE_ID}" \
    --team-id "${MACOS_NOTARY_TEAM_ID}" \
    --password "${MACOS_NOTARY_APP_PASSWORD}"
  xcrun notarytool submit "${dmg_artifact}" --keychain-profile "usb-mirror-sync-notary" --wait
  xcrun stapler staple "${dmg_artifact}"
fi
