# USB Mirror Sync

USB Mirror Sync is a Windows tray app built for one job: keep selected folders from a removable USB drive mirrored onto a PC without forcing a full copy every time the drive is plugged in.

<div class="grid cards" markdown>

-   ### Pull-First by Default

    The normal path is `USB -> shadow cache -> local target`, which makes the USB the source of truth for ingest.

-   ### Optional Publish Back to USB

    If you need it, the app can also push `local target -> shadow cache -> USB`, either manually or automatically.

-   ### Incremental by Design

    A persistent manifest and optional shadow cache let the app skip unchanged files and reuse known state between runs.

-   ### Built for the Tray

    Sync, open logs, edit config, launch the setup wizard, and eject the drive directly from the taskbar icon.

</div>

## Core Ideas

- The app is Windows-only.
- Jobs are tied to a configured USB drive letter.
- `source` paths are relative to the USB root.
- `target` paths are absolute folders on the PC.
- `shadow` is a staging cache, not the live destination folder.

## Start Here

- New user: [Getting Started](getting-started.md)
- Need to hand-edit JSON: [Configuration](configuration.md)
- Want to understand cache behavior and delete rules: [How Sync Works](sync-model.md)
- Working on the repo: [Contributing](contributing.md)
