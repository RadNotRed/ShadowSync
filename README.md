# USB Mirror Sync

USB Mirror Sync is a cross-platform tray app for keeping selected folders from a removable USB drive mirrored onto a PC, with an optional shadow cache and manual or automatic push back to the drive.

## What It Does

- Detects a configured Windows drive letter such as `E:` or a mounted drive path on macOS/Linux
- Pulls from `USB -> shadow cache -> local target`
- Optionally pushes from `local target -> shadow cache -> USB`
- Skips unchanged files using a persistent manifest cache
- Watches mounted folders for live changes instead of doing timed full rescans
- Shows tray-based progress, setup, logs, and manual sync actions
- Supports Windows, macOS, and Linux release artifacts

## Documentation

- GitHub Pages docs: `https://radnotred.github.io/USBFileSync/`
- Local docs source: [`docs/`](docs/)
- Example config: [`config.example.json`](config.example.json)
- Contribution guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)

## Install

Release artifacts are produced automatically from the version in `Cargo.toml` for mainstream 64-bit desktop targets:

- `usb_mirror_sync-windows-x64-portable-v<version>.zip`
- `usb_mirror_sync-windows-x64-setup-v<version>.exe`
- `usb_mirror_sync-windows-arm64-portable-v<version>.zip`
- `usb_mirror_sync-windows-arm64-setup-v<version>.exe`
- `usb_mirror_sync-macos-x64-v<version>.dmg`
- `usb_mirror_sync-macos-x64-v<version>.tar.gz`
- `usb_mirror_sync-macos-arm64-v<version>.dmg`
- `usb_mirror_sync-macos-arm64-v<version>.tar.gz`
- `usb_mirror_sync-linux-x64-v<version>.tar.gz`
- `usb_mirror_sync-linux-arm64-v<version>.tar.gz`

Windows keeps the installer with optional desktop shortcut creation and optional run-at-startup registration. Startup is off by default. macOS now ships a drag-to-Applications `.dmg` plus a raw archive fallback. Linux ships per-architecture archives.

## Runtime Files

On first launch the app creates:

- `%LOCALAPPDATA%\UsbMirrorSync\config.json`
- `%LOCALAPPDATA%\UsbMirrorSync\manifest.json`
- `%LOCALAPPDATA%\UsbMirrorSync\shadow\`
- `%LOCALAPPDATA%\UsbMirrorSync\sync.log`

Use the tray menu `Setup Wizard` action for guided configuration, or edit `config.json` directly.

On Windows, set `drive.letter`. On macOS and Linux, set `drive.path` to the mounted USB root.

## Development

```powershell
cargo test
cargo build --release
```

For local docs work:

```powershell
python -m pip install -r requirements-docs.txt
python -m mkdocs serve
```
