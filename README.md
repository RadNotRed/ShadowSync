# ShadowSync

ShadowSync is a cross-platform tray app that mirrors selected USB folders to your computer with a staged shadow cache, fast incremental syncs, optional push-back, and safe eject workflows.

## 📌 What it is

ShadowSync is built for a simple removable-drive workflow:

- Plug in a USB drive
- Pull changed files onto your computer
- Work from the local folder
- Optionally push changes back to the USB
- Eject safely from the tray when you're done

## ⚙️ What it does

- `USB -> shadow cache -> local folder` pull sync by default
- Optional `local folder -> shadow cache -> USB` push-back flow
- Manifest-based incremental syncing to skip unchanged files
- Tray icon controls for sync, logs, folders, setup, and eject
- Native Rust setup wizard for easier configuration
- Single-instance protection and config recovery handling
- Windows, macOS, and Linux packaging support

## 🚀 Quick start

1. Download the correct release for your platform from the GitHub Releases page.
2. Launch ShadowSync and let it create its local app data files.
3. Open the setup wizard from the tray and configure your USB source and local target.
4. Run `Sync from USB now` to pull files onto the machine.
5. Use `Sync to USB now` only when you want to publish changes back to the drive.

## 📚 Documentation

The README stays intentionally short. Use the docs for setup, behavior, and maintenance details:

- 🌐 [Docs site](https://radnotred.github.io/USBFileSync/)
- 🛠️ [Getting Started](docs/getting-started.md)
- 🧩 [Configuration](docs/configuration.md)
- 🖱️ [Tray App](docs/tray-app.md)
- 🔄 [Sync Model](docs/sync-model.md)
- 🧹 [Reset and Cleanup](docs/reset-and-cleanup.md)
- 📦 [Installer and Releases](docs/installer-and-releases.md)

## 📁 Project links

- 📄 [License](LICENSE)
- 🤝 [Contributing Guidelines](CONTRIBUTING.md)
- 🧪 [Example Config](config.example.json)
- 🧰 [Reset Tools](tools/reset/)

## 💻 Development

```powershell
cargo test --locked
cargo build --release
```

For docs previews:

```powershell
python -m pip install -r requirements-docs.txt
python -m mkdocs serve
```

## 🤖 Note

ShadowSync was developed with AI-assisted tooling, with the repository and releases reviewed and driven through iterative manual refinement.
