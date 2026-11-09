#!/usr/bin/env bash
#
# Build and install Omarchy Dock (CHDevSec fork of omarchy-dock) from this
# checkout:
#
#   ./install.sh
#
# The source lives outside ~/.config/omarchy/plugins on purpose: the Omarchy
# shell watches every file in a plugin directory and reloads the plugin on each
# change, so building there (thousands of writes into target/) makes the whole
# desktop stutter and the wallpaper flicker. The plugin directory gets only the
# two files the shell needs, copied here at the end.
#
# Omarchy 4.5+ owns /usr/bin/omarchy-dock (its native dock's config script), so
# the binary goes to /usr/local/bin/omarchy-dock-plugin, which Service.qml
# supervises.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
plugin_id=io.github.chdevsec.omarchy-dock
plugin_dir="$HOME/.config/omarchy/plugins/$plugin_id"
# The id this fork had before it was published as CHDevSec's.
old_id=io.github.caio.dock
old_dir="$HOME/.config/omarchy/plugins/$old_id"
dest=/usr/local/bin/omarchy-dock-plugin

die() {
  printf '\nomarchy-dock: %s\n' "$1" >&2
  exit 1
}

case "$root" in
  "$HOME/.config/omarchy/plugins"/*) die "do not build inside the plugins directory (the shell reloads on every write); clone it somewhere else, e.g. ~/Projects/omarchy-dock" ;;
esac

command -v cargo >/dev/null 2>&1 || die "cargo is not installed: sudo pacman -S --needed rust"

echo "==> Building (release)"
cd "$root"
# Lower priority so the desktop stays responsive while rustc runs.
nice -n 10 cargo build --release --locked --bin omarchy-dock

echo "==> Installing $dest (needs sudo)"
sudo install -m755 target/release/omarchy-dock "$dest"

echo "==> Updating the plugin files in $plugin_dir"
mkdir -p "$plugin_dir"
for f in manifest.json Service.qml; do
  cmp -s "$f" "$plugin_dir/$f" || install -m644 "$f" "$plugin_dir/$f"
done

# One-time move from the old plugin id. Only a directory holding exactly the
# two files this script used to copy is removed; anything else is left alone.
moved=false
if [[ -d $old_dir ]]; then
  left=$(find "$old_dir" -mindepth 1 -maxdepth 1 ! -name manifest.json ! -name Service.qml | head -1)
  if [[ -z $left ]] && grep -q "\"$old_id\"" "$old_dir/manifest.json" 2>/dev/null; then
    echo "==> Removing the old plugin id $old_id"
    rm -rf "$old_dir"
    shell_json="$HOME/.config/omarchy/shell.json"
    if [[ -f $shell_json ]] && grep -q "\"$old_id\"" "$shell_json"; then
      sed -i "s/\"$old_id\"/\"$plugin_id\"/" "$shell_json"
    fi
    moved=true
  else
    echo "==> Leaving $old_dir alone: it holds files this script did not write"
  fi
fi

echo "==> Restarting the dock"
pkill -f '^(/usr/local/bin/)?omarchy-dock-plugin( |$)' || true
sleep 0.5
if $moved; then
  # The shell keeps watching a plugin directory it loaded even after it is
  # removed, so it has to rescan to pick up the new one. Its supervisor then
  # starts the dock.
  omarchy restart shell
else
  setsid uwsm-app -- omarchy-dock-plugin >/dev/null 2>&1 &
fi

echo "Done."
