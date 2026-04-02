# Release Artifacts

This page is for development and release maintenance.

## What the Workflow Builds

The release workflow reads `package.version` from `Cargo.toml` and creates:

- Windows x64 portable zip and installer
- Windows ARM64 portable zip and installer
- macOS x64 `.dmg` and `.tar.gz`
- macOS ARM64 `.dmg` and `.tar.gz`
- Linux x64 `.tar.gz`
- Linux ARM64 `.tar.gz`

Both artifacts are attached to a GitHub draft release tagged as `v<version>`.

## Version Source of Truth

`Cargo.toml` is the version source of truth. If the same `v<version>` tag already exists on an older commit, the release workflow skips publishing until the version is bumped.

## Publishing Flow

1. Bump `version` in `Cargo.toml`
2. Push to `main` or run the release workflow manually
3. Review the generated draft release
4. Publish it when ready

## Required GitHub Secrets or Variables

None for the current setup. The workflows rely on GitHub's built-in `GITHUB_TOKEN`.
