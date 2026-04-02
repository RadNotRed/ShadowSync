# Configuration

The runtime config lives at `%LOCALAPPDATA%\UsbMirrorSync\config.json`.

## Top-Level Structure

```json
{
  "drive": {},
  "app": {},
  "cache": {},
  "compare": {},
  "jobs": []
}
```

## `drive`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `letter` | string | none | Single drive letter for the removable device, such as `E` or `S` |
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

## Path Rules

- `source` must stay inside the configured USB drive.
- `target` must be an absolute local path.
- `source` should not include the drive letter.
- Duplicate job names are rejected.

## Good Example

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
