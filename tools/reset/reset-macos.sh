#!/usr/bin/env sh
set -eu

echo "USB Mirror Sync reset for macOS"
echo
echo "This removes local app state for the current user:"
echo "  - config, manifest, log, shadow cache"
echo "  - LaunchAgent entries for auto-start if present"
echo "It does NOT remove synced folders or files on your USB drive."
echo
printf "Type RESET to continue: "
read -r confirm
if [ "$confirm" != "RESET" ]; then
  echo "Cancelled."
  exit 0
fi

remove_path() {
  if [ -e "$1" ]; then
    echo "Removing $1"
    rm -rf "$1"
  fi
}

remove_path "$HOME/Library/Application Support/com.rad.UsbMirrorSync"
remove_path "$HOME/Library/Caches/com.rad.UsbMirrorSync"
remove_path "$HOME/Library/Logs/com.rad.UsbMirrorSync"
remove_path "$HOME/Library/Preferences/com.rad.UsbMirrorSync"
remove_path "$HOME/Library/LaunchAgents/com.rad.usbmirrorsync.plist"
remove_path "$HOME/Library/LaunchAgents/USB Mirror Sync.plist"

echo
echo "Reset complete."
