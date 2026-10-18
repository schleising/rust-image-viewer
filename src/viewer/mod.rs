use crate::command::Command;
use crate::effects;
use crate::state::{AppState, FrameLayer};
use crate::viewport::{self, PixelRect};
use dioxus::desktop::DesktopContext;
use dioxus::html::geometry::WheelDelta;
use dioxus::prelude::*;

pub fn view(mut state: Signal<AppState>, window: DesktopContext) -> Element {
    let sync_video = state.read().video_frame_needs_sync();
    if sync_video {
        state.write().retain_video_frame();
    }
    let sync_image = state.read().display_needs_sync();
    if sync_image {
        state.write().sync_display();
    }
    let (
        view_size,
        selection,
        scrub,
        place,
        show_front,
        front,
        back,
        cover_stills,
        front_video,
        back_video,
        front_video_style,
        back_video_style,
        player_paused,
        player_muted,
    ) = {
        let snapshot = state.read();
        let shown = snapshot.shown_video_token();
        let front_video = snapshot.video_front;
        let back_video = snapshot.video_back;
        let front_video_style = front_video.map(|token| {
            video_box_style(
                snapshot.view_size,
                &snapshot.video_box(token),
                shown == Some(token),
            )
        });
        let back_video_style = back_video.map(|token| {
            video_box_style(
                snapshot.view_size,
                &snapshot.video_box(token),
                shown == Some(token),
            )
        });
        (
            snapshot.view_size,
            selection_style(&snapshot),
            scrub_style(&snapshot),
            if snapshot.place_visible {
                snapshot.place_label()
            } else {
                None
            },
            snapshot.show_front,
            snapshot.front.clone(),
            snapshot.back.clone(),
            shown.is_some(),
            front_video,
            back_video,
            front_video_style,
            back_video_style,
            snapshot.video().is_some_and(|item| item.paused),
            snapshot.video().is_some_and(|item| item.muted),
        )
    };
    let pointer_window = window.clone();
    let wheel_window = window.clone();
    let click_window = window.clone();
    let front_url = front.url.clone();
    let back_url = back.url.clone();
    let front_load = front_url.clone();
    let back_load = back_url.clone();
    let front_style = frame_style(view_size, &front, show_front && !cover_stills);
    let back_style = frame_style(view_size, &back, !show_front && !cover_stills);
    let front_src = frame_src(&front_url);
    let back_src = frame_src(&back_url);
    rsx! {
        div {
            id: "stage",
            class: "stage",
            onmounted: move |_| {
                dioxus::document::eval(
                    "(() => { const stage = document.getElementById('stage'); if (!stage || stage.dataset.capture) return; stage.dataset.capture = '1'; stage.addEventListener('wheel', (event) => event.preventDefault(), { passive: false }); stage.addEventListener('pointerdown', (event) => { if (event.button === 0) stage.setPointerCapture(event.pointerId); }); })()",
                );
            },
            onwheel: move |event| {
                event.prevent_default();
                if state.read().long_operation() {
                    return;
                }
                let point = event.data().element_coordinates();
                let delta = event.data().delta();
                let lines = matches!(delta, WheelDelta::Lines(_) | WheelDelta::Pages(_));
                let vector = delta.strip_units();
                let effect = state.write().on_wheel(
                    lines,
                    vector.x,
                    vector.y,
                    (point.x, point.y),
                    std::time::Instant::now(),
                );
                effects::run(state, wheel_window.clone(), effect);
            },
            onpointerdown: move |event| {
                event.prevent_default();
                if state.read().long_operation() {
                    return;
                }
                let point = event.data().element_coordinates();
                if scrub_hit(&state.read(), point.y) {
                    let fraction = scrub_fraction(&state.read(), point.x);
                    let effect = state
                        .write()
                        .dispatch(Command::Seek { fraction }, std::time::Instant::now());
                    effects::run(state, pointer_window.clone(), effect);
                    return;
                }
                let control = event.data().modifiers().contains(Modifiers::CONTROL);
                state.write().pointer_down((point.x, point.y), control);
            },
            onpointermove: move |event| {
                if state.read().long_operation() {
                    return;
                }
                let point = event.data().element_coordinates();
                state.write().pointer_move((point.x, point.y));
                state.write().reveal_place(std::time::Instant::now());
                if state.read().video().is_some() {
                    let token = state.write().poke_scrub();
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        state.write().hide_scrub(token);
                    });
                }
            },
            onpointerup: move |event| {
                let point = event.data().element_coordinates();
                state.write().pointer_up((point.x, point.y));
            },
            ondoubleclick: move |_| {
                if state.read().long_operation() {
                    return;
                }
                let now = std::time::Instant::now();
                if state.read().open_fullscreen_guard(now) {
                    return;
                }
                let effect = state.write().dispatch(Command::ToggleFullscreen, now);
                effects::run(state, click_window.clone(), effect);
            },
            img {
                id: "frame-back",
                class: "frame",
                src: "{back_src}",
                style: "{back_style}",
                onload: move |_| reveal_loaded(&mut state, &back_load),
            }
            img {
                id: "frame-front",
                class: "frame",
                src: "{front_src}",
                style: "{front_style}",
                onload: move |_| reveal_loaded(&mut state, &front_load),
            }
            if let Some((token, style)) = front_video.zip(front_video_style) {
                { video_slot(state, "front", token, style, player_paused, player_muted) }
            }
            if let Some((token, style)) = back_video.zip(back_video_style) {
                { video_slot(state, "back", token, style, player_paused, player_muted) }
            }
            if let Some(style) = selection {
                div { class: "selection", style: "{style}" }
            }
            if let Some((fill, mark)) = scrub {
                div { class: "scrubber",
                    div { class: "scrubber-fill", style: "{fill}" }
                    div { class: "mark", style: "{mark}" }
                }
            }
            if let Some(place) = place {
                div { class: "place", "{place}" }
            }
        }
    }
}

