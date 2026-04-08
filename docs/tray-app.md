# Tray App

## Tray Menu

The app lives in the system tray or menu bar area and exposes the main workflow through the tray menu.

Typical actions:

- `Sync from USB now`
- `Sync to USB now`
- `Eject drive`
- `Setup Wizard`
- `Check for updates`
- `Download latest release`
- `Skip this version`
- `Open mounted drive`
- `Open shadow cache`
- `Open raw config`
- `Open log`
- `Open app folder`
- `Quit`

The menu also shows separate status, progress, and update lines while the tooltip carries more detail.

## Progress Reporting

While a sync is running, the app reports:

- current direction
- current phase
- current job or file
- percent complete or operation counts
- written megabytes when available

This keeps the tray UI compact without losing visibility into what is happening.

## Setup Wizard

The setup wizard is a Rust desktop UI that helps edit `config.json` without hand-editing JSON.

It includes:

- absolute USB source-folder picking with drive-root inference
- job list editing
- per-job shadow-cache versus direct mode
- optional custom cache roots
- path browse actions
- create-missing-target-folder help
- wizard log access
- config recovery context when a broken config is repaired
- save and save-and-close flows

## Single-Instance Protection

If the app is launched twice, it prompts instead of silently double-running. The user can leave the existing copy alone or restart it.

## Update Checks

ShadowSync can check GitHub Releases for new versions. It caches update state locally in `update-state.json`, checks automatically about once every 24 hours, and lets you manually check, open the release page, or skip a specific version from the tray.

## Config Recovery

If the app fails to parse `config.json`, it can automatically launch the wizard, back up the broken config to `config.invalid.<timestamp>.json`, and replace it with a safe template so the UI stays usable.
