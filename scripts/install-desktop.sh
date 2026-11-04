#!/bin/sh
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP=$(ls "$ROOT"/vlswmplayer-*-x86_64.AppImage 2>/dev/null | head -1)
if [ -z "$APP" ]; then
    echo "no AppImage found, run scripts/build-appimage.sh first" >&2
    exit 1
fi

DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$DATA/applications" "$DATA/icons/hicolor/256x256/apps"

cp "$ROOT/assets/vlswmplayer.png" \
   "$DATA/icons/hicolor/256x256/apps/vlswmplayer.png"

cat > "$DATA/applications/vlswmplayer.desktop" <<EOF
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

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DATA/applications" >/dev/null 2>&1 || true
fi

echo "installed: $DATA/applications/vlswmplayer.desktop"
echo "exec:      $APP"
