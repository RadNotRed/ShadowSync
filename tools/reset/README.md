# Reset Tools

These scripts remove ShadowSync's local app state from the current user account.

They are intended for:

- clearing broken config or cache state
- removing tray auto-start shortcuts
- resetting the app to a first-run state

They do not remove:

- your synced target folders
- files on the USB drive
- the installed application binaries themselves

Scripts:

- `reset-windows.bat`
- `reset-macos.sh`
- `reset-linux.sh`

Each script removes local app data such as:

- `config.json`
- `manifest.json`
- `sync.log`
- `shadow/`
- single-instance lock files
- startup shortcuts or launch entries created for the current user

Run them only when ShadowSync is closed.
