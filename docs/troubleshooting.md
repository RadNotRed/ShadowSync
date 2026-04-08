# Troubleshooting

## The Drive Is Mounted but Nothing Syncs

Check:

- the job `source` paths point at the actual mounted USB folders
- all jobs are on the same drive or mount root
- the configured drive letter/path matches the actual removable drive if you are using a manual or legacy relative-path config
- `sync_on_insert` is enabled if you expect a sync on plug-in
- `sync_while_mounted` is enabled if you expect USB-side live mirroring
- `auto_sync_to_usb` is enabled if you expect local changes to publish automatically

## The Next Sync Re-Copied Everything

That usually means ShadowSync no longer trusts its previous baseline. Common reasons:

- `manifest.json` was deleted or reset
- the USB-side marker `.usb-mirror-sync/state.json` is missing or belongs to a different sync history
- the shadow cache for a cached job was removed

In that case the app performs a full baseline rebuild and then returns to incremental behavior on later runs.

## A Local Delete Did Not Remove the File from the USB

That is expected unless push sync runs. Local deletions only flow to the USB during the push direction.

## Config Parse Failed

The app can auto-open the Setup Wizard when `config.json` is invalid. Recovery flow can back up the broken file and restore a safe default config so the app can start again.

## The Shadow Folder Is Confusing

`shadow` is staging cache, not the live PC destination. The configured `target` folder is the actual live mirrored folder on the machine. Jobs with `use_shadow_cache = false` skip the shadow folder entirely.

## The Cache Disappears After Eject

Check `cache.clear_shadow_on_eject`. The current generated config writes this as `false`, but older or hand-edited configs should set it explicitly so you do not depend on missing-field behavior.

## The App Launches Twice

Only one instance is allowed. Starting a second copy shows an `Already running` warning and lets you cancel or retry startup.

## Where to Look for Logs

Use the tray action `Open log` or `Open app folder` first. If you want a full clean slate, use the reset scripts in `tools/reset/`.

The reset tools are:

- `tools/reset/reset-windows.bat`
- `tools/reset/reset-macos.sh`
- `tools/reset/reset-linux.sh`
