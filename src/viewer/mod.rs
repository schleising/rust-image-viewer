use crate::command::Command;
use crate::effects;
use crate::state::AppState;
use crate::viewport::{self, PixelRect};
use dioxus::desktop::DesktopContext;
use dioxus::html::geometry::WheelDelta;
use dioxus::prelude::*;

pub fn view(mut state: Signal<AppState>, window: DesktopContext) -> Element {
    let video = state.read().video().cloned();
    let frame = state.read().frame_index;
    let generation = state.read().image_generation;
    let preview = state.read().preview_ticket;
    let busy = state.read().busy;
    let size = state.read().pixel_size();
    let view_size = state.read().view_size;
    let fitted = state.read().viewport.fitted;
    let scale = state.read().viewport.scale;
    let translation = state.read().viewport.translation;
    let selection = selection_style(&state.read());
    let scrub = scrub_style(&state.read());
    let class = if busy { "stage busy" } else { "stage" };
    let pointer_window = window.clone();
    let click_window = window.clone();
    let geometry = media_style(view_size, size, scale, translation, fitted);
    rsx! {
        div {
            id: "stage",
            class: "{class}",
            onmounted: move |_| {
                dioxus::document::eval(
                    "(() => { const stage = document.getElementById('stage'); if (!stage || stage.dataset.capture) return; stage.dataset.capture = '1'; stage.addEventListener('pointerdown', (event) => { if (event.button === 0) stage.setPointerCapture(event.pointerId); }); })()",
                );
            },
            onwheel: move |event| {
                event.prevent_default();
                let point = event.data().element_coordinates();
                let delta = event.data().delta();
                let lines = matches!(delta, WheelDelta::Lines(_) | WheelDelta::Pages(_));
                let delta_y = delta.strip_units().y;
                state.write().on_wheel(lines, delta_y, (point.x, point.y));
            },
            onpointerdown: move |event| {
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
                let point = event.data().element_coordinates();
                state.write().pointer_move((point.x, point.y));
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
                let effect = state
                    .write()
                    .dispatch(Command::ToggleFullscreen, std::time::Instant::now());
                effects::run(state, click_window.clone(), effect);
            },
            if let Some(video) = video {
                video {
                    id: "player",
                    src: "app://video/{video.token}",
                    autoplay: !video.paused,
                    r#loop: true,
                    style: "{geometry}",
                }
            } else {
                img {
                    class: "frame",
                    src: "app://image/current?g={generation}&f={frame}&p={preview}",
                    style: "{geometry}",
                }
            }
            if let Some(style) = selection {
                div { class: "selection", style: "{style}" }
            }
            if let Some((style, mark)) = scrub {
                div { class: "scrubber", style: "{style}",
                    div { class: "mark", style: "{mark}" }
                }
            }
        }
    }
}

pub fn sync_video(video: &crate::document::VideoDocument) {
    let paused = if video.paused { "true" } else { "false" };
    let muted = if video.muted { "true" } else { "false" };
    let script = format!(
        "(() => {{ const v = document.getElementById('player'); if (!v) return; v.volume = {volume}; v.muted = {muted}; if (Number.isFinite({position}) && Math.abs((v.currentTime || 0) - {position}) > 0.35) v.currentTime = {position}; if ({paused}) v.pause(); else v.play(); }})()",
        volume = video.volume,
        position = video.position,
    );
    dioxus::document::eval(&script);
}

pub fn bind_player(mut state: Signal<AppState>) {
    spawn(async move {
        let mut eval = dioxus::document::eval(
            "(() => { const attach = () => { const v = document.getElementById('player'); if (!v || v.dataset.bound) return; v.dataset.bound = '1'; const send = () => dioxus.send([v.currentTime || 0, v.duration || 0, v.videoWidth || 0, v.videoHeight || 0].join(',')); v.addEventListener('timeupdate', send); v.addEventListener('loadedmetadata', send); send(); }; attach(); new MutationObserver(attach).observe(document.documentElement, {childList:true, subtree:true}); })()",
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
    let mark = format!("left:calc({}% - 4px);", fraction * 100.0);
    Some(("".into(), mark))
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
