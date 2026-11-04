#!/bin/sh
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
BUILD="$ROOT/build"
APPDIR="$BUILD/vlswmplayer.AppDir"
OUT="$ROOT/vlswmplayer-$VERSION-x86_64.AppImage"

mkdir -p "$BUILD"
cargo build --release

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/vlswmplayer" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp target/release/vlswmplayer "$APPDIR/usr/bin/vlswmplayer"
cp assets/JetBrainsMonoNerdFont-Regular.ttf "$APPDIR/usr/share/vlswmplayer/"
cp assets/vlswmplayer.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/vlswmplayer.png"
cp assets/vlswmplayer.png "$APPDIR/vlswmplayer.png"
cp packaging/vlswmplayer.desktop "$APPDIR/usr/share/applications/"
cp packaging/vlswmplayer.desktop "$APPDIR/vlswmplayer.desktop"
cp packaging/AppRun "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"

if [ ! -x "$BUILD/yt-dlp" ]; then
    echo "downloading yt-dlp"
    curl -fsSL -o "$BUILD/yt-dlp" \
        https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux
    chmod +x "$BUILD/yt-dlp"
fi
cp "$BUILD/yt-dlp" "$APPDIR/usr/bin/yt-dlp"

if [ ! -x "$BUILD/appimagetool" ]; then
    echo "downloading appimagetool"
    curl -fsSL -o "$BUILD/appimagetool" \
        https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x "$BUILD/appimagetool"
fi

ARCH=x86_64 "$BUILD/appimagetool" --appimage-extract-and-run "$APPDIR" "$OUT"
echo "built $OUT"
