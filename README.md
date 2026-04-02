# USB Mirror Sync

Windows tray app for syncing one or more folders from a USB drive identified by drive letter onto your PC.

What this MVP does:

- Detects a configured drive letter like `E:`
- Auto-syncs when the drive appears
- Watches the USB for changes while mounted instead of re-running a timed full sync
- Can optionally watch the local target and push changes back to the USB
- Mirrors multiple USB folders into local PC folders
- Shows live sync progress in the tray menu and tooltip
- Skips unchanged files using a persistent local manifest cache
- Replaces changed files and optionally deletes removed files
- Keeps an optional local shadow cache that mirrors the USB source
- Ejects the drive after a successful sync
- Clears the local shadow cache after eject if configured
- Includes a Windows setup wizard so users can build/edit config without hand-writing JSON

What it assumes:

- Windows only
- The USB folders are the source of truth for each configured job
- The app writes the mirrored result into local PC folders

## Config

At first launch the app creates:

- `%LOCALAPPDATA%\UsbMirrorSync\config.json`
- `%LOCALAPPDATA%\UsbMirrorSync\manifest.json`
- `%LOCALAPPDATA%\UsbMirrorSync\shadow\`
- `%LOCALAPPDATA%\UsbMirrorSync\sync.log`

Edit `config.json` to point at your actual folders. `source` is the folder path inside the USB drive, and `target` is the absolute local PC path. A matching example is also included in this repo as [`config.example.json`](/C:/Users/rad/Documents/github/file-sync/config.example.json).

You can also use the tray menu `Setup Wizard` action to edit the config in a small Windows form instead of editing JSON directly.

Example:

```json
{
  "drive": {
    "letter": "E",
    "eject_after_sync": true
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
      "target": "C:\\Users\\rad\\Documents\\Important",
      "mirror_deletes": true
    }
  ]
}
```

## Cache model

There are two layers of cache:

- `manifest.json`: persistent metadata and hashes used to skip unchanged files on later runs
- `shadow\`: optional local shadow cache that mirrors the USB source and also acts as the staging source for the real target folder

`shadow` is not the live folder. Your configured `target` folder is the live mirrored destination on the PC. The normal pull flow is `USB -> shadow cache -> live target`.

Manual `Sync to USB now` uses the reverse path: `live target -> shadow cache -> USB`.

If you want `shadow` to remain as a persistent local cache/master copy, set `clear_shadow_on_eject` to `false`. If it is `true`, the cache is deleted after eject/remove and only the manifest remains.

## Build

```powershell
cargo build --release
```

The binary will be at:

- [`target\release\usb_mirror_sync.exe`](/C:/Users/rad/Documents/github/file-sync/target/release/usb_mirror_sync.exe)

## Notes

- If the USB drive was changed outside this app, the next run may do a full recopy of tracked files to re-establish a known state.
- `poll_interval_seconds` is now the drive-detection and config-reload interval. Mounted-folder changes are handled by filesystem watchers.
- `sync_while_mounted: true` means USB-side file changes trigger `USB -> shadow -> target` automatically while the drive stays mounted.
- `auto_sync_to_usb: true` means local target changes trigger `target -> shadow -> USB` automatically while the drive stays mounted.
- `mirror_deletes: true` only deletes files from `shadow` and the local target when they were removed from the USB source. Deleting a file from the local target does not delete it from the USB.
- The tray menu exposes `Sync from USB now`, `Sync to USB now`, `Eject drive`, `Setup Wizard`, `Open raw config`, `Open log`, `Open app folder`, and `Quit`.
