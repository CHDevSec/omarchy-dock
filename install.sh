#!/usr/bin/env bash
#
# Build and install Caio Dock (fork of omarchy-dock) from this checkout.
#
#   cd ~/.config/omarchy/plugins/io.github.caio.dock && ./install.sh
#
# Omarchy 4.5+ owns /usr/bin/omarchy-dock (its native dock's config script), so
# this fork installs its binary as /usr/local/bin/omarchy-dock-plugin, which is
# what Service.qml supervises. No pacman package is involved.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest=/usr/local/bin/omarchy-dock-plugin

die() {
  printf '\ncaio-dock: %s\n' "$1" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || die "cargo is not installed: sudo pacman -S --needed rust"

echo "==> Building (release)"
cd "$root"
cargo build --release --locked --bin omarchy-dock

echo "==> Installing $dest (needs sudo)"
sudo install -m755 target/release/omarchy-dock "$dest"

echo "==> Restarting the dock"
pkill -f '^(/usr/local/bin/)?omarchy-dock-plugin( |$)' || true
sleep 0.5
setsid uwsm-app -- omarchy-dock-plugin >/dev/null 2>&1 &

echo "Done."
