use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcoding {
    pub url: String,
    pub preset: String,
    pub protocol: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: u64,
    pub url: String,
    pub artwork: Option<String>,
    pub media: Vec<Transcoding>,
    pub track_authorization: String,
    pub path: Option<std::path::PathBuf>,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            artist: String::new(),
            duration: 0,
            url: String::new(),
            artwork: None,
            media: Vec::new(),
            track_authorization: String::new(),
            path: None,
        }
    }
}


fn text(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn id_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn parse_search(json: &str) -> Result<Vec<Track>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let Some(entries) = root.get("entries").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut tracks = Vec::new();
    for entry in entries {
        let Some(id) = entry.get("id").and_then(id_of) else {
            continue;
        };
        let Some(url) = text(entry.get("url")) else {
            continue;
        };
        let duration = entry
            .get("duration")
            .and_then(Value::as_f64)
            .map(|d| d.round() as u64)
            .unwrap_or(0);
        let artwork = entry
            .get("thumbnails")
            .and_then(Value::as_array)
            .and_then(|list| list.last())
            .and_then(|thumb| text(thumb.get("url")))
            .map(str::to_owned);

        tracks.push(Track {
            id,
            title: text(entry.get("title")).unwrap_or("untitled").to_owned(),
            artist: text(entry.get("uploader")).unwrap_or("unknown").to_owned(),
            duration,
            url: url.to_owned(),
            artwork,
            media: Vec::new(),
            track_authorization: String::new(),
            path: None,
        });
    }
    Ok(tracks)
}

pub fn search_args(query: &str) -> Vec<String> {
    vec![
        "--flat-playlist".to_owned(),
        "-J".to_owned(),
        "--no-warnings".to_owned(),
        "--socket-timeout".to_owned(),
        "8".to_owned(),
        format!("scsearch10:{query}"),
    ]
}

pub fn resolve_ytdlp(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }

    if let Some(path) = std::env::var_os("VLSWMPLAYER_YTDLP") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("yt-dlp");
            if bundled.is_file() {
                return Some(bundled);
            }
        }
    }

    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join("yt-dlp"))
        .find(|candidate| candidate.is_file())
}

pub fn search(query: &str, ytdlp: &Path) -> Result<Vec<Track>, String> {
    search_with_timeout(query, ytdlp, std::time::Duration::from_secs(20))
}

pub fn search_with_timeout(
    query: &str,
    ytdlp: &Path,
    timeout: std::time::Duration,
) -> Result<Vec<Track>, String> {
    use std::io::Read;

    let mut child = std::process::Command::new(ytdlp)
        .args(search_args(query))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let deadline = std::time::Instant::now() + timeout;

    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                let mut out = Vec::new();
                let mut err = Vec::new();
                stdout.read_to_end(&mut out).map_err(|e| e.to_string())?;
                stderr.read_to_end(&mut err).map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err(String::from_utf8_lossy(&err).trim().to_owned());
                }
                return parse_search(&String::from_utf8_lossy(&out));
            }
            None => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("TIMEOUT".to_owned());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn http_get(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(8)))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64)")
        .call()
        .map_err(|e| e.to_string())?;
    let body = response.body_mut().read_to_vec().map_err(|e| e.to_string())?;
    String::from_utf8(body).map_err(|e| e.to_string())
}

pub fn parse_client_id(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    value
        .get("data")
        .and_then(Value::as_str)
        .filter(|s| s.len() >= 20)
        .map(str::to_owned)
}

pub fn client_id() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".cache/yt-dlp/soundcloud/client_id.json");
    parse_client_id(&std::fs::read_to_string(path).ok()?)
}

fn artwork_big(url: &str) -> String {
    url.replace("-large.", "-t500x500.")
}

pub fn parse_api_search(json: &str) -> Result<Vec<Track>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let Some(items) = root.get("collection").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut tracks = Vec::new();
    for item in items {
        if item.get("kind").and_then(Value::as_str) != Some("track") {
            continue;
        }
        if let Some(track) = track_from_api(item) {
            if !track.media.is_empty() {
                tracks.push(track);
            }
        }
    }
    Ok(tracks)
}

