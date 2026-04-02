# Installer and releases

This page is for build and release details. Most people only need to grab one of the prebuilt artifacts for their OS.

## Windows installer

Windows gets both a portable ZIP and an Inno Setup installer per architecture (x64 and ARM64). Each installer:

- registers ShadowSync in the Start Menu
- offers optional desktop shortcut and run-at-startup tasks
- copies the current executable, README, and `config.example.json`

Startup registration is unchecked by default.

## Artifacts

GitHub Actions publishes every artifact into a draft `v<version>` release:

- `shadowsync-windows-x64-portable-v<version>.zip`
- `shadowsync-windows-x64-setup-v<version>.exe`
- `shadowsync-windows-arm64-portable-v<version>.zip`
- `shadowsync-windows-arm64-setup-v<version>.exe`
- `shadowsync-macos-x64-v<version>.tar.gz`
- `shadowsync-macos-x64-v<version>.dmg`
- `shadowsync-macos-arm64-v<version>.tar.gz`
- `shadowsync-macos-arm64-v<version>.dmg`
- `shadowsync-linux-x64-v<version>.tar.gz`
- `shadowsync-linux-arm64-v<version>.tar.gz`

The macOS `.dmg` bundles `ShadowSync.app` plus an `Applications` shortcut for drag-and-drop installs; the `.tar.gz` remains as a lightweight alternative. Linux archives include the binary, docs, example config, and desktop integration files for GTK/KDE.

## Release automation

Releases are driven by `.github/workflows/release.yml`. The workflow:

1. Reads `package.version` from `Cargo.toml` and determines whether a `v<version>` tag already exists.
2. Builds and tests for Windows, macOS, and Linux (including multiple architectures).
3. Packages each target with the helper scripts in `.github/scripts/`.
4. Drafts or updates the release, writes generated release notes, and uploads every artifact.

If a `v<version>` tag already points to a previous commit, the workflow skips until the version or tag moves.

## Optional secrets

Unsigned builds only rely on the default `GITHUB_TOKEN`. Add the macOS signing/notary secrets when you want the installer signed and notarized:

- `MACOS_CERTIFICATE_P12_BASE64`
- `MACOS_CERTIFICATE_PASSWORD`
- `MACOS_SIGNING_IDENTITY`
- `MACOS_NOTARY_APPLE_ID`
- `MACOS_NOTARY_TEAM_ID`
- `MACOS_NOTARY_APP_PASSWORD`
