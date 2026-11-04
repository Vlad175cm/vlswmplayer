#!/bin/sh
# Installs vlswmplayer for the current user:
#   ~/.local/bin/vlswmplayer                      (the AppImage itself)
#   ~/.local/share/applications/vlswmplayer.desktop
#   ~/.local/share/icons/hicolor/256x256/apps/vlswmplayer.png
#
# Usage: ./install.sh [path/to/vlswmplayer-*.AppImage]
set -eu

REPO="Vlad175cm/vlswmplayer"
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

BIN_DIR="$HOME/.local/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
APP="$BIN_DIR/vlswmplayer"
ICON_DIR="$DATA_HOME/icons/hicolor/256x256/apps"
DESKTOP_DIR="$DATA_HOME/applications"
STATE_DIR="$DATA_HOME/vlswmplayer"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

find_appimage() {
    if [ "$#" -gt 0 ] && [ -n "$1" ]; then
        printf '%s\n' "$1"
        return 0
    fi
    set -- "$SCRIPT_DIR"/vlswmplayer-*-x86_64.AppImage
    [ -f "$1" ] || return 1
    ls -1 "$SCRIPT_DIR"/vlswmplayer-*-x86_64.AppImage | sort -V | tail -1
}

if ! SRC=$(find_appimage "$@"); then
    if [ -x "$SCRIPT_DIR/update.sh" ]; then
        say "no local AppImage, installing the latest release"
        exec "$SCRIPT_DIR/update.sh"
    fi
    die "AppImage not found.
Download it from https://github.com/$REPO/releases
or build with scripts/build-appimage.sh, then run ./install.sh <file>."
fi
[ -f "$SRC" ] || die "$SRC is not a file"

mkdir -p "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR" "$STATE_DIR"

say "installing $SRC"
install -m 755 "$SRC" "$APP"
chmod +x "$APP"

install_icon() {
    if [ -f "$SCRIPT_DIR/assets/vlswmplayer.png" ]; then
        install -m 644 "$SCRIPT_DIR/assets/vlswmplayer.png" "$ICON_DIR/vlswmplayer.png"
        return
    fi
    tmp=$(mktemp -d)
    (
        cd "$tmp" || exit 1
        "$APP" --appimage-extract \
            usr/share/icons/hicolor/256x256/apps/vlswmplayer.png >/dev/null 2>&1 || true
    )
    extracted="$tmp/squashfs-root/usr/share/icons/hicolor/256x256/apps/vlswmplayer.png"
    if [ -f "$extracted" ]; then
        install -m 644 "$extracted" "$ICON_DIR/vlswmplayer.png"
    fi
    rm -rf "$tmp"
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

version=$(basename "$SRC" | sed -n 's/^vlswmplayer-\(.*\)-x86_64\.AppImage$/\1/p')
[ -n "$version" ] || version=unknown
printf '%s\n' "$version" > "$STATE_DIR/version"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
fi

say ""
say "installed vlswmplayer $version"
say "  binary:  $APP"
say "  desktop: $DESKTOP_DIR/vlswmplayer.desktop"
say ""
say "It is now visible in launchers: rofi, wofi, quickshell, any .desktop reader."
say "Config and likes stay in ~/.config/vlswmplayer, cache in ~/.cache/vlswmplayer."
