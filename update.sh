#!/bin/sh
# Downloads the latest GitHub release and reinstalls the player for the current user.
# Cache (~/.cache/vlswmplayer) and likes (~/.config/vlswmplayer) are never touched.
#
# Usage: ./update.sh
set -eu

REPO="Vlad175cm/vlswmplayer"
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
API="https://api.github.com/repos/$REPO/releases/latest"

BIN_DIR="$HOME/.local/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
APP="$BIN_DIR/vlswmplayer"
ICON_DIR="$DATA_HOME/icons/hicolor/256x256/apps"
DESKTOP_DIR="$DATA_HOME/applications"
STATE_DIR="$DATA_HOME/vlswmplayer"
VERSION_FILE="$STATE_DIR/version"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$1"
    else
        die "neither curl nor wget is installed"
    fi
}

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fL --progress-bar -o "$2" "$1"
    else
        wget -q --show-progress -O "$2" "$1"
    fi
}

json=$(fetch "$API") || die "cannot reach GitHub API"
latest=$(printf '%s\n' "$json" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
asset=$(printf '%s\n' "$json" | sed -n 's/.*"browser_download_url": *"\([^"]*x86_64\.AppImage\)".*/\1/p' | head -1)
[ -n "$latest" ] || die "no release found"
[ -n "$asset" ] || die "release $latest has no x86_64 AppImage"

latest_version=$(printf '%s' "$latest" | sed 's/^v//')
current=$(cat "$VERSION_FILE" 2>/dev/null || echo "not installed")
say "installed: $current"
say "latest:    $latest_version"
if [ "$current" = "$latest_version" ]; then
    say "already up to date, reinstalling anyway"
fi

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
say "downloading $asset"
download "$asset" "$tmp"
chmod +x "$tmp"
"$tmp" --appimage-offset >/dev/null 2>&1 || die "downloaded file is not a valid AppImage"

mkdir -p "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR" "$STATE_DIR"
install -m 755 "$tmp" "$APP"
chmod +x "$APP"

install_icon() {
    if [ -f "$SCRIPT_DIR/assets/vlswmplayer.png" ]; then
        install -m 644 "$SCRIPT_DIR/assets/vlswmplayer.png" "$ICON_DIR/vlswmplayer.png"
        return
    fi
    dir=$(mktemp -d)
    (
        cd "$dir" || exit 1
        "$APP" --appimage-extract \
            usr/share/icons/hicolor/256x256/apps/vlswmplayer.png >/dev/null 2>&1 || true
    )
    extracted="$dir/squashfs-root/usr/share/icons/hicolor/256x256/apps/vlswmplayer.png"
    if [ -f "$extracted" ]; then
        install -m 644 "$extracted" "$ICON_DIR/vlswmplayer.png"
    fi
    rm -rf "$dir"
}
install_icon

cat > "$DESKTOP_DIR/vlswmplayer.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=vlswmplayer
Comment=Minimal SoundCloud player for VLSWM
Exec=$APP
Icon=vlswmplayer
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=vlswmplayer
EOF
chmod 644 "$DESKTOP_DIR/vlswmplayer.desktop"
printf '%s\n' "$latest_version" > "$VERSION_FILE"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
fi

say ""
say "updated to $latest_version"
say "cache and likes untouched"
