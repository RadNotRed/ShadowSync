#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


ORDER = {
    ("windows", "x64", "portable"): 0,
    ("windows", "x64", "setup"): 1,
    ("windows", "arm64", "portable"): 2,
    ("windows", "arm64", "setup"): 3,
    ("macos", "x64", "dmg"): 4,
    ("macos", "x64", "archive"): 5,
    ("macos", "arm64", "dmg"): 6,
    ("macos", "arm64", "archive"): 7,
    ("linux", "x64", "archive"): 8,
    ("linux", "arm64", "archive"): 9,
}


def sha256_for(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def format_size(size: int) -> str:
    units = ["B", "KB", "MB", "GB"]
    value = float(size)
    for unit in units:
        if value < 1024 or unit == units[-1]:
            if unit == "B":
                return f"{int(value)} {unit}"
            return f"{value:.2f} {unit}"
        value /= 1024
    return f"{size} B"


def parse_asset_name(filename: str) -> tuple[str, str, str]:
    if not filename.startswith("shadowsync-"):
        return ("other", "other", "other")

    base = filename
    if base.endswith(".tar.gz"):
        base = base[:-7]
        extension = "archive"
    else:
        base = Path(base).suffix.lstrip(".")
        base = filename[: -(len(base) + 1)]
        extension = "setup" if filename.endswith(".exe") else "portable" if filename.endswith(".zip") else "dmg"

    parts = base.split("-")
    if len(parts) < 4:
        return ("other", "other", extension)

    platform = parts[1]
    arch = parts[2]
    if platform in {"linux", "macos"}:
        artifact_kind = "archive" if extension == "archive" else extension
    else:
        artifact_kind = parts[3]
    return (platform, arch, artifact_kind)


def iter_assets(root: Path) -> list[Path]:
    return sorted(
        [path for path in root.rglob("*") if path.is_file() and path.name.startswith("shadowsync-")],
        key=lambda path: ORDER.get(parse_asset_name(path.name), 999),
    )


def build_download_table(repo: str, tag: str, assets: list[Path]) -> str:
    lines = [
        "## Downloads",
        "",
        "| Platform | Format | File | Size | SHA-256 |",
        "| --- | --- | --- | ---: | --- |",
    ]

    for asset in assets:
        platform, arch, artifact_kind = parse_asset_name(asset.name)
        download_url = f"https://github.com/{repo}/releases/download/{tag}/{asset.name}"
        platform_name = {
            "windows": "Windows",
            "macos": "macOS",
            "linux": "Linux",
            "other": "Other",
        }.get(platform, platform)
        arch_name = {
            "x64": "x64",
            "arm64": "ARM64",
            "other": "other",
        }.get(arch, arch)
        platform_label = f"{platform_name} {arch_name}" if platform != "other" else "Other"
        format_label = {
            "portable": "Portable",
            "setup": "Installer",
            "dmg": "DMG",
            "archive": "Archive",
        }.get(artifact_kind, artifact_kind.capitalize())
        lines.append(
            "| {platform} | {fmt} | [{name}]({url}) | {size} | `{sha}` |".format(
                platform=platform_label,
                fmt=format_label,
                name=asset.name,
                url=download_url,
                size=format_size(asset.stat().st_size),
                sha=sha256_for(asset),
            )
        )

    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--artifacts-dir", required=True)
    parser.add_argument("--notes-file", required=True)
    args = parser.parse_args()

    notes_path = Path(args.notes_file)
    existing_notes = notes_path.read_text(encoding="utf-8").rstrip()
    assets = iter_assets(Path(args.artifacts_dir))
    download_table = build_download_table(args.repo, args.tag, assets)

    final_notes = existing_notes
    if final_notes:
        final_notes += "\n\n"
    final_notes += download_table + "\n"
    notes_path.write_text(final_notes, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
