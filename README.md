# ShadowSync

ShadowSync is a cross-platform tray app that mirrors selected USB folders to your computer using a shadow cache for fast incremental syncs.

## 📌 What it is

ShadowSync is meant for a straightforward removable-drive workflow:

- Plug in a USB drive
- Pull the latest changes onto your computer
- Work from the local folder instead of the drive
- Push back to the USB only when you want to
- Eject safely from the tray when you're done

## ⚙️ What it does

- Pull sync with `USB -> shadow cache -> local folder`
- Optional push-back with `local folder -> shadow cache -> USB`
- Incremental syncing that skips unchanged files
- Tray controls for sync, logs, folders, setup, and eject
- Native Rust setup wizard
- Single-instance protection and config recovery
- Windows, macOS, and Linux release builds

## 🚀 Quick start

1. Download the right release for your platform from the GitHub Releases page.
2. Launch ShadowSync and let it create its local app data files.
3. Open the setup wizard from the tray and configure your USB source and local target.
4. Run `Sync from USB now` to pull files onto the machine.
5. Use `Sync to USB now` only when you want to publish changes back to the drive.

## 📚 Documentation

This README stays short on purpose. Use the docs for setup details, behavior, and troubleshooting:

- 🌐 [Docs site](https://radnotred.github.io/ShadowSync/)
- 🛠️ [Getting Started](https://radnotred.github.io/ShadowSync/getting-started/)
- 🧩 [Configuration](https://radnotred.github.io/ShadowSync/configuration/)
- 🖱️ [Tray App](https://radnotred.github.io/ShadowSync/tray-app/)
- 🔄 [Sync Model](https://radnotred.github.io/ShadowSync/sync-model/)
- 🧹 [Reset and Cleanup](https://radnotred.github.io/ShadowSync/reset-and-cleanup/)
- 📦 [Installer and Releases](https://radnotred.github.io/ShadowSync/installer-and-releases/)

## 📁 Project links

- 📄 [License](LICENSE)
- 🤝 [Contributing Guidelines](CONTRIBUTING.md)
- 🧪 [Example Config](config.example.json)
- 🧰 [Reset Tools](tools/reset/)

## 💻 Development

```powershell
cargo test --locked
cargo wizard
cargo build --release
```

To open only the setup UI without starting the tray workflow:

```powershell
cargo wizard
```

For a release-mode UI run:

```powershell
cargo wizard-release
```

For the closest thing to a native Rust "dev server" for the wizard, use `cargo-watch` so the UI rebuilds and relaunches on every save:

```powershell
cargo install cargo-watch
cargo watch-wizard
```

To keep tests running while you edit:

```powershell
cargo watch-tests
```

These aliases are development-only helpers from [.cargo/config.toml](.cargo/config.toml). They do not add runtime or release bloat.

For docs previews:

```powershell
python -m pip install -r requirements-docs.txt
python -m mkdocs serve
```

## 🤖 Note

ShadowSync was built with AI-assisted tooling and then refined through manual review and iteration.
