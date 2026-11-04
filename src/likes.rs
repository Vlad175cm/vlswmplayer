use crate::sc::Track;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn default_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("vlswmplayer/likes.json");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home).join(".config/vlswmplayer/likes.json");
    }
    PathBuf::from("/tmp/vlswmplayer/likes.json")
}

fn to_value(track: &Track) -> Value {
    json!({
        "id": track.id,
        "title": track.title,
        "artist": track.artist,
        "duration": track.duration,
        "url": track.url,
        "artwork": track.artwork,
    })
}

fn from_value(value: &Value) -> Option<Track> {
    let id = value.get("id")?.as_str()?.to_owned();
    Some(Track {
        id,
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("untitled")
            .to_owned(),
        artist: value
            .get("artist")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        duration: value.get("duration").and_then(Value::as_u64).unwrap_or(0),
        url: value
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        artwork: value
            .get("artwork")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..Default::default()
    })
}

pub fn parse(text: &str) -> Vec<Track> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    items.iter().filter_map(from_value).collect()
}

pub fn to_text(tracks: &[Track]) -> String {
    let items: Vec<Value> = tracks.iter().map(to_value).collect();
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_owned())
}

pub fn load(path: &Path) -> Vec<Track> {
    std::fs::read_to_string(path)
        .map(|text| parse(&text))
        .unwrap_or_default()
}

pub fn save(path: &Path, tracks: &[Track]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, to_text(tracks))
}

pub fn is_liked(tracks: &[Track], id: &str) -> bool {
    tracks.iter().any(|t| t.id == id)
}

/// Adds the track if absent, removes it otherwise. Returns true when it is liked now.
pub fn toggle(tracks: &mut Vec<Track>, track: &Track) -> bool {
    if let Some(index) = tracks.iter().position(|t| t.id == track.id) {
        tracks.remove(index);
        false
    } else {
        tracks.insert(0, track.clone());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("title {id}"),
            artist: "a".into(),
            duration: 123,
            url: "https://soundcloud.com/a/b".into(),
            artwork: Some("https://x/y.png".into()),
            ..Default::default()
        }
    }

    #[test]
    fn toggles_on_and_off() {
        let mut list = Vec::new();
        assert!(toggle(&mut list, &track("1")));
        assert!(is_liked(&list, "1"));
        assert!(!toggle(&mut list, &track("1")));
        assert!(!is_liked(&list, "1"));
    }

    #[test]
    fn newest_like_is_first() {
        let mut list = Vec::new();
        toggle(&mut list, &track("1"));
        toggle(&mut list, &track("2"));
        assert_eq!(list[0].id, "2");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn roundtrips_through_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("likes.json");
        let list = vec![track("1"), track("2")];
        save(&path, &list).unwrap();
        let back = load(&path);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].title, "title 1");
        assert_eq!(back[1].artwork.as_deref(), Some("https://x/y.png"));
    }

    #[test]
    fn corrupt_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("likes.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_empty());
        assert!(parse("[]").is_empty());
        assert!(parse("{}").is_empty());
    }
}
