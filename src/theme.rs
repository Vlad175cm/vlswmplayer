use std::path::PathBuf;

pub const BLACK: egui::Color32 = egui::Color32::BLACK;
pub const WHITE: egui::Color32 = egui::Color32::WHITE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Black,
    White,
}

impl Theme {
    pub fn is_white(self) -> bool {
        self == Theme::White
    }

    pub fn bg(self) -> egui::Color32 {
        if self.is_white() { WHITE } else { BLACK }
    }

    pub fn fg(self) -> egui::Color32 {
        if self.is_white() { BLACK } else { WHITE }
    }
}

pub fn parse(text: &str) -> Theme {
    if text.trim().eq_ignore_ascii_case("white") {
        Theme::White
    } else {
        Theme::Black
    }
}

pub fn state_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("vlswm/theme");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home).join(".local/state/vlswm/theme");
    }
    PathBuf::from("/tmp/vlswm/theme")
}

pub struct Watcher {
    path: PathBuf,
    text: String,
    theme: Theme,
}

impl Watcher {
    pub fn new() -> Self {
        Self::at(state_path())
    }

    pub fn at(path: PathBuf) -> Self {
        let (text, theme) = match std::fs::read_to_string(&path) {
            Ok(text) => (text.clone(), parse(&text)),
            Err(_) => (String::new(), Theme::Black),
        };
        Self { path, text, theme }
    }

    pub fn current(&self) -> Theme {
        self.theme
    }

    /// Reads the theme file and returns the new theme only when it really changed.
    pub fn poll(&mut self) -> Option<Theme> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => {
                if text == self.text {
                    return None;
                }
                self.text = text;
                let theme = parse(&self.text);
                if theme == self.theme {
                    None
                } else {
                    self.theme = theme;
                    Some(theme)
                }
            }
            Err(_) => {
                if self.theme == Theme::Black {
                    self.text.clear();
                    None
                } else {
                    self.text.clear();
                    self.theme = Theme::Black;
                    Some(Theme::Black)
                }
            }
        }
    }
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_black() {
        assert_eq!(parse("black\n"), Theme::Black);
    }

    #[test]
    fn parses_white() {
        assert_eq!(parse(" white "), Theme::White);
    }

    #[test]
    fn junk_is_black() {
        assert_eq!(parse("nonsense"), Theme::Black);
    }

    #[test]
    fn empty_is_black() {
        assert_eq!(parse(""), Theme::Black);
    }

    #[test]
    fn watcher_reports_only_real_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme");
        std::fs::write(&path, "black\n").unwrap();
        let mut w = Watcher::at(path.clone());
        assert_eq!(w.current(), Theme::Black);
        std::fs::write(&path, "white\n").unwrap();
        assert_eq!(w.poll(), Some(Theme::White));
        assert_eq!(w.poll(), None);
    }

    #[test]
    fn missing_file_is_black() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent");
        let mut w = Watcher::at(path);
        assert_eq!(w.current(), Theme::Black);
        assert_eq!(w.poll(), None);
    }

    #[test]
    fn disappearing_file_falls_back_to_black() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme");
        std::fs::write(&path, "white\n").unwrap();
        let mut w = Watcher::at(path.clone());
        assert_eq!(w.current(), Theme::White);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(w.poll(), Some(Theme::Black));
    }
}
