# Getting Started

## Pick the Right Download

ShadowSync ships in platform-specific forms:

- Windows: portable zip or installer
- macOS: `.dmg` plus a `.tar.gz` fallback
- Linux: per-architecture `.tar.gz`

Startup is disabled by default in the Windows installer.

## First Setup

1. Launch the app.
2. Open `Setup Wizard` from the tray/menu bar icon.
3. Add at least one job by choosing:
   - the actual USB source folder on the mounted drive
   - the absolute local target folder on your computer
4. Pick whether that job should use the shadow cache or sync directly.
5. Save the config and leave the app running in the tray.

The current wizard saves absolute USB source paths and infers the drive root from those selections. If you edit `config.json` manually, legacy relative `source` paths are still supported when `drive.letter` or `drive.path` is set.

The app then creates a small per-user data folder with:

- `config.json`
- `manifest.json`
- `update-state.json`
- `wizard.log`
- `shadow/`
- `sync.log`

You may also see `config.invalid.<timestamp>.json` backup files after config recovery.

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
    "root": null,
    "shadow_copy": true,
    "clear_shadow_on_eject": false
  },
  "compare": {
    "hash_on_metadata_change": true
  },
  "jobs": [
    {
      "name": "Documents",
      "source": "E:\\Backups\\Documents",
      "target": "C:\\Users\\YOUR_NAME\\Documents\\Important",
      "mirror_deletes": true,
      "use_shadow_cache": true,
      "shadow_root": null
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
- `Check for updates`
- `Download latest release`
- `Skip this version`
- `Open mounted drive`
- `Open shadow cache`
- `Open raw config`
- `Open log`
- `Open app folder`

If `eject_after_sync` is enabled, a successful sync from either direction can eject the drive automatically. If a second copy of the app is launched, the single-instance guard shows a warning and lets you cancel or retry startup.

## Need a Clean Reset?

If you want to wipe local app state and start fresh, use the scripts in `tools/reset/`:

- `tools/reset/reset-windows.bat`
- `tools/reset/reset-macos.sh`
- `tools/reset/reset-linux.sh`

These remove local app state like config, manifest, logs, and shadow cache. They do not delete the folders you synced or files on the USB drive.
