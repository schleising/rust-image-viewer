use crate::document;
use crate::effects;
use crate::state::{AppState, Dialog};
use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;
use std::time::Instant;

pub fn view(state: Signal<AppState>, window: DesktopContext) -> Element {
    let dialog = state.read().dialog.clone();
    match dialog {
        Some(Dialog::Info) => info(state),
        Some(Dialog::Adjust {
            colour,
            contrast,
            brightness,
        }) => adjust(state, window, colour, contrast, brightness),
        None => rsx! {},
    }
}

fn info(mut state: Signal<AppState>) -> Element {
    let lines = state
        .read()
        .image()
        .map(document::info_lines)
        .unwrap_or_default();
    rsx! {
        div { class: "dialog-back",
            div { class: "dialog",
                h2 { "Image Information" }
                for (name, value) in lines {
                    div { class: "info-row",
                        span { "{name}" }
                        span { "{value}" }
                    }
                }
                div { class: "dialog-actions",
                    button {
                        onclick: move |_| {
                            state.write().dialog = None;
                        },
                        "OK"
                    }
                }
            }
        }
    }
}

fn adjust(
    state: Signal<AppState>,
    window: DesktopContext,
    colour: f32,
    contrast: f32,
    brightness: f32,
) -> Element {
    let colour_window = window.clone();
    let contrast_window = window.clone();
    let brightness_window = window.clone();
    let cancel_window = window.clone();
    let ok_window = window;
    rsx! {
        div { class: "dialog-back",
            div { class: "dialog",
                h2 { "Adjust Image" }
                {slider(state, colour_window, "Colour", colour, contrast, brightness, Slider::Colour)}
                {slider(state, contrast_window, "Contrast", colour, contrast, brightness, Slider::Contrast)}
                {slider(state, brightness_window, "Brightness", colour, contrast, brightness, Slider::Brightness)}
                div { class: "dialog-actions",
                    button {
                        onclick: move |_| finish(state, cancel_window.clone(), false),
                        "Cancel"
                    }
                    button {
                        onclick: move |_| finish(state, ok_window.clone(), true),
                        "OK"
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Slider {
    Colour,
    Contrast,
    Brightness,
}

fn slider(
    mut state: Signal<AppState>,
    window: DesktopContext,
    label: &'static str,
    colour: f32,
    contrast: f32,
    brightness: f32,
    which: Slider,
) -> Element {
    let value = match which {
        Slider::Colour => colour,
        Slider::Contrast => contrast,
        Slider::Brightness => brightness,
    };
    let shown = (value * 10.0).round() as i32;
    rsx! {
        label { class: "slider",
            span { "{label}" }
            input {
                r#type: "range",
                min: "0",
                max: "20",
                step: "1",
                value: "{shown}",
                oninput: move |event| {
                    let next = event.value().parse::<f32>().unwrap_or(10.0) / 10.0;
                    let (colour, contrast, brightness) = match which {
                        Slider::Colour => (next, contrast, brightness),
                        Slider::Contrast => (colour, next, brightness),
                        Slider::Brightness => (colour, contrast, next),
                    };
                    let effect = state.write().dispatch(
                        crate::command::Command::Preview { colour, contrast, brightness },
                        Instant::now(),
                    );
                    effects::run(state, window.clone(), effect);
                },
            }
        }
    }
}

fn finish(mut state: Signal<AppState>, window: DesktopContext, ok: bool) {
    let command = if ok {
        crate::command::Command::CommitPreview
    } else {
        crate::command::Command::CancelPreview
    };
    let effect = state.write().dispatch(command, Instant::now());
    effects::run(state, window, effect);
}
