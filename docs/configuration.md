# Configuration

Most users can use the Setup Wizard and never touch this file.

If you do want to edit it directly, the runtime config is stored in the app's local data folder as `config.json`.

## Basic Shape

```json
{
  "drive": {},
  "app": {},
  "cache": {},
  "compare": {},
  "jobs": []
}
```

## The Important Parts

- `drive`: where the USB is expected to appear
- `app`: sync behavior and watch settings
- `cache`: shadow cache behavior
- `jobs`: which USB folders map to which local folders

## `drive`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `letter` | string or null | none | Windows-only drive letter such as `E` or `S` |
| `path` | string or null | none | Absolute mounted USB path, used on macOS and Linux |
| `eject_after_sync` | bool | `true` | Ejects the drive after a successful sync run |

## `app`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `sync_on_insert` | bool | `true` | Runs a USB-to-PC sync when the configured drive appears |
| `sync_while_mounted` | bool | `true` | Watches the USB for live changes while mounted |
| `auto_sync_to_usb` | bool | `false` | Watches local targets and pushes changes back to the USB |
| `poll_interval_seconds` | integer | `2` | Used for drive detection and config reload checks, clamped to `1..60` |

## `cache`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `root` | string or null | app-managed path | Optional custom location for the shadow cache root |
| `shadow_copy` | bool | `true` | Keeps a local shadow copy for staging and reuse |
| `clear_shadow_on_eject` | bool | `false` | Deletes the shadow cache when the drive is ejected or removed |

## `compare`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `hash_on_metadata_change` | bool | `true` | Uses hashing when metadata indicates a file may have changed |

## `jobs`

Each job maps one USB-relative source path to one absolute PC target path.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `name` | string | none | Must be unique |
| `source` | string | none | Relative path inside the USB root |
| `target` | string | none | Absolute local folder path |
| `mirror_deletes` | bool | `true` | Follows source-side deletes for the active sync direction |

## Rules

- `source` must stay inside the configured USB drive.
- `target` must be an absolute local path.
- On Windows, set `drive.letter`.
- On macOS and Linux, set `drive.path`.
- Duplicate job names are rejected.

## Example

```json
{
  "drive": {
    "letter": "S",
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
      "name": "ExampleType",
      "source": "MVA\\Example Folder On USB",
      "target": "C:\\Users\\user\\Documents\\ExampleFolder",
      "mirror_deletes": true
    }
  ]
}
```
