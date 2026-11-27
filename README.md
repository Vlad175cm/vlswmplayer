# vlswmplayer

English | [Русский](README.ru.md)

Hi, I'm **Vlad175cm**. This is my personal music player, written for the desktop
I actually use — [VLSWM](https://github.com/Vlad175cm/VLSWM), a minimal
black-and-white Wayland shell.

It plays SoundCloud without an account: type a query, press Enter, listen.
The interface is deliberately boring: only black and white, only square
corners, one monospace font. It should look like part of the shell, not like
a separate app on top of it.

![vlswmplayer](assets/vlswmplayer.png)

## What it does

- **ZEN** — my personal SoundCloud wave, built from the tracks I like. I group
  them into broad genres, pull similar tracks through SoundCloud's related
  endpoint and genre search, never repeat a track, and keep the same artist out
  of the last ten tracks. It is my attempt at a Yandex Music "My Wave" for
  SoundCloud.
- **Search** SoundCloud tracks without logging in, through the same public
  `api-v2` endpoint the web player uses.
- **Streams instantly** — the resolved HLS stream goes straight to `mpv`,
  no download before playback.
- **Caches what you play.** First play streams and downloads in the
  background; every next play starts from disk in well under a second.
- **Likes** — a local JSON list, a separate `LIKES` tab, `*` marker on rows.
- **Portable library** — the hamburger menu exports and imports likes as one
  JSON file, so the same library moves between Linux and iOS.
- **Follows the shell theme.** Reads the file VLSWM writes on `SUPER+T`, so the
  player flips between black and white together with the rest of the desktop.
- **Keyboard first**, mouse optional.

I removed the local-file uploader: the player is SoundCloud-only now.

## Screens

The ZEN wave, search results, the likes tab and the now-playing panel.
Everything is drawn with 1 px lines on a pure black or pure white background.

## Requirements

- Arch Linux (or an Arch-based distribution) on Wayland
- `mpv` — the playback engine
- `fuse2` — to run the AppImage

`yt-dlp` is bundled inside the AppImage, no separate installation needed.
JetBrainsMono Nerd Font is bundled too.

## Install

```bash
git clone https://github.com/Vlad175cm/vlswmplayer.git
cd vlswmplayer
./install.sh
```

`install.sh` downloads the latest release and puts everything in place:

- `~/.local/bin/vlswmplayer` — the AppImage itself, executable
- `~/.local/share/applications/vlswmplayer.desktop` — visible in rofi, wofi,
  quickshell and any other `.desktop` reader
- `~/.local/share/icons/hicolor/256x256/apps/vlswmplayer.png`

Pass a local file if you already downloaded one:
`./install.sh ~/Downloads/vlswmplayer-1.4.0-x86_64.AppImage`.
The file is copied, so the build in the source tree stays where it is.

### Update

```bash
cd vlswmplayer
./update.sh
```

It pulls the newest release from GitHub and replaces the binary, the desktop
entry and the icon. Your likes (`~/.config/vlswmplayer`) and the track cache
(`~/.cache/vlswmplayer`) are never touched.

### Run without installing

```bash
chmod +x vlswmplayer-1.4.0-x86_64.AppImage
./vlswmplayer-1.4.0-x86_64.AppImage
```

If FUSE is not available:

```bash
./vlswmplayer-1.4.0-x86_64.AppImage --appimage-extract-and-run
```

## Controls

| Key | Action |
| --- | --- |
| `SPACE` | play / pause |
| `←` / `→` | previous / next track |
| `SHIFT+←` / `SHIFT+→` | seek ∓5 s |
| `↑` / `↓` | volume ±5 |
| `TAB` | switch `ZEN` / `SEARCH` / `LIKES` |
| `F` | like or unlike the selected track (on ZEN — the current one) |
| `M` | open the menu (export or import likes) |
| `1` / `2` | menu items while the menu is open |
| `CTRL+SHIFT+C` | clear the track cache |
| `/` | focus the search field |
| `ENTER` | play the selected track; on ZEN — start or pause the wave |
| `Q` | quit |

Rows are clickable, progress and volume bars are draggable, the mouse wheel
scrolls the list.

## Files

| Path | Contents |
| --- | --- |
| `~/.config/vlswmplayer/config` | volume, last query, UI scale |
| `~/.config/vlswmplayer/likes.json` | liked tracks |
| `~/.cache/vlswmplayer/tracks/` | cached audio, one file per track |
| `~/.cache/vlswmplayer/client_id` | SoundCloud client id refreshed from the website |

Config example:

```ini
volume = 70
last_query =
scale = 0.85
```

## How it works

- Search: `GET https://api-v2.soundcloud.com/search/tracks` with a `client_id`
  from the player's cache. If the id goes stale, the player scrapes a fresh one
  from `soundcloud.com` and caches it; `yt-dlp scsearch10` is the last fallback.
- ZEN: liked tracks are hydrated in batches (`/tracks?ids=`), grouped into
  HIP-HOP / ELECTRONIC / HARDSTYLE / AMBIENT / ROCK / POP / OTHER, and used as
  seeds for `/tracks/{id}/related` and genre search. Candidates are filtered by
  media availability, duration, already-played ids and the last ten artists.
  The queue holds 20 tracks and refills below five.
- Playback: the track's `media.transcodings` entry is resolved into a direct
  HLS URL and handed to `mpv` over its JSON IPC socket. Encrypted
  (`*-encrypted-hls`) tracks are filtered out.
- State: mpv pushes `time-pos`, `duration`, `pause` and `idle-active` as
  property changes; the UI only reads them.
- Cache: `yt-dlp` downloads the same track into `~/.cache/vlswmplayer/tracks`
  while it plays, so the next start needs no network.

## Build from source

```bash
cargo build --release
bash scripts/build-appimage.sh
```

The script downloads `yt-dlp` and `appimagetool`, packs the binary, the font,
the icon and the `.desktop` file, and writes
`vlswmplayer-<version>-x86_64.AppImage` next to the sources.

## Notes

- SoundCloud's public endpoints are not an official API. They can change, and
  the `client_id` can expire; the `yt-dlp` fallback exists for that reason.
- The player does not touch your SoundCloud account: no login, no likes
  written back to the service.
- Tracks protected by DRM are skipped.

## License

[MIT](LICENSE). The bundled JetBrains Mono font is licensed under the
[SIL Open Font License 1.1](assets/JetBrainsMono-OFL.txt).
