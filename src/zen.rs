use crate::sc::Track;
use std::collections::{BTreeMap, HashSet, VecDeque};

pub const QUEUE_TARGET: usize = 20;
pub const QUEUE_LOW: usize = 5;
pub const ARTIST_WINDOW: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Genre {
    HipHop,
    Electronic,
    Hardstyle,
    Ambient,
    Rock,
    Pop,
    Other,
}

impl Genre {
    pub const ALL: [Genre; 7] = [
        Genre::HipHop,
        Genre::Electronic,
        Genre::Hardstyle,
        Genre::Ambient,
        Genre::Rock,
        Genre::Pop,
        Genre::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Genre::HipHop => "HIP-HOP",
            Genre::Electronic => "ELECTRONIC",
            Genre::Hardstyle => "HARDSTYLE",
            Genre::Ambient => "AMBIENT",
            Genre::Rock => "ROCK",
            Genre::Pop => "POP",
            Genre::Other => "OTHER",
        }
    }

    pub fn query(self) -> &'static str {
        match self {
            Genre::HipHop => "hip hop",
            Genre::Electronic => "electronic",
            Genre::Hardstyle => "hardstyle",
            Genre::Ambient => "ambient",
            Genre::Rock => "rock",
            Genre::Pop => "pop",
            Genre::Other => "",
        }
    }

    /// Maps SoundCloud's free-form genre and tags into one of the buckets.
    pub fn from_tags(genre: &str, tags: &str) -> Genre {
        let text = format!("{genre} {tags}").to_lowercase();
        let has = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));

        if has(&[
            "hardstyle",
            "hardtekk",
            "hardtek",
            "hardcore",
            "rawstyle",
            "uptempo",
            "frenchcore",
            "gabber",
        ]) {
            Genre::Hardstyle
        } else if has(&[
            "hip-hop",
            "hip hop",
            "hiphop",
            "rap",
            "trap",
            "drill",
            "phonk",
            "grime",
            "boom bap",
            "mylancore",
            "cloud rap",
            "sexyswag",
            "bftd",
            "widepeek",
        ]) {
            Genre::HipHop
        } else if has(&[
            "electronic",
            "edm",
            "dance",
            "house",
            "techno",
            "electro",
            "dubstep",
            "dnb",
            "drum & bass",
            "drum and bass",
            "trance",
            "idm",
            "synth",
            "garage",
            "jungle",
            "breakbeat",
            "hyperpop",
            "bass",
        ]) {
            Genre::Electronic
        } else if has(&[
            "ambient",
            "chill",
            "lofi",
            "lo-fi",
            "drone",
            "downtempo",
            "sleep",
            "study",
        ]) {
            Genre::Ambient
        } else if has(&["rock", "metal", "punk", "grunge", "indie", "shoegaze"]) {
            Genre::Rock
        } else if has(&["pop", "r&b", "rnb", "soul", "funk", "disco"]) {
            Genre::Pop
        } else {
            Genre::Other
        }
    }
}

fn artist_of(track: &Track) -> String {
    let key = if track.artist_key.trim().is_empty() {
        &track.artist
    } else {
        &track.artist_key
    };
    key.trim().to_lowercase()
}

fn build_pool(weights: &BTreeMap<Genre, i32>) -> VecDeque<Genre> {
    let max = Genre::ALL
        .iter()
        .map(|genre| weights.get(genre).copied().unwrap_or(0).clamp(0, 4))
        .max()
        .unwrap_or(0);
    let mut pool = VecDeque::new();
    for step in 0..max {
        for genre in Genre::ALL {
            if weights.get(&genre).copied().unwrap_or(0) > step {
                pool.push_back(genre);
            }
        }
    }
    pool
}

#[derive(Clone, Debug)]
pub struct Zen {
    pub queue: Vec<Track>,
    pub index: usize,
    pub active: bool,
    pub loading: bool,
    pub serial: u64,
    pub error: Option<String>,
    pub weights: BTreeMap<Genre, i32>,
    base_weights: BTreeMap<Genre, i32>,
    pool: VecDeque<Genre>,
    recent: VecDeque<String>,
    seen: HashSet<String>,
    excluded: HashSet<String>,
    liked: HashSet<String>,
}

