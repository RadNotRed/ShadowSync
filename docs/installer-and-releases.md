# Installer and Releases

## Installer Behavior

The Windows installer is built with Inno Setup and supports:

- standard app installation
- Start Menu entry
- optional desktop shortcut
- optional run-at-startup registration

Startup is disabled by default.

## Artifact Matrix

The release workflow publishes native artifacts for the main 64-bit desktop targets:

- Windows x64: portable zip and installer exe
- Windows ARM64: portable zip and installer exe
- macOS Intel: `.dmg` and `.tar.gz`
- macOS Apple silicon: `.dmg` and `.tar.gz`
- Linux x64: `.tar.gz`
- Linux ARM64: `.tar.gz`

The macOS `.dmg` contains a normal `.app` bundle plus an `Applications` shortcut for drag-and-drop installation. The `.tar.gz` is kept as a raw fallback artifact.

Linux archives include the binary, docs, example config, and basic desktop integration files.

## Automated Releases

Releases are generated from GitHub Actions using the package version in `Cargo.toml`.

Flow:

1. Bump `package.version`
2. Push to `main`
3. The release workflow builds, tests, packages, and drafts `v<version>`
4. The workflow uploads:
   - `usb_mirror_sync-windows-x64-portable-v<version>.zip`
   - `usb_mirror_sync-windows-x64-setup-v<version>.exe`
   - `usb_mirror_sync-windows-arm64-portable-v<version>.zip`
   - `usb_mirror_sync-windows-arm64-setup-v<version>.exe`
   - `usb_mirror_sync-macos-x64-v<version>.dmg`
   - `usb_mirror_sync-macos-x64-v<version>.tar.gz`
   - `usb_mirror_sync-macos-arm64-v<version>.dmg`
   - `usb_mirror_sync-macos-arm64-v<version>.tar.gz`
   - `usb_mirror_sync-linux-x64-v<version>.tar.gz`
   - `usb_mirror_sync-linux-arm64-v<version>.tar.gz`

If a version tag already exists on an older commit, the workflow skips republishing until the Cargo version changes.

## GitHub Pages Docs

The docs site is deployed through GitHub Pages using a separate workflow and a Material for MkDocs build.

## Secrets and Variables

No custom repository secrets or variables are required for unsigned release artifacts and GitHub Pages docs.

Optional macOS signing and notarization is enabled when these secrets are present:

- `MACOS_CERTIFICATE_P12_BASE64`
- `MACOS_CERTIFICATE_PASSWORD`
- `MACOS_SIGNING_IDENTITY`
- `MACOS_NOTARY_APPLE_ID`
- `MACOS_NOTARY_TEAM_ID`
- `MACOS_NOTARY_APP_PASSWORD`

The workflows rely on the built-in `GITHUB_TOKEN`.
