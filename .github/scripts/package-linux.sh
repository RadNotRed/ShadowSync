#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:?repo root is required}"
target_triple="${2:?target triple is required}"
arch_label="${3:?arch label is required}"

version="$(python -c "import pathlib,re; text=pathlib.Path(r'${repo_root}/Cargo.toml').read_text(); print(re.search(r'(?s)\[package\].*?version\s*=\s*\"([^\"]+)\"', text).group(1))")"
release_dir="${repo_root}/target/${target_triple}/release"
binary_path="${release_dir}/usb_mirror_sync"
stage_root="${release_dir}/package-linux-${arch_label}"
artifact="${release_dir}/usb_mirror_sync-linux-${arch_label}-v${version}.tar.gz"

rm -rf "${stage_root}"
mkdir -p "${stage_root}/integrations"

cp "${binary_path}" "${stage_root}/usb_mirror_sync"
chmod +x "${stage_root}/usb_mirror_sync"
cp "${repo_root}/README.md" "${stage_root}/README.md"
cp "${repo_root}/config.example.json" "${stage_root}/config.example.json"
cp "${repo_root}/.github/assets/icon.svg" "${stage_root}/integrations/usb-mirror-sync.svg"

cat > "${stage_root}/integrations/usb-mirror-sync.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=USB Mirror Sync
Comment=Mirror folders between a USB drive and local storage
Exec=usb_mirror_sync
Icon=usb-mirror-sync
Terminal=false
Categories=Utility;FileTools;
EOF

tar -czf "${artifact}" -C "${stage_root}" .