impl Zen {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            index: 0,
            active: false,
            loading: false,
            serial: 0,
            error: None,
            weights: BTreeMap::new(),
            base_weights: BTreeMap::new(),
            pool: VecDeque::new(),
            recent: VecDeque::new(),
            seen: HashSet::new(),
            excluded: HashSet::new(),
            liked: HashSet::new(),
        }
    }

    pub fn begin(&mut self, likes: &[Track]) -> bool {
        self.queue.clear();
        self.index = 0;
        self.active = false;
        self.loading = false;
        self.serial += 1;
        self.error = None;
        self.weights.clear();
        self.base_weights.clear();
        self.pool.clear();
        self.recent.clear();
        self.seen.clear();
        self.excluded.clear();
        self.liked = likes.iter().map(|track| track.id.clone()).collect();

        for track in likes {
            let genre = Genre::from_tags(&track.genre, &track.tags);
            *self.weights.entry(genre).or_insert(0) += 1;
        }
        if self.weights.is_empty() {
            return false;
        }
        self.base_weights = self.weights.clone();
        self.pool = build_pool(&self.weights);
        self.active = true;
        true
    }

    pub fn current(&self) -> Option<&Track> {
        self.queue.get(self.index)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn needs_refill(&self) -> bool {
        self.queue.len().saturating_sub(self.index + 1) < QUEUE_LOW
    }

    pub fn next_genre(&mut self) -> Option<Genre> {
        if self.pool.is_empty() {
            self.pool = build_pool(&self.weights);
        }
        if self.pool.is_empty() {
            self.pool = build_pool(&self.base_weights);
        }
        self.pool.pop_front()
    }

    /// Adds candidates that pass the filters, best genre match first.
    pub fn enqueue(&mut self, candidates: Vec<Track>, target: Genre) {
        let mut scored: Vec<(i32, Track)> = Vec::new();
        for track in candidates {
            if !self.accepts(&track) {
                continue;
            }
            let bucket = Genre::from_tags(&track.genre, &track.tags);
            let score = if bucket == target {
                6
            } else if target == Genre::Other || bucket == Genre::Other {
                1
            } else {
                -3
            };
            scored.push((score, track));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        for (_, track) in scored {
            if self.queue.len() >= QUEUE_TARGET {
                break;
            }
            if !self.accepts(&track) {
                continue;
            }
            if self.seen.insert(track.id.clone()) {
                let artist = artist_of(&track);
                if !artist.is_empty() {
                    self.recent.push_back(artist);
                    while self.recent.len() > ARTIST_WINDOW {
                        self.recent.pop_front();
                    }
                }
                self.queue.push(track);
            }
        }
    }

    fn accepts(&self, track: &Track) -> bool {
        if track.id.is_empty()
            || track.media.is_empty()
            || track.duration < 30
            || self.seen.contains(&track.id)
            || self.excluded.contains(&track.id)
            || self.liked.contains(&track.id)
        {
            return false;
        }
        let artist = artist_of(track);
        !self.recent.contains(&artist)
    }

    /// Moves to the next queued track. Returns None when the queue needs a refill.
    pub fn next(&mut self) -> Option<Track> {
        if self.index + 1 >= self.queue.len() {
            return None;
        }
        self.index += 1;
        self.queue.get(self.index).cloned()
    }

    pub fn previous(&mut self) -> Option<Track> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        self.queue.get(self.index).cloned()
    }

    /// User pressed next: exclude the current track and lower its genre weight.
    pub fn skip(&mut self) -> Option<Track> {
        if let Some(track) = self.current().cloned() {
            self.excluded.insert(track.id.clone());
            let genre = Genre::from_tags(&track.genre, &track.tags);
            let weight = self.weights.entry(genre).or_insert(0);
            *weight = (*weight - 2).max(0);
            self.pool.clear();
        }
        self.next()
    }

    pub fn on_like(&mut self, track: &Track, liked: bool) {
        let genre = Genre::from_tags(&track.genre, &track.tags);
        let weight = self.weights.entry(genre).or_insert(0);
        if liked {
            *weight += 3;
        } else {
            *weight = (*weight - 2).max(0);
        }
        self.pool.clear();
    }

    pub fn mark_failed(&mut self) {
        if let Some(track) = self.current().cloned() {
            self.excluded.insert(track.id);
        }
    }
}

