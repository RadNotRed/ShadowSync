# How Sync Works

## Pull Path

For cached jobs, the normal ingest path is:

```text
USB source -> shadow cache -> local target
```

For direct jobs, the ingest path is:

```text
USB source -> local target
```

This runs when the drive is inserted or when you trigger `Sync from USB now`.

## Push Path

For cached jobs, the reverse path is:

```text
local target -> shadow cache -> USB source
```

For direct jobs, the reverse path is:

```text
local target -> USB source
```

This runs when:

- you choose `Sync to USB now`
- `auto_sync_to_usb` is enabled and the drive is mounted

## Why the Shadow Cache Exists

The shadow cache is the staging layer between the USB and the live local target for jobs with `use_shadow_cache = true`.

It helps the app:

- retain a local copy of known USB state
- avoid unnecessary recopy work
- reduce direct dependence on live destination state
- stage pull and push operations through a predictable path

The shadow cache is not the live PC folder. Your configured `target` remains the live destination on the machine. Each cached job gets its own folder under the shared `cache.root` or its job-specific `shadow_root`.

## Manifest Cache

`manifest.json` stores metadata about previously synced files so unchanged files can be skipped on later runs.

When `hash_on_metadata_change` is enabled, ShadowSync hashes files whose size or modified time changed before deciding whether content really needs to be recopied.

## USB Drive Marker

ShadowSync also writes a small marker file on the USB drive:

```text
.usb-mirror-sync/state.json
```

That marker stores the last sync token for the drive. If the local manifest and USB marker no longer match, ShadowSync treats the next run as a full baseline rebuild so incremental decisions stay safe.

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

## Eject Behavior

If `eject_after_sync` is enabled, a successful sync can eject the USB drive automatically. If shadow caching is enabled and `clear_shadow_on_eject` is also enabled, ShadowSync removes cached job folders after that eject or after it notices the drive was removed.
