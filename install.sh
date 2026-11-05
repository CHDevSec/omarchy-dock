#!/usr/bin/env bash
#
# Build and install Caio Dock (fork of omarchy-dock) from this checkout.
#
#   ~/Projects/caio-dock/install.sh
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
plugin_dir="$HOME/.config/omarchy/plugins/io.github.caio.dock"
dest=/usr/local/bin/omarchy-dock-plugin

die() {
  printf '\ncaio-dock: %s\n' "$1" >&2
  exit 1
}

case "$root" in
  "$HOME/.config/omarchy/plugins"/*) die "do not build inside the plugins directory (the shell reloads on every write); use ~/Projects/caio-dock" ;;
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

echo "==> Restarting the dock"
pkill -f '^(/usr/local/bin/)?omarchy-dock-plugin( |$)' || true
sleep 0.5
setsid uwsm-app -- omarchy-dock-plugin >/dev/null 2>&1 &

echo "Done."
