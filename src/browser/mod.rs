mod thumbnail;
mod video_frame;

use crate::catalog::{middle_elide, EntryKind};
use crate::effects;
use crate::protocol::{self, ThumbStatus};
use crate::state::AppState;
use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

pub fn view(mut state: Signal<AppState>, window: DesktopContext) -> Element {
    let generation = state.read().folder_generation;
    let px = state.read().thumb_px();
    let epoch = state.read().thumb_epoch;
    let entries = state.read().folder.entries.clone();
    let highlighted = state.read().folder.highlighted;
    let _ = epoch;
    rsx! {
        div {
            id: "browser",
            class: "browser",
            onmounted: move |_| {
                dioxus::document::eval(
                    "requestAnimationFrame(() => { const cell = document.querySelector('.cell.selected'); if (cell) cell.scrollIntoView({block:'nearest', inline:'nearest'}); });",
                );
            },
            onscroll: move |event| {
                state.write().set_scroll(event.data().scroll_top());
            },
            for (index, entry) in entries.into_iter().enumerate() {
                {
                    let selected = index == highlighted;
                    let label = middle_elide(&entry.label, 22);
                    let kind = entry.kind;
                    let src = thumb_src(generation, index, px, kind);
                    let message = thumb_message(generation, index, px, kind);
                    let badge = video_badge(generation, index, px);
                    let class = if selected { "cell selected" } else { "cell" };
                    let window = window.clone();
                    rsx! {
                        button {
                            key: "{index}",
                            class: "{class}",
                            onclick: move |_| {
                                let effect = state.write().activate_index(index);
                                effects::run(state, window.clone(), effect);
                            },
                            div { class: "thumb-box",
                                if let Some(message) = message {
                                    div { class: "thumb-error", "{message}" }
                                } else if let Some(src) = src {
                                    img {
                                        class: if badge || matches!(kind, EntryKind::Image) { "thumb" } else { "thumb icon" },
                                        src: "{src}",
                                    }
                                    if badge {
                                        div { class: "video-badge" }
                                    }
                                } else {
                                    img { class: "thumb loading", src: "app://icon/loading.png" }
                                }
                            }
                            div { class: "label", "{label}" }
                        }
                    }
                }
            }
        }
    }
}

fn thumb_src(generation: u64, index: usize, px: u32, kind: EntryKind) -> Option<String> {
    match kind {
        EntryKind::Parent | EntryKind::Directory => Some("app://icon/folder.png".into()),
        EntryKind::Video => match protocol::thumb_status(generation, index, px) {
            ThumbStatus::Ready { .. } => Some(format!("app://thumb/{generation}/{index}?s={px}")),
            ThumbStatus::Failed(_) => Some("app://icon/video.png".into()),
            ThumbStatus::Missing => None,
        },
        EntryKind::Image => match protocol::thumb_status(generation, index, px) {
            ThumbStatus::Ready { .. } => Some(format!("app://thumb/{generation}/{index}?s={px}")),
            _ => None,
        },
    }
}

fn video_badge(generation: u64, index: usize, px: u32) -> bool {
    matches!(
        protocol::thumb_status(generation, index, px),
        ThumbStatus::Ready { animated: true }
    )
}

fn thumb_message(generation: u64, index: usize, px: u32, kind: EntryKind) -> Option<String> {
    if kind != EntryKind::Image {
        return None;
    }
    match protocol::thumb_status(generation, index, px) {
        ThumbStatus::Failed(message) => Some(message),
        _ => None,
    }
}

pub fn schedule(state: &AppState) {
    thumbnail::ensure(&state.folder, state.folder_generation, state.thumb_px());
}
