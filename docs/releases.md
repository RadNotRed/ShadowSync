# Release artifacts

This page summarizes how release assets are produced and named.

## Workflow output

The release workflow in `.github/workflows/release.yml` builds, tests, and packages the following:

- Windows x64 portable (`shadowsync-windows-x64-portable-v<version>.zip`) and installer (`shadowsync-windows-x64-setup-v<version>.exe`)
- Windows ARM64 portable (`shadowsync-windows-arm64-portable-v<version>.zip`) and installer (`shadowsync-windows-arm64-setup-v<version>.exe`)
- macOS x64 `.tar.gz` and `.dmg` (`shadowsync-macos-x64-v<version>.*`)
- macOS ARM64 `.tar.gz` and `.dmg` (`shadowsync-macos-arm64-v<version>.*`)
- Linux x64 archive (`shadowsync-linux-x64-v<version>.tar.gz`)
- Linux ARM64 archive (`shadowsync-linux-arm64-v<version>.tar.gz`)

Every artifact lands in the draft `v<version>` release, along with the generated README and config example found inside the `.tar.gz`/`.zip`.

## Version control

`Cargo.toml` is the source of truth. The workflow reads `package.version`, derives `tag=v<version>`, and only publishes if that tag is new or pointing at the current commit. Tags already present on previous commits will skip packaging until the version or tag changes again.

## Publication steps

1. Update `package.version` in `Cargo.toml`.
2. Push to `main` or trigger the workflow manually (`workflow_dispatch`).
3. Review the generated draft release (notes are auto-created via `gh release generate-notes`).
4. Publish the release once you have smoke-tested the artifacts.

## Secrets

Artifacts only require the standard `GITHUB_TOKEN`. Add the macOS signing/notarization secrets if the workflow should automatically sign and notarize the `.dmg`:

- `MACOS_CERTIFICATE_P12_BASE64`
- `MACOS_CERTIFICATE_PASSWORD`
- `MACOS_SIGNING_IDENTITY`
- `MACOS_NOTARY_APPLE_ID`
- `MACOS_NOTARY_TEAM_ID`
- `MACOS_NOTARY_APP_PASSWORD`
