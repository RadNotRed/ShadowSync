# Installer and Releases

## Installer Behavior

The Windows installer is built with Inno Setup and supports:

- standard app installation
- Start Menu entry
- optional desktop shortcut
- optional run-at-startup registration

Startup is disabled by default.

## Portable Behavior

The portable zip contains the app binary, example config, README, and setup wizard script. It does not register startup or install shortcuts.

## Automated Releases

Releases are generated from GitHub Actions using the package version in `Cargo.toml`.

Flow:

1. Bump `package.version`
2. Push to `main`
3. The release workflow builds, tests, packages, and drafts `v<version>`
4. The workflow uploads:
   - `usb_mirror_sync-portable-v<version>.zip`
   - `usb_mirror_sync-setup-v<version>.exe`

If a version tag already exists on an older commit, the workflow skips republishing until the Cargo version changes.

## GitHub Pages Docs

The docs site is deployed through GitHub Pages using a separate workflow and a Material for MkDocs build.

## Secrets and Variables

No custom repository secrets or variables are required for the current release and docs flow.

The workflows rely on the built-in `GITHUB_TOKEN`.
