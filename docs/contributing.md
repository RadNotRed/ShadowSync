# Contributing

ShadowSync is a cross-platform utility with file movement, caching, packaging, and tray UX all tied together. Keep changes scoped and document behavior changes in the same patch.

## Quick Checklist

- Keep platform-specific behavior explicit
- Preserve the documented sync model unless you are intentionally changing it
- Add or update tests when sync logic changes
- Update the docs when config fields, workflows, packaging, or tray behavior change

## Useful Commands

```powershell
cargo test
cargo build --release
python -m pip install -r requirements-docs.txt
python -m mkdocs build --strict
```

## Areas That Deserve Extra Care

- `src/sync_engine.rs`: sync direction, delete semantics, manifest handling
- `src/app.rs`: tray behavior, status, startup flow, config recovery
- `src/wizard.rs`: cross-platform setup experience
- `.github/workflows/`: release and documentation automation
- `tools/reset/`: local cleanup and reset behavior
