use crate::sc::Track;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "opus", "m4a", "aac", "wav", "aiff", "aif", "wma", "mka", "webm",
];

#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub active: bool,
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub fraction: f32,
    pub message: Option<String>,
}

impl Progress {
    pub fn overall(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            ((self.done as f32 + self.fraction) / self.total as f32).clamp(0.0, 1.0)
        }
    }
}

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("vlswmplayer");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home).join(".local/share/vlswmplayer");
    }
    PathBuf::from("/tmp/vlswmplayer")
}

fn to_value(track: &Track) -> serde_json::Value {
    serde_json::json!({
        "id": track.id,
        "title": track.title,
        "artist": track.artist,
        "duration": track.duration,
        "artwork": track.artwork,
        "file": track
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(""),
    })
}

fn from_value_in(base: &Path, value: &serde_json::Value) -> Option<Track> {
    let id = value.get("id")?.as_str()?.to_owned();
    let file = value.get("file")?.as_str()?.to_owned();
    let path = base.join("uploaded").join(file);
    Some(Track {
        id,
        title: value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("untitled")
            .to_owned(),
        artist: value
            .get("artist")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_owned(),
        duration: value.get("duration").and_then(|v| v.as_u64()).unwrap_or(0),
        url: path.to_string_lossy().into_owned(),
        artwork: value
            .get("artwork")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        media: Vec::new(),
        track_authorization: String::new(),
        path: Some(path),
    })
}

pub fn load() -> Vec<Track> {
    load_from(&data_dir())
}

pub fn load_from(base: &Path) -> Vec<Track> {
    let Ok(text) = std::fs::read_to_string(base.join("uploaded.json")) else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(&text)
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|value| from_value_in(base, value))
        .filter(|t| t.path.as_ref().is_some_and(|p| p.is_file()))
        .collect()
}

pub fn save(tracks: &[Track]) -> std::io::Result<()> {
    save_to(&data_dir(), tracks)
}

pub fn save_to(base: &Path, tracks: &[Track]) -> std::io::Result<()> {
    std::fs::create_dir_all(base)?;
    let items: Vec<serde_json::Value> = tracks.iter().map(to_value).collect();
    let text = serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_owned());
    std::fs::write(base.join("uploaded.json"), text)
}

fn ext_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn read_tags(path: &Path) -> (String, String, u64, Option<Vec<u8>>) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_owned();

    let Ok(tagged) = Probe::open(path).and_then(|p| p.read()) else {
        return (stem, "unknown".to_owned(), 0, None);
    };

    let duration = tagged.properties().duration().as_secs();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let Some(tag) = tag else {
        return (stem, "unknown".to_owned(), duration, None);
    };

    let title = tag.title().map(|t| t.to_string()).unwrap_or(stem);
    let artist = tag
        .artist()
        .map(|a| a.to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    let picture = tag.pictures().first().map(|p| p.data().to_vec());
    (title, artist, duration, picture)
}

fn copy_with_progress(
    source: &Path,
    target: &Path,
    progress: &Arc<Mutex<Progress>>,
) -> std::io::Result<()> {
    let mut input = std::fs::File::open(source)?;
    let mut output = std::fs::File::create(target)?;
    let size = input.metadata().map(|m| m.len()).unwrap_or(0);
    let mut copied: u64 = 0;
    let mut buffer = vec![0u8; 64 * 1024];

    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        copied += read as u64;
        if let Ok(mut state) = progress.lock() {
            state.fraction = if size > 0 {
                copied as f32 / size as f32
            } else {
                0.0
            };
        }
    }
    output.flush()?;
    Ok(())
}

/// Copies audio files into the app data directory and returns the new tracks.
pub fn import(
    files: &[PathBuf],
    existing: &[Track],
    progress: &Arc<Mutex<Progress>>,
) -> Vec<Track> {
    import_into(&data_dir(), files, existing, progress)
}

pub fn import_into(
    base: &Path,
    files: &[PathBuf],
    existing: &[Track],
    progress: &Arc<Mutex<Progress>>,
) -> Vec<Track> {
    let tracks_dir = base.join("uploaded");
    let artwork_dir = base.join("artwork");
    std::fs::create_dir_all(&tracks_dir).ok();
    std::fs::create_dir_all(&artwork_dir).ok();

    let usable: Vec<&PathBuf> = files.iter().filter(|p| ext_allowed(p)).collect();
    {
        let mut state = progress.lock().unwrap();
        state.active = true;
        state.total = usable.len();
        state.done = 0;
        state.fraction = 0.0;
        state.message = None;
        state.current.clear();
    }

    let mut added = Vec::new();
    for (index, source) in usable.iter().enumerate() {
        let file_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("track")
            .to_owned();
        {
            let mut state = progress.lock().unwrap();
            state.current = file_name.clone();
            state.fraction = 0.0;
        }

        let id = format!("local:{file_name}");
        if existing.iter().any(|t| t.id == id) || added.iter().any(|t: &Track| t.id == id) {
            let mut state = progress.lock().unwrap();
            state.done = index + 1;
            continue;
        }

        let (title, artist, duration, picture) = read_tags(source);
        let target = tracks_dir.join(&file_name);
        if copy_with_progress(source, &target, progress).is_err() {
            let mut state = progress.lock().unwrap();
            state.done = index + 1;
            continue;
        }

        let artwork = picture.and_then(|bytes| {
            let path = artwork_dir.join(format!("{}.img", id.replace(':', "_")));
            std::fs::write(&path, bytes).ok()?;
            Some(format!("file:{}", path.display()))
        });

        added.push(Track {
            id,
            title,
            artist,
            duration,
            url: target.to_string_lossy().into_owned(),
            artwork,
            media: Vec::new(),
            track_authorization: String::new(),
            path: Some(target),
        });

        let mut state = progress.lock().unwrap();
        state.done = index + 1;
        state.fraction = 0.0;
    }

    let mut state = progress.lock().unwrap();
    state.active = false;
    state.current.clear();
    if added.is_empty() {
        state.message = Some("NOTHING ADDED".to_owned());
    } else {
        state.message = Some(format!("ADDED {}", added.len()));
    }
    added
}


#[cfg(test)]
mod tests {
    use super::*;

    fn wav(path: &Path) {
        let samples: Vec<i16> = (0..8000).map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16).collect();
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&16000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn imports_a_file_into_a_base_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("My Track.wav");
        wav(&source);

        let progress = Arc::new(Mutex::new(Progress::default()));
        let added = import_into(tmp.path(), &[source], &[], &progress);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].title, "My Track");
        assert!(added[0].path.as_ref().unwrap().is_file());

        save_to(tmp.path(), &added).unwrap();
        let loaded = load_from(tmp.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "local:My Track.wav");
    }

    #[test]
    fn skips_duplicates_and_unknown_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("a.wav");
        wav(&source);
        let text = tmp.path().join("notes.txt");
        std::fs::write(&text, "hi").unwrap();

        let progress = Arc::new(Mutex::new(Progress::default()));
        let first = import_into(tmp.path(), &[source.clone()], &[], &progress);
        let again = import_into(tmp.path(), &[source, text], &first, &progress);
        assert!(again.is_empty());
    }

    #[test]
    fn progress_overall_is_clamped() {
        let mut progress = Progress::default();
        assert_eq!(progress.overall(), 0.0);
        progress.total = 4;
        progress.done = 2;
        progress.fraction = 0.5;
        assert!((progress.overall() - 0.625).abs() < f32::EPSILON);
    }
}
