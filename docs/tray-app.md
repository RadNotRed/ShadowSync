# Tray App

## Tray Menu

The app lives in the system tray or menu bar area and exposes the main workflow through the tray menu.

Typical actions:

- `Sync from USB now`
- `Sync to USB now`
- `Eject drive`
- `Setup Wizard`
- `Open mounted drive`
- `Open shadow cache`
- `Open raw config`
- `Open log`
- `Open app folder`
- `Quit`

The menu also shows a short status line while the tooltip carries more detail.

## Progress Reporting

While a sync is running, the app reports:

- current direction
- current phase
- current job or file
- copy/delete counts when available

This keeps the tray UI compact without losing visibility into what is happening.

## Setup Wizard

The setup wizard is a Rust desktop UI that helps edit `config.json` without hand-editing JSON.

It includes:

- cross-platform drive and mount-path setup
- job list editing
- path browse actions
- config recovery context when a broken config is repaired
- save and save-and-close flows

## Single-Instance Protection

If the app is launched twice, it prompts instead of silently double-running. The user can leave the existing copy alone or restart it.

## Config Recovery

If the app fails to parse `config.json`, it can automatically launch the wizard, back up the broken config, and replace it with a safe template so the UI stays usable.
