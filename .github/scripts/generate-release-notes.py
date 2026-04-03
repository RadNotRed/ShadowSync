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


def friendly_platform(platform: str) -> str:
    return {
        "windows": "Windows",
        "macos": "macOS",
        "linux": "Linux",
        "other": "Other",
    }.get(platform, platform)


def friendly_arch(arch: str) -> str:
    return {
        "x64": "x64",
        "arm64": "ARM64",
        "other": "other",
    }.get(arch, arch)


def friendly_format(kind: str) -> str:
    return {
        "portable": "Portable",
        "setup": "Installer",
        "dmg": "DMG",
        "archive": "Archive",
    }.get(kind, kind.capitalize())


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


def download_url(repo: str, tag: str, asset: Path) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{asset.name}"


def build_quick_install(repo: str, tag: str, assets: list[Path]) -> str:
    grouped: dict[tuple[str, str, str], Path] = {}
    for asset in assets:
        grouped[parse_asset_name(asset.name)] = asset

    lines = [
        "## Quick install",
        "",
        "| Platform | Recommended | Alternate |",
        "| --- | --- | --- |",
    ]

    windows_x64_setup = grouped.get(("windows", "x64", "setup"))
    windows_x64_portable = grouped.get(("windows", "x64", "portable"))
    windows_arm64_setup = grouped.get(("windows", "arm64", "setup"))
    windows_arm64_portable = grouped.get(("windows", "arm64", "portable"))
    macos_x64_dmg = grouped.get(("macos", "x64", "dmg"))
    macos_x64_archive = grouped.get(("macos", "x64", "archive"))
    macos_arm64_dmg = grouped.get(("macos", "arm64", "dmg"))
    macos_arm64_archive = grouped.get(("macos", "arm64", "archive"))
    linux_x64_archive = grouped.get(("linux", "x64", "archive"))
    linux_arm64_archive = grouped.get(("linux", "arm64", "archive"))

    def link(asset: Path | None) -> str:
        if asset is None:
            return "Not available"
        return f"[{asset.name}]({download_url(repo, tag, asset)})"

    lines.append(
        f"| Windows x64 | {link(windows_x64_setup)} | {link(windows_x64_portable)} |"
    )
    lines.append(
        f"| Windows ARM64 | {link(windows_arm64_setup)} | {link(windows_arm64_portable)} |"
    )
    lines.append(
        f"| macOS x64 | {link(macos_x64_dmg)} | {link(macos_x64_archive)} |"
    )
    lines.append(
        f"| macOS ARM64 | {link(macos_arm64_dmg)} | {link(macos_arm64_archive)} |"
    )
    lines.append(
        f"| Linux x64 | {link(linux_x64_archive)} | Extract and run `shadowsync` |"
    )
    lines.append(
        f"| Linux ARM64 | {link(linux_arm64_archive)} | Extract and run `shadowsync` |"
    )
    return "\n".join(lines)


def build_download_table(repo: str, tag: str, assets: list[Path]) -> str:
    lines = [
        "## Downloads",
        "",
        "| Platform | Format | File | Size | SHA-256 |",
        "| --- | --- | --- | ---: | --- |",
    ]

    for asset in assets:
        platform, arch, artifact_kind = parse_asset_name(asset.name)
        platform_name = friendly_platform(platform)
        arch_name = friendly_arch(arch)
        platform_label = f"{platform_name} {arch_name}" if platform != "other" else "Other"
        format_label = friendly_format(artifact_kind)
        lines.append(
            "| {platform} | {fmt} | [{name}]({url}) | {size} | `{sha}` |".format(
                platform=platform_label,
                fmt=format_label,
                name=asset.name,
                url=download_url(repo, tag, asset),
                size=format_size(asset.stat().st_size),
                sha=sha256_for(asset),
            )
        )

    return "\n".join(lines)


def build_verification_section(assets: list[Path]) -> str:
    lines = [
        "## Verify downloads",
        "",
        "Use the SHA-256 values above to confirm the file you downloaded matches the published artifact.",
        "",
        "```bash",
        "# macOS / Linux",
        "shasum -a 256 <filename>",
        "",
        "# Windows PowerShell",
        "Get-FileHash <filename> -Algorithm SHA256",
        "```",
    ]
    if assets:
        lines.extend(
            [
                "",
                "Published files in this release:",
                "",
            ]
        )
        for asset in assets:
            lines.append(f"- `{asset.name}`")
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
    quick_install = build_quick_install(args.repo, args.tag, assets)
    download_table = build_download_table(args.repo, args.tag, assets)
    verification = build_verification_section(assets)

    final_notes = existing_notes
    if final_notes:
        final_notes += "\n\n"
    final_notes += quick_install + "\n\n" + download_table + "\n\n" + verification + "\n"
    notes_path.write_text(final_notes, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