impl Default for Zen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sc::Transcoding;

    fn track(id: &str, artist: &str, genre: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("title {id}"),
            artist: artist.into(),
            artist_key: artist.into(),
            duration: 180,
            url: format!("https://soundcloud.com/{artist}/{id}"),
            genre: genre.into(),
            media: vec![Transcoding {
                url: "u".into(),
                preset: "aac_160k".into(),
                protocol: "hls".into(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn maps_dirty_tags_to_buckets() {
        let cases = [
            ("Hip-hop & Rap", "", Genre::HipHop),
            ("Trap", "", Genre::HipHop),
            ("Rap", "", Genre::HipHop),
            ("Dance & EDM", "", Genre::Electronic),
            ("Electronic", "", Genre::Electronic),
            ("hardstyle", "", Genre::Hardstyle),
            ("", "Hardtekk Tekk Hardstyle Hardcore", Genre::Hardstyle),
            ("Ambient", "", Genre::Ambient),
            ("Soundtrack", "", Genre::Other),
            ("BFTD", "", Genre::HipHop),
            ("sexyswag", "", Genre::HipHop),
            ("2016", "", Genre::Other),
            ("", "", Genre::Other),
        ];
        for (genre, tags, expected) in cases {
            assert_eq!(Genre::from_tags(genre, tags), expected, "genre={genre} tags={tags}");
        }
    }

    #[test]
    fn empty_likes_do_not_start() {
        let mut zen = Zen::new();
        assert!(!zen.begin(&[]));
        assert!(!zen.is_active());
    }

    #[test]
    fn begin_counts_weights() {
        let mut zen = Zen::new();
        let likes = vec![
            track("1", "a", "Hip-hop & Rap"),
            track("2", "b", "Trap"),
            track("3", "c", "Ambient"),
        ];
        assert!(zen.begin(&likes));
        assert_eq!(zen.weights.get(&Genre::HipHop), Some(&2));
        assert_eq!(zen.weights.get(&Genre::Ambient), Some(&1));
        assert!(zen.is_active());
    }

    #[test]
    fn enqueue_filters_liked_seen_short_and_media_less() {
        let mut zen = Zen::new();
        zen.begin(&[track("liked", "a", "Trap")]);

        let mut short = track("short", "b", "Trap");
        short.duration = 10;
        let mut no_media = track("nomedia", "c", "Trap");
        no_media.media.clear();

        zen.enqueue(
            vec![
                track("liked", "a", "Trap"),
                short,
                no_media,
                track("ok", "d", "Trap"),
            ],
            Genre::HipHop,
        );
        assert_eq!(zen.queue.len(), 1);
        assert_eq!(zen.queue[0].id, "ok");
    }

    #[test]
    fn enqueue_does_not_repeat_artists_in_the_window() {
        let mut zen = Zen::new();
        zen.begin(&[track("seed", "seed", "Trap")]);
        let candidates: Vec<Track> = (0..15)
            .map(|i| track(&format!("t{i}"), if i < 3 { "same" } else { "other" }, "Trap"))
            .collect();
        zen.enqueue(candidates, Genre::HipHop);
        assert_eq!(zen.queue.len(), 2);
        assert_eq!(zen.queue[0].artist, "same");
        assert_eq!(zen.queue[1].artist, "other");
    }

    #[test]
    fn next_and_previous_walk_the_queue() {
        let mut zen = Zen::new();
        zen.begin(&[track("seed", "seed", "Trap")]);
        zen.enqueue(
            vec![track("1", "a", "Trap"), track("2", "b", "Trap")],
            Genre::HipHop,
        );
        assert_eq!(zen.current().unwrap().id, "1");
        assert_eq!(zen.next().unwrap().id, "2");
        assert_eq!(zen.previous().unwrap().id, "1");
        assert!(zen.previous().is_none());
    }

    #[test]
    fn skip_excludes_and_lowers_weight() {
        let mut zen = Zen::new();
        zen.begin(&[
            track("seed", "seed", "Trap"),
            track("seed2", "seed2", "Ambient"),
        ]);
        zen.enqueue(vec![track("1", "a", "Trap"), track("2", "b", "Trap")], Genre::HipHop);
        let before = zen.weights.get(&Genre::HipHop).copied().unwrap();
        assert_eq!(zen.skip().unwrap().id, "2");
        assert!(zen.excluded.contains("1"));
        assert!(zen.weights.get(&Genre::HipHop).copied().unwrap() < before);
    }

    #[test]
    fn like_raises_weight_and_pool_has_all_genres() {
        let mut zen = Zen::new();
        zen.begin(&[
            track("seed", "seed", "Trap"),
            track("seed2", "seed2", "Ambient"),
        ]);
        zen.on_like(&track("x", "x", "Ambient"), true);
        assert_eq!(zen.weights.get(&Genre::Ambient), Some(&4));
        let mut seen = HashSet::new();
        for _ in 0..20 {
            if let Some(genre) = zen.next_genre() {
                seen.insert(genre);
            }
        }
        assert!(seen.contains(&Genre::HipHop));
        assert!(seen.contains(&Genre::Ambient));
    }
}
