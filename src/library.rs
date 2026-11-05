use crate::sc::Track;
use serde_json::{json, Value};
use std::path::Path;

pub const KIND: &str = "likes";

pub fn export(path: &Path, tracks: &[Track]) -> Result<(), String> {
    let items: Vec<Value> = tracks
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "title": t.title,
                "artist": t.artist,
                "duration": t.duration,
                "url": t.url,
                "artwork": t.artwork,
            })
        })
        .collect();
    let document = json!({
        "app": "vlswmplayer",
        "version": 1,
        "kind": KIND,
        "tracks": items,
    });
    let text = serde_json::to_string_pretty(&document).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

pub fn import(path: &Path) -> Result<Vec<Track>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let items = if let Some(array) = value.as_array() {
        array.clone()
    } else if let Some(array) = value.get("tracks").and_then(Value::as_array) {
        array.clone()
    } else {
        return Err("no tracks in file".to_owned());
    };

    Ok(items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_owned();
            Some(Track {
                id,
                title: item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("untitled")
                    .to_owned(),
                artist: item
                    .get("artist")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
                duration: item.get("duration").and_then(Value::as_u64).unwrap_or(0),
                url: item
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                artwork: item
                    .get("artwork")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                media: Vec::new(),
                track_authorization: String::new(),
                path: None,
            })
        })
        .collect())
}

/// Adds tracks that are not in `current` yet, returns how many were added.
pub fn merge(current: &mut Vec<Track>, incoming: Vec<Track>) -> usize {
    let mut added = 0;
    for track in incoming {
        if !current.iter().any(|existing| existing.id == track.id) {
            current.push(track);
            added += 1;
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("title {id}"),
            artist: "a".into(),
            duration: 100,
            url: "https://soundcloud.com/x".into(),
            artwork: Some("https://x/y.png".into()),
            ..Default::default()
        }
    }

    #[test]
    fn roundtrips_and_merges() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("likes.json");
        let source = vec![track("1"), track("2")];
        export(&path, &source).unwrap();

        let imported = import(&path).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[1].title, "title 2");

        let mut current = vec![track("1")];
        assert_eq!(merge(&mut current, imported), 1);
        assert_eq!(current.len(), 2);
    }

    #[test]
    fn imports_a_plain_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.json");
        std::fs::write(&path, r#"[{"id":"7","title":"t","artist":"a","duration":5}]"#).unwrap();
        let tracks = import(&path).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "7");
    }

    #[test]
    fn rejects_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(import(&path).is_err());
    }
}
