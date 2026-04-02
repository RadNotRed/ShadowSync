#!/usr/bin/env sh
set -eu

echo "USB Mirror Sync reset for Linux"
echo
echo "This removes local app state for the current user:"
echo "  - config, manifest, log, shadow cache"
echo "  - autostart desktop entries if present"
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

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"

remove_path "$data_home/rad/UsbMirrorSync"
remove_path "$config_home/UsbMirrorSync"
remove_path "$HOME/.config/autostart/usb-mirror-sync.desktop"
remove_path "$HOME/.config/autostart/USB Mirror Sync.desktop"

echo
echo "Reset complete."
