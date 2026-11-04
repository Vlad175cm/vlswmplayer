use std::path::{Path, PathBuf};

pub const DEFAULT_VOLUME: u8 = 80;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub volume: u8,
    pub last_query: String,
    pub scale: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            volume: DEFAULT_VOLUME,
            last_query: String::new(),
            scale: 0.85,
        }
    }
}

pub fn parse(text: &str) -> Config {
    let mut config = Config::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "volume" => {
                if let Ok(v) = value.parse::<i32>() {
                    config.volume = v.clamp(0, 100) as u8;
                }
            }
            "last_query" => config.last_query = value.to_owned(),
            "scale" => {
                if let Ok(v) = value.parse::<f32>() {
                    config.scale = v.clamp(0.5, 1.5);
                }
            }
            _ => {}
        }
    }
    config
}

impl Config {
    pub fn to_text(&self) -> String {
        format!(
            "volume = {}\nlast_query = {}\nscale = {}\n",
            self.volume, self.last_query, self.scale
        )
    }

    pub fn load(path: &Path) -> Config {
        std::fs::read_to_string(path)
            .map(|text| parse(&text))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_text())
    }
}

pub fn default_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("vlswmplayer/config");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home).join(".config/vlswmplayer/config");
    }
    PathBuf::from("/tmp/vlswmplayer/config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = parse("");
        assert_eq!(c.volume, 80);
        assert_eq!(c.last_query, "");
    }

    #[test]
    fn reads_values() {
        let c = parse("volume = 55\nlast_query = aphex twin\n");
        assert_eq!(c.volume, 55);
        assert_eq!(c.last_query, "aphex twin");
    }

    #[test]
    fn scale_is_clamped() {
        assert_eq!(parse("scale = 0.7").scale, 0.7);
        assert_eq!(parse("scale = 9").scale, 1.5);
        assert_eq!(parse("scale = 0.1").scale, 0.5);
    }

    #[test]
    fn clamps_volume() {
        assert_eq!(parse("volume = 300").volume, 100);
        assert_eq!(parse("volume = -4").volume, 0);
    }

    #[test]
    fn roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let c = Config {
            volume: 42,
            last_query: "xtal".into(),
            scale: 0.8,
        };
        c.save(&path).unwrap();
        let back = Config::load(&path);
        assert_eq!(back.volume, 42);
        assert_eq!(back.last_query, "xtal");
        assert_eq!(back.scale, 0.8);
    }
}