fn track_from_api(item: &Value) -> Option<Track> {
    let id = item.get("id").and_then(id_of)?;
    let permalink = text(item.get("permalink_url"))?.to_owned();
    let duration = item
        .get("duration")
        .and_then(Value::as_f64)
        .map(|ms| (ms / 1000.0).round() as u64)
        .unwrap_or(0);
    let media = item
        .get("media")
        .and_then(|m| m.get("transcodings"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|t| {
                    let protocol = text(t.get("format").and_then(|f| f.get("protocol")))
                        .unwrap_or("")
                        .to_owned();
                    if protocol.contains("encrypted") {
                        return None;
                    }
                    Some(Transcoding {
                        url: text(t.get("url"))?.to_owned(),
                        preset: text(t.get("preset")).unwrap_or("").to_owned(),
                        protocol,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(Track {
        id,
        title: text(item.get("title")).unwrap_or("untitled").to_owned(),
        artist: text(item.get("user").and_then(|u| u.get("username")))
            .unwrap_or("unknown")
            .to_owned(),
        duration,
        url: permalink,
        artwork: text(item.get("artwork_url")).map(artwork_big),
        media,
        track_authorization: text(item.get("track_authorization"))
            .unwrap_or("")
            .to_owned(),
        path: None,
    })
}

pub fn search_api(query: &str, cid: &str) -> Result<Vec<Track>, String> {
    let url = format!(
        "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit=10",
        encode(query),
        encode(cid)
    );
    parse_api_search(&http_get(&url)?)
}

pub fn api_track(id: &str, cid: &str) -> Result<Track, String> {
    let url = format!(
        "https://api-v2.soundcloud.com/tracks/{}?client_id={}",
        encode(id),
        encode(cid)
    );
    let value: Value = serde_json::from_str(&http_get(&url)?).map_err(|e| e.to_string())?;
    track_from_api(&value).ok_or_else(|| "bad track json".to_owned())
}

pub fn pick_transcoding(media: &[Transcoding]) -> Option<&Transcoding> {
    for preset in ["aac_160k", "abr_sq", "aac_96k"] {
        if let Some(found) = media.iter().find(|t| t.preset == preset) {
            return Some(found);
        }
    }
    media.first()
}

pub fn parse_stream_url(json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(json).ok()?;
    value
        .get("url")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

pub fn resolve_stream(track: &Track, cid: &str) -> Result<String, String> {
    let transcoding = pick_transcoding(&track.media).ok_or("no transcoding")?;
    let url = format!(
        "{}?client_id={}&track_authorization={}",
        transcoding.url,
        encode(cid),
        encode(&track.track_authorization)
    );
    parse_stream_url(&http_get(&url)?).ok_or_else(|| "no stream url".to_owned())
}

pub fn cache_args(url: &str, out: &Path) -> Vec<String> {
    vec![
        "-f".to_owned(),
        "bestaudio".to_owned(),
        "--no-playlist".to_owned(),
        "--no-warnings".to_owned(),
        "--socket-timeout".to_owned(),
        "15".to_owned(),
        "-o".to_owned(),
        out.to_string_lossy().into_owned(),
        url.to_owned(),
    ]
}

pub fn ytdlp_stream(url: &str, ytdlp: &Path) -> Result<String, String> {
    let output = std::process::Command::new(ytdlp)
        .args([
            "-g",
            "-f",
            "bestaudio",
            "--no-warnings",
            "--socket-timeout",
            "15",
            url,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_owned)
        .filter(|line| line.starts_with("http"))
        .ok_or_else(|| "no url".to_owned())
}

/// Fast path: api-v2 with the cached client id, falling back to yt-dlp.
pub fn search_fast(query: &str, ytdlp: &Path) -> Result<Vec<Track>, String> {
    if let Some(cid) = client_id() {
        if let Ok(tracks) = search_api(query, &cid) {
            return Ok(tracks);
        }
    }
    search(query, ytdlp)
}

/// Stream URL for a track: api-v2 first, then yt-dlp.
pub fn stream_url(track: &Track, ytdlp: &Path) -> Result<String, String> {
    if let Some(cid) = client_id() {
        if let Ok(ready) = hydrate(track, &cid) {
            if let Ok(url) = resolve_stream(&ready, &cid) {
                return Ok(url);
            }
        }
    }
    ytdlp_stream(&track.url, ytdlp)
}

/// Tracks restored from likes carry no media, so ask api-v2 for a fresh copy.
pub fn hydrate(track: &Track, cid: &str) -> Result<Track, String> {
    if track.media.is_empty() {
        api_track(&track.id, cid)
    } else {
        Ok(track.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/search.json");

    #[test]
    fn parses_fixture() {
        let tracks = parse_search(FIXTURE).unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].id, "2268857915");
        assert_eq!(tracks[0].title, "Xtal");
        assert_eq!(tracks[0].artist, "aphextwin");
        assert_eq!(tracks[0].duration, 293);
        assert_eq!(
            tracks[0].artwork.as_deref(),
            Some("https://i1.sndcdn.com/a-original.png")
        );
    }

    #[test]
    fn missing_artist_and_duration_are_safe() {
        let tracks = parse_search(FIXTURE).unwrap();
        assert_eq!(tracks[1].artist, "unknown");
        assert_eq!(tracks[1].duration, 228);
        assert_eq!(tracks[2].duration, 0);
        assert_eq!(tracks[2].artwork, None);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_search("not json").is_err());
        assert!(parse_search("{}").is_ok());
        assert_eq!(parse_search("{}").unwrap().len(), 0);
    }

    #[test]
    fn entries_without_url_are_skipped() {
        let json = r#"{"entries":[{"id":"1","title":"no url"},{"id":"2","url":"u"}]}"#;
        let tracks = parse_search(json).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "2");
    }

    #[test]
    fn search_args_request_json() {
        let a = search_args("aphex twin");
        assert!(a.contains(&"scsearch10:aphex twin".to_string()));
        assert!(a.contains(&"-J".to_string()));
        assert!(a.contains(&"--flat-playlist".to_string()));
    }

    #[test]
    fn search_times_out_instead_of_hanging() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("yt-dlp");
        std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let started = std::time::Instant::now();
        let result = search_with_timeout("x", &fake, std::time::Duration::from_millis(300));
        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

#[test]
    #[ignore]
    fn manual_search() {
        let ytdlp = resolve_ytdlp(None).expect("yt-dlp not found");
        let tracks = search("aphex twin", &ytdlp).unwrap();
        assert!(!tracks.is_empty());
        for t in tracks.iter().take(3) {
            println!("{} — {} ({})", t.artist, t.title, t.duration);
        }
    }

    #[test]
    fn parses_client_id_cache() {
        let good = r#"{"yt-dlp_version":"1","data":"abcdefghijklmnopqrstuvwxyz012345"}"#;
        assert_eq!(
            parse_client_id(good).as_deref(),
            Some("abcdefghijklmnopqrstuvwxyz012345")
        );
        assert_eq!(parse_client_id("junk"), None);
        assert_eq!(parse_client_id(r#"{"data":"short"}"#), None);
    }

    #[test]
    fn parses_api_search() {
        let json = include_str!("../tests/fixtures/api_search.json");
        let tracks = parse_api_search(json).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].artist, "idk");
        assert_eq!(tracks[0].duration, 228);
        assert_eq!(
            tracks[0].artwork.as_deref(),
            Some("https://i1.sndcdn.com/artworks-abc-t500x500.png")
        );
        assert_eq!(tracks[0].media.len(), 2);
        assert_eq!(tracks[1].artwork, None);
        assert_eq!(tracks[1].media.len(), 1);
    }

    #[test]
    fn picks_160k_transcoding() {
        let media = vec![
            Transcoding {
                url: "abr".into(),
                preset: "abr_sq".into(),
                protocol: "hls".into(),
            },
            Transcoding {
                url: "best".into(),
                preset: "aac_160k".into(),
                protocol: "hls".into(),
            },
        ];
        assert_eq!(pick_transcoding(&media).unwrap().url, "best");
    }

    #[test]
    fn drops_drm_only_tracks() {
        let json = r#"{"collection":[
            {"id":1,"kind":"track","title":"drm","permalink_url":"p",
             "media":{"transcodings":[{"url":"u","preset":"aac_160k",
             "format":{"protocol":"cbc-encrypted-hls"}}]}},
            {"id":2,"kind":"track","title":"ok","permalink_url":"p",
             "media":{"transcodings":[{"url":"u","preset":"aac_160k",
             "format":{"protocol":"hls"}}]}}
        ]}"#;
        let tracks = parse_api_search(json).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "ok");
    }

    #[test]
    fn encodes_query() {
        assert_eq!(encode("aphex twin"), "aphex%20twin");
        assert_eq!(encode("a/b"), "a%2Fb");
    }

    #[test]
    fn cache_args_write_to_a_fixed_file() {
        let args = cache_args("https://example/track", std::path::Path::new("/tmp/42.audio"));
        assert!(args.contains(&"bestaudio".to_string()));
        assert!(args.contains(&"/tmp/42.audio".to_string()));
    }

    #[test]
    fn parses_stream_url() {
        assert_eq!(
            parse_stream_url(r#"{"url":"https://x/y"}"#).as_deref(),
            Some("https://x/y")
        );
        assert_eq!(parse_stream_url("{}"), None);
    }
}
