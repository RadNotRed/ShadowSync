# Contributing

## Scope

USB Mirror Sync is a Windows-first Rust tray application. Changes should preserve the core model:

- USB pull path: `USB -> shadow cache -> local target`
- Optional push path: `local target -> shadow cache -> USB`
- Manifest cache for incremental sync
- Single-instance tray workflow

## Local Setup

Recommended environment:

- Windows
- Rust stable toolchain
- PowerShell
- Python 3.x for docs work

Core commands:

```powershell
cargo test
cargo build --release
```

Docs commands:

```powershell
python -m pip install -r requirements-docs.txt
python -m mkdocs build --strict
python -m mkdocs serve
```

## Project Areas

- `src/`: app, sync engine, config handling, watcher logic, wizard integration
- `assets/`: setup wizard PowerShell UI
- `.github/assets/`: SVG branding source used by the Windows build pipeline
- `.github/installer/`: Inno Setup installer script
- `.github/workflows/`: release and docs automation
- `docs/`: GitHub Pages documentation source

## Change Expectations

- Keep Windows-specific behavior explicit.
- Do not silently change sync direction semantics.
- Preserve the shadow-cache model unless the change intentionally redesigns it.
- Prefer incremental, testable changes over large rewrites.
- Update docs when user-facing behavior changes.

## Testing

Before pushing:

- Run `cargo test`
- Run `cargo build --release` for packaging-related changes
- Run `python -m mkdocs build --strict` for docs changes

If a change affects installer or release packaging, verify the relevant workflow or packaging script paths as well.

## Release Flow

Release versioning is driven by `Cargo.toml`.

1. Bump `package.version` in `Cargo.toml`
2. Push to `main`
3. GitHub Actions drafts a release tagged `v<version>`
4. The workflow attaches the portable zip and installer exe

No custom repo secrets are currently required. The workflows use the built-in `GITHUB_TOKEN`.
