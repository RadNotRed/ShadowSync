# Getting Started

## Pick the Right Download

USB Mirror Sync ships in platform-specific forms:

- Windows: portable zip or installer
- macOS: `.dmg` plus a `.tar.gz` fallback
- Linux: per-architecture `.tar.gz`

Startup is disabled by default in the Windows installer.

## First Setup

1. Launch the app.
2. Open `Setup Wizard` from the tray/menu bar icon.
3. Point the app at the USB source:
   - Windows: set `drive.letter`
   - macOS/Linux: set `drive.path`
4. Add at least one job mapping a USB-relative `source` to an absolute local `target`.
5. Save the config and leave the app running in the tray.

The app then creates a small per-user data folder with:

- `config.json`
- `manifest.json`
- `shadow/`
- `sync.log`

If the config is missing or invalid, the app can open the Setup Wizard automatically and recover to a safe default config.

## Example Job

A basic job says:

- which folder on the USB to watch
- which folder on your computer should receive the mirrored copy

Example:

```json
{
  "drive": {
    "letter": "E",
    "eject_after_sync": false
  },
  "app": {
    "sync_on_insert": true,
    "sync_while_mounted": true,
    "auto_sync_to_usb": false,
    "poll_interval_seconds": 2
  },
  "cache": {
    "shadow_copy": true,
    "clear_shadow_on_eject": false
  },
  "compare": {
    "hash_on_metadata_change": true
  },
  "jobs": [
    {
      "name": "Documents",
      "source": "Backups\\Documents",
      "target": "C:\\Users\\YOUR_NAME\\Documents\\Important",
      "mirror_deletes": true
    }
  ]
}
```

## Everyday Use

Tray actions include:

- `Sync from USB now`
- `Sync to USB now`
- `Eject drive`
- `Setup Wizard`
- `Open mounted drive`
- `Open shadow cache`
- `Open raw config`
- `Open log`
- `Open app folder`

If a second copy of the app is launched, the single-instance guard shows a warning and lets you cancel or retry startup.

## Need a Clean Reset?

If you want to wipe local app state and start fresh, use the scripts in `tools/reset/`:

- `tools/reset/reset-windows.bat`
- `tools/reset/reset-macos.sh`
- `tools/reset/reset-linux.sh`

These remove local app state like config, manifest, logs, and shadow cache. They do not delete the folders you synced or files on the USB drive.
