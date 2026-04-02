#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:?repo root is required}"
target_triple="${2:?target triple is required}"
arch_label="${3:?arch label is required}"

version="$(python -c "import pathlib,re; text=pathlib.Path(r'${repo_root}/Cargo.toml').read_text(); print(re.search(r'(?s)\[package\].*?version\s*=\s*\"([^\"]+)\"', text).group(1))")"
release_dir="${repo_root}/target/${target_triple}/release"
binary_path="${release_dir}/shadowsync"
stage_root="${release_dir}/package-linux-${arch_label}"
artifact="${release_dir}/shadowsync-linux-${arch_label}-v${version}.tar.gz"

rm -rf "${stage_root}"
mkdir -p "${stage_root}/integrations"

cp "${binary_path}" "${stage_root}/shadowsync"
chmod +x "${stage_root}/shadowsync"
cp "${repo_root}/README.md" "${stage_root}/README.md"
cp "${repo_root}/config.example.json" "${stage_root}/config.example.json"
cp "${repo_root}/.github/assets/icon.svg" "${stage_root}/integrations/shadowsync.svg"

cat > "${stage_root}/integrations/usb-mirror-sync.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=ShadowSync
Comment=Mirror folders between a USB drive and local storage
Exec=shadowsync
Icon=shadowsync
Terminal=false
Categories=Utility;FileTools;
EOF

tar -czf "${artifact}" -C "${stage_root}" .
