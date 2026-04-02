# How Sync Works

## Pull Path

The normal ingest path is:

```text
USB source -> shadow cache -> local target
```

This is what runs when the drive is inserted or when you trigger `Sync from USB now`.

## Push Path

The reverse path is:

```text
local target -> shadow cache -> USB source
```

This runs when:

- you choose `Sync to USB now`
- `auto_sync_to_usb` is enabled and the drive is mounted

## Why the Shadow Cache Exists

The shadow cache is the staging layer between the USB and the live local target.

It helps the app:

- retain a local copy of known USB state
- avoid unnecessary recopy work
- reduce direct dependence on live destination state
- stage pull and push operations through a predictable path

The shadow cache is not the live PC folder. Your configured `target` remains the live destination on the machine.

## Manifest Cache

`manifest.json` stores metadata about previously synced files so unchanged files can be skipped on later runs.

If the app loses confidence in its known state, it may perform extra copy work to re-establish a reliable baseline.

## Delete Rules

`mirror_deletes` follows the active source side:

- During pull sync, USB-side deletions can remove files from `shadow` and the local target
- During push sync, local-target deletions can remove files from `shadow` and the USB

Deleting a file from the local target does not remove it from the USB unless you run or enable push sync.

## Watchers Versus Polling

Mounted folder changes are handled by filesystem watchers.

`poll_interval_seconds` is still used, but only for:

- drive insert/remove detection
- config reload checks

It is not the interval for repeated full mirror runs.

## Conflict Model

This is a mirror tool, not a conflict-resolution engine.

If the same file is changed on both sides, the later sync direction wins.
