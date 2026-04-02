# Getting Started

## Choose a Release

USB Mirror Sync ships in two Windows-friendly forms:

- Portable zip: unzip and run `usb_mirror_sync.exe`
- Installer: installs the app, Start Menu entry, optional desktop shortcut, and optional startup registration

Startup is disabled by default in the installer.

## First Launch

On first launch the app creates:

- `%LOCALAPPDATA%\UsbMirrorSync\config.json`
- `%LOCALAPPDATA%\UsbMirrorSync\manifest.json`
- `%LOCALAPPDATA%\UsbMirrorSync\shadow\`
- `%LOCALAPPDATA%\UsbMirrorSync\sync.log`

If the config is missing or invalid, the app can open the Setup Wizard automatically and recover to a safe default config.

## Configure a First Job

You can use either:

- `Setup Wizard` from the tray menu
- Direct edits to `config.json`

Each job maps one USB-side folder to one local PC folder.

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

## Daily Use

Tray actions include:

- `Sync from USB now`
- `Sync to USB now`
- `Eject drive`
- `Setup Wizard`
- `Open raw config`
- `Open log`
- `Open app folder`

If a second copy of the app is launched, the single-instance guard shows a warning and lets you cancel or retry startup.
