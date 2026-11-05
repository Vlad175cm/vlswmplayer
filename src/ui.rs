use crate::app::{App, DialogKind};
use crate::likes;
use crate::model::View;
use crate::util::format_time;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};

pub const HEADER_H: f32 = 32.0;
pub const FOOTER_H: f32 = 28.0;
pub const LEFT_W: f32 = 300.0;
pub const PAD: f32 = 18.0;
pub const ART: f32 = 210.0;
pub const ROW_H: f32 = 28.0;

#[derive(Clone, Debug, Default)]
pub struct Hits {
    pub search: Option<Rect>,
    pub progress: Option<Rect>,
    pub volume: Option<Rect>,
    pub prev: Option<Rect>,
    pub play: Option<Rect>,
    pub next: Option<Rect>,
    pub rows: Vec<(usize, Rect)>,
    pub tabs: Vec<(View, Rect)>,
    pub menu_button: Option<Rect>,
    pub menu_panel: Option<Rect>,
    pub menu_items: Vec<Rect>,
    pub dialog_confirm: Option<Rect>,
    pub dialog_cancel: Option<Rect>,
    pub list: Option<Rect>,
    pub capacity: usize,
}

struct Palette {
    bg: Color32,
    fg: Color32,
}

fn text(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    s: &str,
    size: f32,
    color: Color32,
) -> f32 {
    let galley = p.layout_no_wrap(s.to_owned(), FontId::monospace(size), color);
    let w = galley.size().x;
    p.galley(pos, galley, color);
    let _ = anchor;
    w
}

fn text_at(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    s: &str,
    size: f32,
    color: Color32,
) {
    p.text(pos, anchor, s, FontId::monospace(size), color);
}

fn elide(p: &Painter, s: &str, size: f32, width: f32) -> String {
    let probe = p.layout_no_wrap("M".to_owned(), FontId::monospace(size), Color32::WHITE);
    let char_w = probe.size().x.max(1.0);
    let max = (width / char_w).floor().max(1.0) as usize;
    let len = s.chars().count();
    if len <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn section(p: &Painter, x: f32, y: f32, w: f32, name: &str, fg: Color32) {
    let title = format!("— {name} ");
    let tw = text(p, Pos2::new(x, y), Align2::LEFT_TOP, &title, 11.0, fg);
    let ly = y + 9.0;
    p.line_segment(
        [Pos2::new(x + tw + 6.0, ly), Pos2::new(x + w, ly)],
        Stroke::new(1.0, fg),
    );
}

fn bar(p: &Painter, rect: Rect, frac: f32, pal: &Palette) {
    p.rect_filled(rect, CornerRadius::ZERO, pal.bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, pal.fg),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(1.0);
    if inner.width() > 0.0 {
        let w = inner.width() * frac.clamp(0.0, 1.0);
        p.rect_filled(
            Rect::from_min_size(inner.min, Vec2::new(w, inner.height())),
            CornerRadius::ZERO,
            pal.fg,
        );
    }
}

fn button(p: &Painter, rect: Rect, label: &str, filled: bool, pal: &Palette) {
    if filled {
        p.rect_filled(rect, CornerRadius::ZERO, pal.fg);
        text_at(p, rect.center(), Align2::CENTER_CENTER, label, 13.0, pal.bg);
    } else {
        p.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, pal.fg),
            StrokeKind::Inside,
        );
        text_at(p, rect.center(), Align2::CENTER_CENTER, label, 13.0, pal.fg);
    }
}

