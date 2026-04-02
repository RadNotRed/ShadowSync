# ShadowSync

ShadowSync keeps a USB drive mirrored to your computer without a full copy every time. It tracks a shadow cache, watches for file changes, and exposes tray controls so you can sync, open folders, or eject safely with one menu.

## Why it matters

- **Pull-first by default:** New files flow `USB → shadow cache → local folder`, which keeps the USB as the authoritative source and leaves your working directory untouched.
- **Optional push-back:** When you edit locally, ShadowSync stages changes in the same shadow cache so you can run a one-shot push or leave it on auto-sync to publish back to USB.
- **Fast increments:** A manifest cache skips unchanged files, and a staged shadow copy avoids locking the live folder during transfers.
- **Native tray & wizard:** Right-click to sync, open logs, or launch the setup wizard. The app warns if another instance is running, and the wizard auto-opens when the config is missing or malformed.
- **Cross-platform releases:** Binaries, installers, archives, and reset tools are produced for Windows, macOS, and Linux.

## What to do first

1. Install or extract ShadowSync on your platform (see the docs below for installer vs. portable formats).
2. Launch ShadowSync so it creates `config.json`, `manifest.json`, and the `shadow` cache in your per-user data directory.
3. Point the config at your USB drive (`drive.letter` on Windows, `drive.path` on macOS/Linux) and describe the job(s) that should mirror folders.
4. Use the tray menu to run `Sync from USB now`, check progress, or trigger `Sync to USB now` when you want to publish changes.
5. Eject the drive from the tray to ensure the latest copy and clear the shadow cache (if enabled).

## Resources

- Docs site: `https://radnotred.github.io/USBFileSync/`
- Local docs source: [`docs/`](docs/)
- Config example: [`config.example.json`](config.example.json)
- Reset tools: [`tools/reset/`](tools/reset/)
- Developer notes: [`CONTRIBUTING.md`](CONTRIBUTING.md)

## Release artifacts

Artifacts are built from the `Cargo.toml` version and include native installers as well as portable archives:

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

## Docs & help

- Start setup: [`docs/getting-started.md`](docs/getting-started.md)
- Customize jobs: [`docs/configuration.md`](docs/configuration.md)
- Tray menu facts: [`docs/tray-app.md`](docs/tray-app.md)
- Sync model deep dive: [`docs/sync-model.md`](docs/sync-model.md)
- Reset helpers: [`docs/reset-and-cleanup.md`](docs/reset-and-cleanup.md)
- Release process: [`docs/installer-and-releases.md`](docs/installer-and-releases.md)

## Development

Run the test suite and a release build locally:

```powershell
cargo test --locked
cargo build --release
```

For docs previews:

```powershell
python -m pip install -r requirements-docs.txt
python -m mkdocs serve
```