fn video_slot(
    mut state: Signal<AppState>,
    slot: &'static str,
    token: u64,
    style: String,
    paused: bool,
    document_muted: bool,
) -> Element {
    let visible = state.read().shown_video_token() == Some(token);
    let element_id = if visible { "player" } else { slot };
    let play = visible && !paused;
    let quiet = !visible || document_muted;
    rsx! {
        video {
            key: "{slot}-{token}",
            id: "{element_id}",
            src: "app://video/{token}",
            autoplay: play,
            muted: quiet,
            preload: "auto",
            r#loop: true,
            style: "{style}",
            onloadeddata: move |_| {
                let change = state.read().promote_video_would_change(token);
                if change {
                    state.write().promote_video(token);
                }
            },
        }
    }
}

fn video_box_style(view: (f64, f64), layer: &FrameLayer, visible: bool) -> String {
    let geometry = media_style(
        view,
        layer.size,
        layer.scale,
        layer.translation,
        layer.fitted,
    );
    let z = if visible { 5 } else { 2 };
    let opacity = if visible { "1" } else { "0" };
    format!("{geometry}z-index:{z};opacity:{opacity};")
}

pub fn sync_video(video: &crate::document::VideoDocument) {
    let paused = if video.paused { "true" } else { "false" };
    let muted = if video.muted { "true" } else { "false" };
    let script = format!(
        "(() => {{ document.querySelectorAll('video').forEach((node) => {{ if (node.id !== 'player') {{ node.pause(); node.muted = true; }} }}); const v = document.getElementById('player'); if (!v) return; v.volume = {volume}; v.muted = {muted}; if (Number.isFinite({position}) && Math.abs((v.currentTime || 0) - {position}) > 0.35) v.currentTime = {position}; if ({paused}) v.pause(); else v.play(); }})()",
        volume = video.volume,
        position = video.position,
    );
    dioxus::document::eval(&script);
}

pub fn bind_player(mut state: Signal<AppState>) {
    spawn(async move {
        let mut eval = dioxus::document::eval(
            "(() => { const attach = () => { const v = document.getElementById('player'); if (!v || v.dataset.bound) return; v.dataset.bound = '1'; const send = () => { if (v.id !== 'player') return; dioxus.send([v.currentTime || 0, v.duration || 0, v.videoWidth || 0, v.videoHeight || 0].join(',')); }; v.addEventListener('timeupdate', send); v.addEventListener('loadedmetadata', send); send(); }; attach(); new MutationObserver(attach).observe(document.documentElement, {childList:true, subtree:true, attributes:true, attributeFilter:['id']}); })()",
        );
        loop {
            let Ok(text) = eval.recv::<String>().await else {
                break;
            };
            apply_player_text(&mut state, &text);
        }
    });
}