fn tab(p: &Painter, rect: Rect, label: &str, active: bool, pal: &Palette) {
    if active {
        p.rect_filled(rect, CornerRadius::ZERO, pal.fg);
        text_at(p, rect.center(), Align2::CENTER_CENTER, label, 11.0, pal.bg);
    } else {
        p.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, pal.fg),
            StrokeKind::Inside,
        );
        text_at(p, rect.center(), Align2::CENTER_CENTER, label, 11.0, pal.fg);
    }
}

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let full = ui.max_rect();
    let theme = app.theme.current();
    let pal = Palette {
        bg: theme.bg(),
        fg: theme.fg(),
    };
    let time = ui.input(|i| i.time);
    let p = ui.painter();
    p.rect_filled(full, CornerRadius::ZERO, pal.bg);

    let mut hits = Hits::default();

    let header = Rect::from_min_size(full.min, Vec2::new(full.width(), HEADER_H));
    let footer_top = full.bottom() - FOOTER_H;
    let footer = Rect::from_min_max(Pos2::new(full.left(), footer_top), full.max);
    let body = Rect::from_min_max(
        Pos2::new(full.left(), full.top() + HEADER_H),
        Pos2::new(full.right(), footer_top),
    );

    p.line_segment(
        [header.left_bottom(), header.right_bottom()],
        Stroke::new(1.0, pal.fg),
    );
    p.line_segment(
        [footer.left_top(), footer.right_top()],
        Stroke::new(1.0, pal.fg),
    );

    text_at(
        &p,
        Pos2::new(header.left() + PAD, header.center().y),
        Align2::LEFT_CENTER,
        "VLSWM // PLAYER",
        12.0,
        pal.fg,
    );
    let menu_rect = Rect::from_min_size(
        Pos2::new(header.right() - PAD - 24.0, header.center().y - 11.0),
        Vec2::new(24.0, 22.0),
    );
    for i in 0..3 {
        let y = menu_rect.center().y - 6.0 + i as f32 * 6.0;
        p.line_segment(
            [
                Pos2::new(menu_rect.left() + 4.0, y),
                Pos2::new(menu_rect.right() - 4.0, y),
            ],
            Stroke::new(2.0, pal.fg),
        );
    }
    hits.menu_button = Some(menu_rect);
    text_at(
        &p,
        Pos2::new(menu_rect.left() - 14.0, header.center().y),
        Align2::RIGHT_CENTER,
        &app.hostname,
        12.0,
        pal.fg,
    );

    let split = body.left() + LEFT_W;
    p.line_segment(
        [
            Pos2::new(split, body.top()),
            Pos2::new(split, body.bottom()),
        ],
        Stroke::new(1.0, pal.fg),
    );

    draw_now_playing(app, &p, &pal, body, split, &mut hits);
    draw_search(app, &p, &pal, body, split, &mut hits, time);

    let hints = "SPACE play  ←/→ track  ↑/↓ vol  TAB tab  F like  M menu  ^⇧C cache  / search  Q quit";
    let left = Pos2::new(footer.left() + PAD, footer.center().y);
    let status = match (&app.model.status, &app.model.searching) {
        (Some(s), _) => Some(s.clone()),
        (None, true) => Some("SEARCHING".to_owned()),
        _ => None,
    };
    text_at(&p, left, Align2::LEFT_CENTER, hints, 11.0, pal.fg);
    if let Some(s) = status {
        let blink = (time * 2.0) as u64 % 2 == 0;
        let shown = if blink { s } else { format!("{s} ") };
        text_at(
            &p,
            Pos2::new(footer.right() - PAD, footer.center().y),
            Align2::RIGHT_CENTER,
            &shown,
            11.0,
            pal.fg,
        );
    }

    let progress = app.upload_progress();
    if progress.active {
        let label = format!(
            "UPLOADING {}/{}  {}",
            progress.done.min(progress.total),
            progress.total,
            progress.current
        );
        text_at(
            &p,
            Pos2::new(full.left() + PAD, footer.top() - 34.0),
            Align2::LEFT_BOTTOM,
            &label,
            10.0,
            pal.fg,
        );
        let rect = Rect::from_min_size(
            Pos2::new(full.left() + PAD, footer.top() - 24.0),
            Vec2::new(full.width() - PAD * 2.0, 10.0),
        );
        bar(&p, rect, progress.overall(), &pal);
    }

    if app.menu_open {
        let width = 210.0;
        let panel = Rect::from_min_size(
            Pos2::new(full.right() - PAD - width, header.bottom() + 4.0),
            Vec2::new(width, 80.0),
        );
        p.rect_filled(panel, CornerRadius::ZERO, pal.bg);
        p.rect_stroke(
            panel,
            CornerRadius::ZERO,
            Stroke::new(1.0, pal.fg),
            StrokeKind::Inside,
        );
        for (index, label) in ["ADD LOCAL FILES", "EXPORT LIKES", "IMPORT LIKES"].iter().enumerate() {
            let item = Rect::from_min_size(
                Pos2::new(panel.left() + 1.0, panel.top() + 1.0 + index as f32 * 26.0),
                Vec2::new(width - 2.0, 26.0),
            );
            text_at(
                &p,
                Pos2::new(item.left() + 10.0, item.center().y),
                Align2::LEFT_CENTER,
                label,
                11.0,
                pal.fg,
            );
            hits.menu_items.push(item);
        }
        hits.menu_panel = Some(panel);
    }

    if let Some(kind) = app.dialog {
        let width = 560.0;
        let height = 116.0;
        let panel = Rect::from_center_size(full.center(), Vec2::new(width, height));
        p.rect_filled(panel, CornerRadius::ZERO, pal.bg);
        p.rect_stroke(
            panel,
            CornerRadius::ZERO,
            Stroke::new(1.0, pal.fg),
            StrokeKind::Inside,
        );

        let title = match kind {
            DialogKind::AddFiles => "ADD LOCAL FILES",
            DialogKind::ExportLikes => "EXPORT LIKES",
            DialogKind::ImportLikes => "IMPORT LIKES",
        };
        text_at(
            &p,
            Pos2::new(panel.left() + 14.0, panel.top() + 12.0),
            Align2::LEFT_TOP,
            title,
            11.0,
            pal.fg,
        );

        let input = Rect::from_min_size(
            Pos2::new(panel.left() + 14.0, panel.top() + 34.0),
            Vec2::new(width - 28.0, 30.0),
        );
        p.rect_stroke(
            input,
            CornerRadius::ZERO,
            Stroke::new(1.0, pal.fg),
            StrokeKind::Inside,
        );
        let shown = elide(&p, &app.dialog_input, 12.0, input.width() - 20.0);
        text_at(
            &p,
            Pos2::new(input.left() + 8.0, input.center().y),
            Align2::LEFT_CENTER,
            &shown,
            12.0,
            pal.fg,
        );

        text_at(
            &p,
            Pos2::new(panel.left() + 14.0, panel.bottom() - 20.0),
            Align2::LEFT_BOTTOM,
            "ENTER CONFIRM   ESC CANCEL",
            10.0,
            pal.fg,
        );

        let ok = Rect::from_min_size(
            Pos2::new(panel.right() - 154.0, panel.bottom() - 36.0),
            Vec2::new(62.0, 26.0),
        );
        let cancel = Rect::from_min_size(
            Pos2::new(panel.right() - 84.0, panel.bottom() - 36.0),
            Vec2::new(70.0, 26.0),
        );
        button(&p, ok, "OK", true, &pal);
        button(&p, cancel, "CANCEL", false, &pal);
        hits.dialog_confirm = Some(ok);
        hits.dialog_cancel = Some(cancel);
    }

    app.hits = hits;
}

