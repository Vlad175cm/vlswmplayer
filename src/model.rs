use crate::likes;
use crate::sc::Track;

pub const DEBOUNCE_MS: u64 = 400;
pub const MIN_QUERY_LEN: usize = 2;

pub enum Msg {
    SearchDone {
        serial: u64,
        result: Result<Vec<Track>, String>,
    },
    ArtworkDone {
        id: String,
        result: Result<egui::ColorImage, String>,
    },
    StreamReady {
        serial: u64,
        result: Result<(Track, String), String>,
    },
    CacheDone {
        id: String,
        result: Result<(), String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Search,
    Likes,
}

pub struct Model {
    pub query: String,
    pub results: Vec<Track>,
    pub selected: usize,
    pub search_gen: u64,
    pub download_gen: u64,
    pub searching: bool,
    pub status: Option<String>,
    pub current: Option<Track>,
    pub pending_artwork: Option<egui::ColorImage>,
    pub view: View,
    pub liked: Vec<Track>,
    pub scroll: usize,
}

impl Model {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            search_gen: 0,
            download_gen: 0,
            searching: false,
            status: None,
            current: None,
            pending_artwork: None,
            view: View::Search,
            liked: Vec::new(),
            scroll: 0,
        }
    }

    /// Applies a message. Returns a downloaded path when a track is ready to play.
    pub fn apply(&mut self, msg: Msg) -> Option<std::path::PathBuf> {
        match msg {
            Msg::SearchDone { serial, result } => {
                if serial != self.search_gen {
                    return None;
                }
                self.searching = false;
                match result {
                    Ok(tracks) => {
                        self.results = tracks;
                        self.selected = 0;
                        self.scroll = 0;
                        self.status = None;
                    }
                    Err(_) => {
                        self.results.clear();
                        self.selected = 0;
                        self.status = Some("NO CONNECTION".to_owned());
                    }
                }
                None
            }
            Msg::StreamReady { .. } | Msg::CacheDone { .. } => None,
            Msg::ArtworkDone { id, result } => {
                if let (Some(current), Ok(image)) = (&self.current, result) {
                    if current.id == id {
                        self.pending_artwork = Some(image);
                    }
                }
                None
            }
        }
    }

    pub fn select(&mut self, index: usize) {
        if index < self.results.len() {
            self.selected = index;
            self.current = Some(self.results[index].clone());
        }
    }

    pub fn visible(&self) -> &[Track] {
        match self.view {
            View::Search => &self.results,
            View::Likes => &self.liked,
        }
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn scroll_by(&mut self, delta: i64, capacity: usize) {
        let len = self.visible().len();
        let max = len.saturating_sub(capacity.max(1));
        let next = (self.scroll as i64 + delta).clamp(0, max as i64);
        self.scroll = next as usize;
    }

    /// Keeps the selected row inside the window of `capacity` visible rows.
    pub fn ensure_visible(&mut self, capacity: usize) {
        let capacity = capacity.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + capacity {
            self.scroll = self.selected + 1 - capacity;
        }
        let len = self.visible().len();
        self.scroll = self.scroll.min(len.saturating_sub(capacity));
    }

    pub fn toggle_view(&mut self) {
        self.set_view(match self.view {
            View::Search => View::Likes,
            View::Likes => View::Search,
        });
    }

    /// Likes or unlikes the selected visible track. Returns the new state.
    pub fn toggle_like(&mut self) -> Option<bool> {
        let track = self.visible().get(self.selected)?.clone();
        let liked = likes::toggle(&mut self.liked, &track);
        if self.view == View::Likes {
            self.selected = self.selected.min(self.liked.len().saturating_sub(1));
        }
        Some(liked)
    }

    pub fn next_track(&mut self) -> Option<Track> {
        if self.selected + 1 >= self.visible().len() {
            return None;
        }
        self.selected += 1;
        let track = self.visible()[self.selected].clone();
        self.current = Some(track.clone());
        Some(track)
    }

    pub fn prev_track(&mut self) -> Option<Track> {
        if self.selected == 0 || self.visible().is_empty() {
            return None;
        }
        self.selected -= 1;
        let track = self.visible()[self.selected].clone();
        self.current = Some(track.clone());
        Some(track)
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

pub fn should_search(elapsed_ms: u64, len: usize) -> bool {
    elapsed_ms >= DEBOUNCE_MS && len >= MIN_QUERY_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str) -> Track {
        Track {
            id: title.into(),
            title: title.into(),
            artist: "a".into(),
            duration: 100,
            url: "u".into(),
            ..Default::default()
        }
    }

    #[test]
    fn scroll_clamps_to_the_list() {
        let mut m = Model::new();
        m.liked = (0..30).map(|i| track(&format!("t{i}"))).collect();
        m.set_view(View::Likes);
        m.scroll_by(10, 20);
        assert_eq!(m.scroll, 10);
        m.scroll_by(10, 20);
        assert_eq!(m.scroll, 10);
        m.scroll_by(-100, 20);
        assert_eq!(m.scroll, 0);
    }

    #[test]
    fn selection_pulls_the_window() {
        let mut m = Model::new();
        m.liked = (0..30).map(|i| track(&format!("t{i}"))).collect();
        m.set_view(View::Likes);
        m.selected = 25;
        m.ensure_visible(20);
        assert_eq!(m.scroll, 6);
        m.selected = 3;
        m.ensure_visible(20);
        assert_eq!(m.scroll, 3);
    }

    #[test]
    fn stale_results_are_ignored() {
        let mut m = Model::new();
        m.search_gen = 2;
        m.results = vec![track("new")];
        m.apply(Msg::SearchDone {
            serial: 1,
            result: Ok(vec![track("old")]),
        });
        assert_eq!(m.results[0].title, "new");
        assert!(!m.searching);
    }

    #[test]
    fn fresh_results_replace() {
        let mut m = Model::new();
        m.search_gen = 2;
        m.apply(Msg::SearchDone {
            serial: 2,
            result: Ok(vec![track("new")]),
        });
        assert_eq!(m.results.len(), 1);
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn search_error_is_a_status_not_a_crash() {
        let mut m = Model::new();
        m.search_gen = 1;
        m.apply(Msg::SearchDone {
            serial: 1,
            result: Err("404".into()),
        });
        assert!(m.results.is_empty());
        assert_eq!(m.status.as_deref(), Some("NO CONNECTION"));
    }

    #[test]
    fn debounce_needs_400ms_and_two_chars() {
        assert!(!should_search(399, 5));
        assert!(should_search(400, 2));
        assert!(!should_search(400, 1));
    }

    #[test]
    fn next_track_stops_at_end() {
        let mut m = Model::new();
        m.results = vec![track("a"), track("b")];
        m.selected = 0;
        assert_eq!(m.next_track().unwrap().title, "b");
        assert!(m.next_track().is_none());
    }

#[test]
    fn liking_moves_track_to_likes_view() {
        let mut m = Model::new();
        m.results = vec![track("a")];
        m.selected = 0;
        assert_eq!(m.toggle_like(), Some(true));
        assert_eq!(m.liked.len(), 1);
        m.set_view(View::Likes);
        assert_eq!(m.visible().len(), 1);
        assert_eq!(m.toggle_like(), Some(false));
        assert!(m.visible().is_empty());
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn next_track_follows_the_view() {
        let mut m = Model::new();
        m.liked = vec![track("x"), track("y")];
        m.results = vec![track("a")];
        m.set_view(View::Likes);
        assert_eq!(m.next_track().unwrap().title, "y");
    }

}
