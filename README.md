# USB Mirror Sync

USB Mirror Sync is a Windows tray app for keeping selected folders from a removable USB drive mirrored onto a PC, with an optional shadow cache and manual or automatic push back to the drive.

## What It Does

- Detects a configured USB drive letter such as `E:` or `S:`
- Pulls from `USB -> shadow cache -> local target`
- Optionally pushes from `local target -> shadow cache -> USB`
- Skips unchanged files using a persistent manifest cache
- Watches mounted folders for live changes instead of doing timed full rescans
- Shows tray-based progress, setup, logs, and manual sync actions
- Supports both portable and installer-based releases

## Documentation

- GitHub Pages docs: `https://radnotred.github.io/USBFileSync/`
- Local docs source: [`docs/`](docs/)
- Example config: [`config.example.json`](config.example.json)
- Contribution guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)

## Install

Release artifacts are produced automatically from the version in `Cargo.toml`:

- `usb_mirror_sync-portable-v<version>.zip`
- `usb_mirror_sync-setup-v<version>.exe`

The installer supports optional desktop shortcut creation and optional run-at-startup registration. Startup is off by default.

## Runtime Files

On first launch the app creates:

- `%LOCALAPPDATA%\UsbMirrorSync\config.json`
- `%LOCALAPPDATA%\UsbMirrorSync\manifest.json`
- `%LOCALAPPDATA%\UsbMirrorSync\shadow\`
- `%LOCALAPPDATA%\UsbMirrorSync\sync.log`

Use the tray menu `Setup Wizard` action for guided configuration, or edit `config.json` directly.

## Development

```powershell
cargo test
cargo build --release
```

For local docs work:

```powershell
python -m pip install -r requirements-docs.txt
mkdocs serve
```
