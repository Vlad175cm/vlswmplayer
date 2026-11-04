use crate::input::Action;
use crate::model::{self, Model, Msg};
use crate::sc::{self, Track};
use crate::theme;
use crate::{artwork, cache, config, input, likes, mpv, ui};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

pub struct App {
    pub model: Model,
    pub theme: theme::Watcher,
    pub player: Option<mpv::Player>,
    pub artwork: Option<egui::TextureHandle>,
    pub cfg: config::Config,
    pub ytdlp: Option<PathBuf>,
    pub hostname: String,
    pub search_focus: bool,
    pub last_edit: Option<Instant>,
    pub hits: ui::Hits,
    cache: cache::Cache,
    downloading: HashSet<String>,
    download_serial: u64,
    drag: Option<Drag>,
    swallow_slash: bool,
    zoom_done: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Progress,
    Volume,
}

const FONT: &[u8] = include_bytes!("../assets/JetBrainsMonoNerdFont-Regular.ttf");

fn install_fonts(cc: &eframe::CreationContext<'_>) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "jetbrains".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["jetbrains".to_owned()]);
    }
    cc.egui_ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::BLACK;
    visuals.window_fill = egui::Color32::BLACK;
    visuals.override_text_color = Some(egui::Color32::WHITE);
    cc.egui_ctx.set_visuals(visuals);
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(cc);
        let cfg = config::Config::load(&config::default_path());
        let mut model = Model::new();
        model.query = cfg.last_query.clone();

        let player = mpv::Player::new(cfg.volume).ok();
        model.liked = likes::load(&likes::default_path());
        let ytdlp = sc::resolve_ytdlp(None);
        if ytdlp.is_none() {
            model.status = Some("YT-DLP NOT FOUND".to_owned());
        }

        let (tx, rx) = channel();
        let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();

        let mut app = Self {
            model,
            theme: theme::Watcher::new(),
            player,
            artwork: None,
            cfg,
            ytdlp,
            hostname,
            search_focus: false,
            last_edit: None,
            hits: ui::Hits::default(),
            cache: cache::Cache::new(),
            downloading: HashSet::new(),
            download_serial: 0,
            drag: None,
            swallow_slash: false,
            zoom_done: false,
            tx,
            rx,
        };
        if app.model.query.chars().count() >= model::MIN_QUERY_LEN {
            app.start_search(&cc.egui_ctx);
        }
        app
    }

    pub fn start_search(&mut self, ctx: &egui::Context) {
        let Some(ytdlp) = self.ytdlp.clone() else {
            self.model.status = Some("YT-DLP NOT FOUND".to_owned());
            return;
        };
        self.model.search_gen += 1;
        let serial = self.model.search_gen;
        let query = self.model.query.clone();
        self.model.searching = true;
        self.model.status = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = sc::search_fast(&query, &ytdlp);
            let _ = tx.send(Msg::SearchDone {
                serial,
                result,
            });
            ctx.request_repaint();
        });
    }

    pub fn play_track(&mut self, track: Track, ctx: &egui::Context) {
        let Some(ytdlp) = self.ytdlp.clone() else {
            self.model.status = Some("YT-DLP NOT FOUND".to_owned());
            return;
        };
        if self.player.is_none() {
            match mpv::Player::new(self.cfg.volume) {
                Ok(p) => self.player = Some(p),
                Err(e) => {
                    self.model.status = Some(format!("NO AUDIO: {e}"));
                    return;
                }
            }
        }

        self.download_serial += 1;
        let serial = self.download_serial;
        self.model.download_gen = serial;
        self.model.current = Some(track.clone());
        self.model.status = Some("LOADING".to_owned());
        self.artwork = None;

        match self.cache.get(&track.id) {
            Some(path) => {
                self.model.status = None;
                if let Some(player) = self.player.as_mut() {
                    player.load(&path.to_string_lossy());
                }
            }
            None => {
                self.start_cache_download(&track, &ytdlp, ctx);
                let tx = self.tx.clone();
                let ctx_done = ctx.clone();
                let stream_track = track.clone();
                std::thread::spawn(move || {
                    let result = sc::stream_url(&stream_track, &ytdlp)
                        .map(|url| (stream_track.clone(), url));
                    let _ = tx.send(Msg::StreamReady { serial, result });
                    ctx_done.request_repaint();
                });
            }
        }

        if let Some(url) = track.artwork.clone() {
            let tx = self.tx.clone();
            let ctx = ctx.clone();
            let id = track.id.clone();
            std::thread::spawn(move || {
                let result = artwork::fetch(&url).and_then(|bytes| artwork::decode(&bytes));
                let _ = tx.send(Msg::ArtworkDone { id, result });
                ctx.request_repaint();
            });
        }
    }

    fn start_cache_download(&mut self, track: &Track, ytdlp: &PathBuf, ctx: &egui::Context) {
        if !self.downloading.insert(track.id.clone()) {
            return;
        }
        let target = self.cache.path_for(&track.id);
        let url = track.url.clone();
        let id = track.id.clone();
        let ytdlp = ytdlp.clone();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = std::process::Command::new(&ytdlp)
                .args(sc::cache_args(&url, &target))
                .output()
                .map_err(|e| e.to_string())
                .and_then(|out| {
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
                    }
                });
            let _ = tx.send(Msg::CacheDone { id, result });
            ctx.request_repaint();
        });
    }

    pub fn next_and_play(&mut self, ctx: &egui::Context) {
        if let Some(track) = self.model.next_track() {
            self.play_track(track, ctx);
        }
    }

    pub fn prev_and_play(&mut self, ctx: &egui::Context) {
        if let Some(track) = self.model.prev_track() {
            self.play_track(track, ctx);
        }
    }

    pub fn save_config(&mut self) {
        self.cfg.last_query = self.model.query.clone();
        if let Some(player) = self.player.as_ref() {
            self.cfg.volume = player.volume();
        }
        let _ = self.cfg.save(&config::default_path());
    }


    fn ensure_visible(&mut self) {
        let capacity = self.hits.capacity.max(1);
        self.model.ensure_visible(capacity);
    }

    fn play_selected(&mut self, ctx: &egui::Context) {
        if let Some(track) = self.model.visible().get(self.model.selected).cloned() {
            self.play_track(track, ctx);
        }
    }

    fn change_volume(&mut self, delta: i32) {
        let current = i32::from(self.cfg.volume) + delta;
        let volume = current.clamp(0, 100) as u8;
        self.cfg.volume = volume;
        if let Some(player) = self.player.as_mut() {
            player.set_volume(volume);
        }
        let _ = self.cfg.save(&config::default_path());
    }

    fn seek_to(&mut self, x: f32, rect: egui::Rect) {
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let duration = player.duration();
        if duration == 0 {
            return;
        }
        let frac = crate::util::fraction_at(x, rect.left(), rect.width());
        let target = (frac * duration as f32) as u64;
        if let Some(player) = self.player.as_mut() {
            player.seek(target);
        }
    }

    fn volume_to(&mut self, x: f32, rect: egui::Rect) {
        let frac = crate::util::fraction_at(x, rect.left(), rect.width());
        let volume = (frac * 100.0).round() as u8;
        self.cfg.volume = volume;
        if let Some(player) = self.player.as_mut() {
            player.set_volume(volume);
        }
    }

    fn run_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::PlayPause => match self.player.as_mut() {
                Some(player) => player.toggle(),
                None => self.play_selected(ctx),
            },
            Action::Prev => {
                self.prev_and_play(ctx);
                self.ensure_visible();
            }
            Action::Next => {
                self.next_and_play(ctx);
                self.ensure_visible();
            }
            Action::SeekBack => {
                if let Some(player) = self.player.as_mut() {
                    let target = player.position().saturating_sub(5);
                    player.seek(target);
                }
            }
            Action::SeekForward => {
                if let Some(player) = self.player.as_mut() {
                    let target = player.position() + 5;
                    player.seek(target);
                }
            }
            Action::VolumeUp => self.change_volume(5),
            Action::VolumeDown => self.change_volume(-5),
            Action::FocusSearch => {
                self.search_focus = true;
                self.swallow_slash = true;
            }
            Action::Confirm => {
                self.search_focus = false;
                self.play_selected(ctx);
            }
            Action::ToggleView => {
                self.model.toggle_view();
                self.ensure_visible();
            }
            Action::ScrollUp => {
                let capacity = self.hits.capacity.max(1);
                self.model.scroll_by(-(capacity as i64), capacity);
            }
            Action::ScrollDown => {
                let capacity = self.hits.capacity.max(1);
                self.model.scroll_by(capacity as i64, capacity);
            }
            Action::Like => {
                self.model.toggle_like();
                self.ensure_visible();
                let _ = likes::save(&likes::default_path(), &self.model.liked);
            }
            Action::Quit => {
                self.save_config();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) if self.search_focus => {
                    let swallow = self.swallow_slash && text == "/";
                    self.swallow_slash = false;
                    if !swallow {
                        for ch in text.chars().filter(|c| !c.is_control()) {
                            self.model.query.push(ch);
                        }
                        self.last_edit = Some(Instant::now());
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if self.search_focus {
                        match key {
                            egui::Key::Escape => self.search_focus = false,
                            egui::Key::Backspace => {
                                self.model.query.pop();
                                self.last_edit = Some(Instant::now());
                            }
                            egui::Key::Enter => {
                                self.search_focus = false;
                                self.play_selected(ctx);
                            }
                            _ => {}
                        }
                    } else if modifiers.ctrl && modifiers.shift && key == egui::Key::C {
                        match self.cache.clear() {
                            Ok(_) => self.model.status = Some("CACHE CLEARED".to_owned()),
                            Err(e) => self.model.status = Some(format!("CACHE: {e}")),
                        }
                    } else if let Some(action) = input::action_for(key, modifiers.shift) {
                        self.run_action(action, ctx);
                    }
                }
                _ => {}
            }
        }

        let pressed = ctx.input(|i| i.pointer.primary_pressed());
        let released = ctx.input(|i| i.pointer.primary_released());
        let down = ctx.input(|i| i.pointer.primary_down());
        let pos = ctx.input(|i| i.pointer.interact_pos());

        if pressed {
            if let Some(pos) = pos {
                if let Some((search_tab, likes_tab)) = self.hits.tabs {
                    if search_tab.contains(pos) {
                        self.model.set_view(crate::model::View::Search);
                        return;
                    }
                    if likes_tab.contains(pos) {
                        self.model.set_view(crate::model::View::Likes);
                        return;
                    }
                }
                if let Some(rect) = self.hits.search {
                    if rect.contains(pos) {
                        self.search_focus = true;
                        return;
                    }
                }
                self.search_focus = false;

                if let Some(rect) = self.hits.play {
                    if rect.contains(pos) {
                        self.run_action(Action::PlayPause, ctx);
                        return;
                    }
                }
                if let Some(rect) = self.hits.prev {
                    if rect.contains(pos) {
                        self.run_action(Action::Prev, ctx);
                        return;
                    }
                }
                if let Some(rect) = self.hits.next {
                    if rect.contains(pos) {
                        self.run_action(Action::Next, ctx);
                        return;
                    }
                }
                if let Some(rect) = self.hits.progress {
                    if rect.contains(pos) {
                        self.drag = Some(Drag::Progress);
                        self.seek_to(pos.x, rect);
                        return;
                    }
                }
                if let Some(rect) = self.hits.volume {
                    if rect.contains(pos) {
                        self.drag = Some(Drag::Volume);
                        self.volume_to(pos.x, rect);
                        return;
                    }
                }
                for (index, rect) in self.hits.rows.clone() {
                    if rect.contains(pos) {
                        self.model.select(index);
                        self.ensure_visible();
                        self.play_selected(ctx);
                        return;
                    }
                }
            }
        }

        let wheel = ctx.input(|i| i.smooth_scroll_delta.y);
        if wheel != 0.0 {
            if let (Some(list), Some(pos)) = (self.hits.list, pos) {
                if list.contains(pos) {
                    let capacity = self.hits.capacity.max(1);
                    let step = if wheel > 0.0 { -3 } else { 3 };
                    self.model.scroll_by(step, capacity);
                }
            }
        }

        if down {
            if let (Some(drag), Some(pos)) = (self.drag, pos) {
                match drag {
                    Drag::Volume => {
                        if let Some(rect) = self.hits.volume {
                            self.volume_to(pos.x, rect);
                        }
                    }
                    Drag::Progress => {}
                }
            }
        }

        if released {
            if self.drag == Some(Drag::Progress) {
                if let (Some(rect), Some(pos)) = (self.hits.progress, pos) {
                    self.seek_to(pos.x, rect);
                }
            }
            self.drag = None;
        }
    }

    fn pump(&mut self, ctx: &egui::Context) {
        let messages: Vec<Msg> = self.rx.try_iter().collect();
        for message in messages {
            match message {
                Msg::StreamReady { serial, result } => {
                    if serial != self.model.download_gen {
                        continue;
                    }
                    match result {
                        Ok((track, url)) => {
                            self.model.current = Some(track);
                            self.model.status = None;
                            if let Some(player) = self.player.as_mut() {
                                player.load(&url);
                            }
                        }
                        Err(_) => {
                            self.model.status = Some("TRACK UNAVAILABLE".to_owned());
                            if let Some(next) = self.model.next_track() {
                                self.play_track(next, ctx);
                                self.model.status =
                                    Some("TRACK UNAVAILABLE — SKIPPING".to_owned());
                            }
                        }
                    }
                }
                Msg::CacheDone { id, result } => {
                    self.downloading.remove(&id);
                    if result.is_err() {
                        let _ = self.cache.path_for(&id);
                    }
                }
                other => {
                    let is_artwork = matches!(other, Msg::ArtworkDone { .. });
                    self.model.apply(other);
                    if is_artwork {
                        if let Some(image) = self.model.pending_artwork.take() {
                            self.artwork = Some(ctx.load_texture(
                                "artwork",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.zoom_done {
            ctx.set_zoom_factor(self.cfg.scale);
            self.zoom_done = true;
        }
        self.theme.poll();
        self.handle_input(ctx);
        self.pump(ctx);

        if let Some(player) = self.player.as_mut() {
            if player.take_failed() {
                if let Some(track) = self.model.current.clone() {
                    self.cache.remove(&track.id);
                }
                self.model.status = Some("TRACK UNAVAILABLE".to_owned());
                self.next_and_play(ctx);
            } else if player.finished() {
                self.next_and_play(ctx);
            }
        }

        if self.search_focus && !self.model.searching {
            if let Some(edited) = self.last_edit {
                let elapsed = edited.elapsed().as_millis() as u64;
                if model::should_search(elapsed, self.model.query.chars().count()) {
                    self.last_edit = None;
                    self.start_search(ctx);
                }
            }
        }

        ctx.request_repaint_after(Duration::from_millis(200));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::draw(self, ui);
    }

    fn on_exit(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.quit();
        }
        self.save_config();
    }
}