fn draw_now_playing(
    app: &App,
    p: &Painter,
    pal: &Palette,
    body: Rect,
    split: f32,
    hits: &mut Hits,
) {
    let x = body.left() + PAD;
    let w = split - body.left() - PAD * 2.0;
    let mut y = body.top() + 14.0;

    section(p, x, y, w, "NOW PLAYING", pal.fg);
    y += 26.0;

    let art = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(ART));
    p.rect_stroke(
        art,
        CornerRadius::ZERO,
        Stroke::new(1.0, pal.fg),
        StrokeKind::Inside,
    );
    if let Some(texture) = &app.artwork {
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        p.image(texture.id(), art.shrink(1.0), uv, Color32::WHITE);
    } else {
        text_at(
            p,
            art.center(),
            Align2::CENTER_CENTER,
            "NO ART",
            11.0,
            pal.fg,
        );
    }
    y += ART + 14.0;

    let (title, artist) = match &app.model.current {
        Some(track) => (track.title.clone(), track.artist.to_uppercase()),
        None => ("NOTHING PLAYING".to_owned(), String::new()),
    };

    text_at(
        p,
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        &elide(p, &title, 21.0, w),
        21.0,
        pal.fg,
    );
    y += 28.0;
    text_at(
        p,
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        &elide(p, &artist, 12.0, w),
        12.0,
        pal.fg,
    );
    y += 24.0;

    let (position, duration) = match app.player.as_ref() {
        Some(player) => (player.position(), player.duration()),
        None => (0, 0),
    };
    let frac = if duration > 0 {
        position as f32 / duration as f32
    } else {
        0.0
    };

    let progress = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 12.0));
    bar(p, progress, frac, pal);
    hits.progress = Some(progress);
    y += 16.0;

    text_at(
        p,
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        &format_time(position),
        11.0,
        pal.fg,
    );
    text_at(
        p,
        Pos2::new(x + w, y),
        Align2::RIGHT_TOP,
        &format_time(duration),
        11.0,
        pal.fg,
    );
    y += 20.0;

    let volume = match app.player.as_ref() {
        Some(player) => player.volume(),
        None => app.cfg.volume,
    };
    let label = "VOL ";
    let lw = text(p, Pos2::new(x, y + 2.0), Align2::LEFT_TOP, label, 11.0, pal.fg);
    let vol_rect = Rect::from_min_size(Pos2::new(x + lw + 6.0, y), Vec2::new(120.0, 12.0));
    bar(p, vol_rect, f32::from(volume) / 100.0, pal);
    hits.volume = Some(vol_rect);
    text_at(
        p,
        Pos2::new(x + lw + 6.0 + 126.0, y + 2.0),
        Align2::LEFT_TOP,
        &volume.to_string(),
        11.0,
        pal.fg,
    );
    y += 28.0;

    let playing = app
        .player
        .as_ref()
        .map(|pl| !pl.state().paused && !pl.state().idle)
        .unwrap_or(false);
    let prev = Rect::from_min_size(Pos2::new(x, y), Vec2::new(54.0, 34.0));
    let play = Rect::from_min_size(Pos2::new(x + 62.0, y), Vec2::new(64.0, 34.0));
    let next = Rect::from_min_size(Pos2::new(x + 134.0, y), Vec2::new(54.0, 34.0));
    button(p, prev, "|<", false, pal);
    button(p, play, if playing { "||" } else { ">" }, true, pal);
    button(p, next, ">|", false, pal);
    hits.prev = Some(prev);
    hits.play = Some(play);
    hits.next = Some(next);
}

