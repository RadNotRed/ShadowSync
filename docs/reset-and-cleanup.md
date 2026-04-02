# Reset and Cleanup

USB Mirror Sync stores local state so it can stay incremental between runs. Sometimes that is exactly what you want to remove: a broken config, stale manifest, or shadow cache you no longer trust.

## Reset Scripts

The repo provides platform-specific cleanup scripts under `tools/reset/`:

- `tools/reset/reset-windows.bat`
- `tools/reset/reset-macos.sh`
- `tools/reset/reset-linux.sh`

These scripts are intended to remove the app's per-user state, including:

- `config.json`
- `manifest.json`
- `sync.log`
- `shadow/`

They are for local cleanup. They are not meant to uninstall packaged binaries or remove release artifacts from system install locations.

## When to Use Them

Use a reset script when:

- the config was badly broken and you want to start fresh
- you want to discard the cached manifest and shadow copy
- you are testing a first-run experience
- you want to verify behavior without any retained local state

## Safer First Steps

Before wiping everything:

- use the tray action `Open raw config`
- use the tray action `Open log`
- use the tray action `Open app folder`
- back up `config.json` if you care about your current job setup

## After a Reset

After local state is removed:

1. Launch the app again.
2. Open `Setup Wizard`.
3. Recreate the drive setting:
   - Windows: `drive.letter`
   - macOS/Linux: `drive.path`
4. Recreate your jobs and save.

The app will then build fresh local state on the next sync.
