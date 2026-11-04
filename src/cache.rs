use std::path::PathBuf;

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new() -> Self {
        Self::at(default_dir())
    }

    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn entry(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.audio"))
    }

    pub fn get(&self, id: &str) -> Option<PathBuf> {
        let path = self.entry(id);
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > 0 => Some(path),
            _ => None,
        }
    }

    pub fn path_for(&self, id: &str) -> PathBuf {
        std::fs::create_dir_all(&self.dir).ok();
        self.entry(id)
    }

    pub fn remove(&self, id: &str) {
        let _ = std::fs::remove_file(self.entry(id));
    }

    pub fn clear(&self) -> std::io::Result<usize> {
        if !self.dir.is_dir() {
            return Ok(0);
        }
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                let _ = std::fs::remove_file(entry.path());
                removed += 1;
            }
        }
        Ok(removed)
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("vlswmplayer/tracks");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home).join(".cache/vlswmplayer/tracks");
    }
    PathBuf::from("/tmp/vlswmplayer/tracks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn cache(dir: &Path) -> Cache {
        Cache::at(dir.to_path_buf())
    }

    #[test]
    fn stores_and_finds() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cache(tmp.path());
        assert!(c.get("42").is_none());
        let path = c.path_for("42");
        std::fs::write(&path, b"audio").unwrap();
        assert_eq!(c.get("42"), Some(path));
    }

    #[test]
    fn empty_file_is_not_a_hit() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cache(tmp.path());
        std::fs::create_dir_all(tmp.path()).unwrap();
        std::fs::write(c.path_for("7"), b"").unwrap();
        assert!(c.get("7").is_none());
    }

    #[test]
    fn removes_and_clears() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cache(tmp.path());
        for id in ["1", "2", "3"] {
            std::fs::write(c.path_for(id), b"x").unwrap();
        }
        c.remove("2");
        assert!(c.get("2").is_none());
        assert_eq!(c.clear().unwrap(), 2);
        assert!(c.get("1").is_none());
        assert!(c.get("3").is_none());
    }
}
