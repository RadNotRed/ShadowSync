# Release Artifacts

## What the Release Workflow Builds

The release workflow reads `package.version` from `Cargo.toml` and creates:

- `usb_mirror_sync-portable-v<version>.zip`
- `usb_mirror_sync-setup-v<version>.exe`

Both artifacts are attached to a GitHub draft release tagged as `v<version>`.

## Version Source of Truth

`Cargo.toml` is the version source of truth. If the same `v<version>` tag already exists on an older commit, the release workflow skips publishing until the version is bumped.

## Installer Options

The Windows installer currently supports:

- normal install location
- optional desktop shortcut
- optional startup shortcut, off by default

## Portable Build

The portable zip contains the executable and supporting files without writing an installation entry to Windows.

## Publishing Flow

1. Bump `version` in `Cargo.toml`
2. Push to `main` or run the release workflow manually
3. Review the generated draft release
4. Publish it when ready

## Required GitHub Secrets or Variables

None for the current setup. The workflows rely on GitHub's built-in `GITHUB_TOKEN`.