fn draw_search(
    app: &mut App,
    p: &Painter,
    pal: &Palette,
    body: Rect,
    split: f32,
    hits: &mut Hits,
    time: f64,
) {
    let x = split + 1.0 + PAD;
    let w = body.right() - PAD - x;
    let mut y = body.top() + 14.0;

    let tab_w = 92.0;
    for (index, (view, title)) in [
        (View::Search, "SEARCH"),
        (View::Likes, "LIKES"),
        (View::Uploaded, "UPLOADED"),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(
            Pos2::new(x + index as f32 * tab_w, y),
            Vec2::new(tab_w, 24.0),
        );
        tab(p, rect, title, app.model.view == view, pal);
        hits.tabs.push((view, rect));
    }
    y += 24.0 + 14.0;

    let search = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 32.0));
    let stroke = if app.search_focus { 2.0 } else { 1.0 };
    p.rect_stroke(
        search,
        CornerRadius::ZERO,
        Stroke::new(stroke, pal.fg),
        StrokeKind::Inside,
    );
    let prompt = format!("> {}", app.model.query);
    text_at(
        p,
        Pos2::new(search.left() + 10.0, search.center().y),
        Align2::LEFT_CENTER,
        &elide(p, &prompt, 14.0, w - 28.0),
        14.0,
        pal.fg,
    );
    if app.search_focus && (time * 2.0) as u64 % 2 == 0 {
        let tw = p
            .layout_no_wrap(prompt.clone(), FontId::monospace(14.0), pal.fg)
            .size()
            .x;
        let cx = (search.left() + 10.0 + tw + 2.0).min(search.right() - 14.0);
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(cx, search.center().y - 8.0),
                Vec2::new(8.0, 16.0),
            ),
            CornerRadius::ZERO,
            pal.fg,
        );
    }
    hits.search = Some(search);
    y += 32.0 + 16.0;

    section(p, x, y, w, "RESULTS", pal.fg);
    y += 24.0;

    let columns = [30.0, (w - 30.0 - 130.0 - 50.0 - 24.0).min(520.0), 130.0, 50.0];
    let header = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, ROW_H));
    p.line_segment(
        [header.left_bottom(), header.right_bottom()],
        Stroke::new(1.0, pal.fg),
    );
    let mut cx = x + 4.0;
    for (label, width) in [("#", columns[0]), ("TITLE", columns[1]), ("ARTIST", columns[2]), ("LEN", columns[3])] {
        let anchor = if label == "#" || label == "LEN" {
            Align2::RIGHT_TOP
        } else {
            Align2::LEFT_TOP
        };
        let tx = if anchor == Align2::RIGHT_TOP { cx + width - 8.0 } else { cx };
        text_at(
            p,
            Pos2::new(tx, y + 7.0),
            anchor,
            label,
            10.0,
            pal.fg,
        );
        cx += width + 8.0;
    }
    y += ROW_H;

    if app.model.searching {
        text_at(
            p,
            Pos2::new(x + 4.0, y + 7.0),
            Align2::LEFT_TOP,
            "SEARCHING",
            13.0,
            pal.fg,
        );
        return;
    }
    if app.model.visible().is_empty() {
        let msg = match app.model.view {
            View::Likes => "NO LIKES YET — PRESS F",
            _ if app.model.query.chars().count() < 2 => "TYPE TO SEARCH",
            _ => "— NOTHING FOUND —",
        };
        text_at(
            p,
            Pos2::new(x + 4.0, y + 7.0),
            Align2::LEFT_TOP,
            msg,
            13.0,
            pal.fg,
        );
        return;
    }

    let bottom = body.bottom() - 8.0;
    let capacity = ((bottom - y) / ROW_H).floor().max(1.0) as usize;
    hits.capacity = capacity;
    hits.list = Some(Rect::from_min_max(
        Pos2::new(x, y),
        Pos2::new(x + w, bottom),
    ));
    let visible_len = app.model.visible().len();
    let start = app.model.scroll.min(visible_len.saturating_sub(capacity));

    if visible_len > capacity {
        let track = Rect::from_min_max(Pos2::new(x + w - 2.0, y), Pos2::new(x + w, bottom));
        p.rect_filled(track, CornerRadius::ZERO, pal.bg);
        p.rect_stroke(
            track,
            CornerRadius::ZERO,
            Stroke::new(1.0, pal.fg),
            StrokeKind::Inside,
        );
        let thumb_h = (track.height() * capacity as f32 / visible_len as f32).max(12.0);
        let pos = start as f32 / (visible_len - capacity) as f32;
        let top = track.top() + (track.height() - thumb_h) * pos;
        p.rect_filled(
            Rect::from_min_size(Pos2::new(track.left() + 1.0, top), Vec2::new(track.width() - 2.0, thumb_h)),
            CornerRadius::ZERO,
            pal.fg,
        );
    }
    for (offset, track) in app.model.visible().iter().skip(start).take(capacity).enumerate() {
        let index = start + offset;
        let row = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, ROW_H));
        let selected = index == app.model.selected;
        if selected {
            p.rect_filled(row, CornerRadius::ZERO, pal.fg);
        }
        let color = if selected { pal.bg } else { pal.fg };
        let mut cx = x + 4.0;
        let mut cell = |text: &str, width: f32, anchor: Align2, p: &Painter| {
            let tx = if anchor == Align2::RIGHT_TOP { cx + width - 8.0 } else { cx };
            let shown = elide(p, text, 13.0, width);
            text_at(p, Pos2::new(tx, y + 7.0), anchor, &shown, 13.0, color);
            cx += width + 8.0;
        };
        cell(&format!("{:02}", index + 1), columns[0], Align2::RIGHT_TOP, p);
        let title = if likes::is_liked(&app.model.liked, &track.id) {
            format!("* {}", track.title)
        } else {
            track.title.clone()
        };
        cell(&title, columns[1], Align2::LEFT_TOP, p);
        cell(&track.artist, columns[2], Align2::LEFT_TOP, p);
        cell(&format_time(track.duration), columns[3], Align2::RIGHT_TOP, p);

        hits.rows.push((index, row));
        y += ROW_H;
    }
}
