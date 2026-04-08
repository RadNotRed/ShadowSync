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

- `drive`: where the USB is expected to appear when you are using manual or legacy path setup
- `app`: sync behavior and watch settings
- `cache`: shared shadow-cache defaults
- `jobs`: which USB folders map to which local folders, and whether each job uses cache or direct mode

The current wizard writes absolute USB source paths into `jobs[].source` and derives the drive root from them. Relative `source` paths still work in hand-edited configs when `drive.letter` or `drive.path` is set.

## `drive`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `letter` | string or null | platform default in generated template | Windows-only drive letter such as `E` or `S`; mainly useful for manual configs |
| `path` | string or null | platform default in generated template on macOS/Linux | Absolute mounted USB path; mainly useful for manual configs |
| `eject_after_sync` | bool | `true` | Ejects the drive after a successful pull or push sync |

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
| `shadow_copy` | bool | `true` | Compatibility flag; current behavior is controlled per job by `jobs[].use_shadow_cache` |
| `clear_shadow_on_eject` | bool | template writes `false` | Deletes shadow-cache folders when the drive is removed or after an ejecting sync; set this explicitly in older configs |

## `compare`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `hash_on_metadata_change` | bool | `true` | Uses hashing when metadata indicates a file may have changed |

## `jobs`

Each job maps one USB source folder to one absolute PC target path.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `name` | string | none | Must be unique |
| `source` | string | none | Absolute USB path preferred; relative path still works for manual/legacy configs |
| `target` | string | none | Absolute local folder path |
| `mirror_deletes` | bool | `true` | Follows source-side deletes for the active sync direction |
| `use_shadow_cache` | bool | `true` | When `true`, the job stages through a per-job cache folder; when `false`, it syncs directly |
| `shadow_root` | string or null | none | Optional per-job cache root; overrides `cache.root` for that job |

## Rules

- All jobs must point at the same mounted USB drive or mount root.
- `source` should be an absolute USB folder path when created by the wizard.
- Relative `source` values must stay inside the configured USB drive.
- `target` must be an absolute local path.
- On Windows, `drive.letter` is only required if you rely on relative `source` paths.
- On macOS and Linux, `drive.path` is only required if you rely on relative `source` paths.
- Duplicate job names are rejected.
- Relative cache roots are resolved under the app data folder.

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
    "root": null,
    "shadow_copy": true,
    "clear_shadow_on_eject": false
  },
  "compare": {
    "hash_on_metadata_change": true
  },
  "jobs": [
    {
      "name": "ExampleType",
      "source": "S:\\MVA\\Example Folder On USB",
      "target": "C:\\Users\\user\\Documents\\ExampleFolder",
      "mirror_deletes": true,
      "use_shadow_cache": true,
      "shadow_root": null
    }
  ]
}
```