fn apply_player_text(state: &mut Signal<AppState>, text: &str) {
    let mut parts = text.split(',');
    let position = parts
        .next()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0.0);
    let duration = parts
        .next()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0.0);
    let width = parts.next().and_then(|text| text.parse().ok()).unwrap_or(0);
    let height = parts.next().and_then(|text| text.parse().ok()).unwrap_or(0);
    if width > 0 && height > 0 {
        state.write().set_video_size((width, height), duration);
    }
    state.write().set_video_time(position, duration);
}

const FRAME_PLACEHOLDER: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

fn frame_src(url: &str) -> String {
    if url.is_empty() {
        FRAME_PLACEHOLDER.to_string()
    } else {
        url.to_string()
    }
}

fn reveal_loaded(state: &mut Signal<AppState>, url: &str) {
    if url.is_empty() {
        return;
    }
    let reveal = state.read().reveal_would_change(url);
    if reveal {
        state.write().reveal_frame(url);
    }
}

fn frame_style(view: (f64, f64), layer: &FrameLayer, on_top: bool) -> String {
    if layer.url.is_empty() {
        return "position:absolute;width:0;height:0;opacity:0;".into();
    }
    let geometry = media_style(
        view,
        layer.size,
        layer.scale,
        layer.translation,
        layer.fitted,
    );
    let z = if on_top { 3 } else { 1 };
    let opacity = if on_top { "1" } else { "0" };
    format!("{geometry}z-index:{z};opacity:{opacity};")
}

fn media_style(
    view: (f64, f64),
    size: Option<(u32, u32)>,
    scale: f64,
    translation: (f64, f64),
    fitted: bool,
) -> String {
    let Some(size) = size else {
        return "width:100%;height:100%;object-fit:contain;".into();
    };
    if size.0 == 0 || size.1 == 0 {
        return "width:100%;height:100%;object-fit:contain;".into();
    }
    let factor = if fitted { 1.0 } else { scale };
    let (base_width, base_height) = viewport::fitted_box(view, size);
    let width = base_width * factor;
    let height = base_height * factor;
    format!(
        "position:absolute;left:0;top:0;width:{width}px;height:{height}px;transform:translate({x}px,{y}px);",
        x = translation.0,
        y = translation.1,
    )
}

fn selection_style(state: &AppState) -> Option<String> {
    let rect = state.viewport.selection?;
    let size = state.pixel_size()?;
    Some(rect_style(state, size, rect))
}

fn rect_style(state: &AppState, size: (u32, u32), rect: PixelRect) -> String {
    let display = viewport::fit_factor(state.view_size, size) * state.viewport.scale;
    let left = state.viewport.translation.0 + rect.x as f64 * display;
    let top = state.viewport.translation.1 + rect.y as f64 * display;
    let width = rect.width as f64 * display;
    let height = rect.height as f64 * display;
    format!("left:{left}px;top:{top}px;width:{width}px;height:{height}px;")
}

fn scrub_style(state: &AppState) -> Option<(String, String)> {
    let video = state.video()?;
    if !state.scrub_visible || video.duration <= 0.0 {
        return None;
    }
    let fraction = (video.position / video.duration).clamp(0.0, 1.0);
    let percent = fraction * 100.0;
    let fill = format!("width:{percent}%;");
    let mark = format!("left:calc({percent}% - 6px);");
    Some((fill, mark))
}

fn scrub_hit(state: &AppState, y: f64) -> bool {
    if state.video().is_none() {
        return false;
    }
    let bar = state.view_size.1 - 100.0;
    (y - bar).abs() <= 16.0
}

fn scrub_fraction(state: &AppState, x: f64) -> f32 {
    let width = (state.view_size.0 - 200.0).max(1.0);
    ((x - 100.0) / width).clamp(0.0, 1.0) as f32
}
