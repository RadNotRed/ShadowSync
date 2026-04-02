# Troubleshooting

## The Drive Is Mounted but Nothing Syncs

Check:

- the configured drive letter matches the actual removable drive
- `sync_on_insert` is enabled if you expect a sync on plug-in
- `sync_while_mounted` is enabled if you expect USB-side live mirroring
- `auto_sync_to_usb` is enabled if you expect local changes to publish automatically

## A Local Delete Did Not Remove the File from the USB

That is expected unless push sync runs. Local deletions only flow to the USB during the push direction.

## Config Parse Failed

The app can auto-open the Setup Wizard when `config.json` is invalid. Recovery flow can back up the broken file and restore a safe default config so the app can start again.

## The Shadow Folder Is Confusing

`shadow` is staging cache, not the live PC destination. The configured `target` folder is the actual live mirrored folder on the machine.

## The App Launches Twice

Only one instance is allowed. Starting a second copy shows an `Already running` warning and lets you cancel or retry startup.

## Where to Look for Logs

Use the tray action `Open log` or `Open app folder` first. If you want a full clean slate, use the reset scripts in `tools/reset/`.

The reset tools are:

- `tools/reset/reset-windows.bat`
- `tools/reset/reset-macos.sh`
- `tools/reset/reset-linux.sh`
