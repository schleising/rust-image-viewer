use crate::document::{self, AnimFrame};
use crate::state::{AppState, Effect};
use crate::transforms::{self, Params};
use crate::wake::{self, Note};
use dioxus::desktop::DesktopContext;
use dioxus::prelude::{ReadableExt, Signal, WritableExt};
use std::sync::Arc;

pub fn run(mut state: Signal<AppState>, window: DesktopContext, effect: Effect) {
    match effect {
        Effect::None => {}
        Effect::Quit => window.close(),
        Effect::ToggleFullscreen => {
            let fullscreen = state.read().fullscreen;
            window.set_fullscreen(fullscreen);
            if !fullscreen {
                // Leaving full screen drops the web view as the keyboard
                // target when the animation finishes. Claim it now, and again
                // on the resize that ends that animation.
                focus_page(&window);
            }
        }
        Effect::SyncVideo => {
            let next = state.read().video_epoch.wrapping_add(1);
            state.write().video_epoch = next;
        }
        Effect::FocusPage => focus_page(&window),
        Effect::ListFolder(path) => {
            wake::spawn_blocking(move || {
                let result = crate::catalog::list_folder(&path).map_err(|error| error.to_string());
                wake::post(Note::Folder(result));
            });
        }
        Effect::LoadFile { ticket, path } => {
            wake::spawn_blocking(move || {
                let result = document::load(&path).map_err(|error| error.message);
                wake::post(Note::Loaded { ticket, result });
            });
        }
        Effect::OpenFile {
            folder,
            ticket,
            path,
        } => {
            wake::spawn_blocking(move || {
                let result =
                    crate::catalog::list_folder(&folder).map_err(|error| error.to_string());
                wake::post(Note::Folder(result));
            });
            wake::spawn_blocking(move || {
                let result = document::load(&path).map_err(|error| error.message);
                wake::post(Note::Loaded { ticket, result });
            });
        }
        Effect::Save { ticket } => {
            let image = state.read().image().cloned();
            let stamp = document::local_stamp(chrono::Local::now());
            wake::spawn_blocking(move || {
                let result = match image {
                    Some(image) => document::save_document(&image, &stamp)
                        .map(|_| ())
                        .map_err(|error| error.message),
                    None => Err("there is no image to save".into()),
                };
                wake::post(Note::Saved { ticket, result });
            });
        }
        Effect::Transform { ticket, id, params } => {
            let frames = state.read().frames_arc();
            spawn_frames(ticket, frames, move |frames| {
                document::apply_transform_all(&frames, id, &params)
                    .map_err(|error| error.to_string())
            });
        }
        Effect::Crop { ticket, rect } => {
            let frames = state.read().frames_arc();
            spawn_frames(ticket, frames, move |frames| {
                document::crop_all(&frames, rect).map_err(|error| error.message)
            });
        }
        Effect::Preview {
            ticket,
            colour,
            contrast,
            brightness,
        } => {
            let frame = source_frame(&state);
            wake::spawn_blocking(move || {
                let image = frame.map(|frame| {
                    Arc::new(transforms::apply_adjust(
                        &frame, colour, contrast, brightness,
                    ))
                });
                wake::post(Note::Preview { ticket, image });
            });
        }
        Effect::CommitAdjust {
            ticket,
            colour,
            contrast,
            brightness,
        } => {
            let frames = state.read().adjust_frames();
            spawn_frames(ticket, frames, move |frames| {
                let params = Params::Adjust {
                    colour,
                    contrast,
                    brightness,
                };
                adjust_each(&frames, &params)
            });
        }
    }
}

fn source_frame(state: &Signal<AppState>) -> Option<Arc<image::RgbaImage>> {
    let state = state.read();
    let index = state.frame_index;
    state
        .image()
        .and_then(|image| image.frames.get(index))
        .map(|frame| Arc::clone(&frame.image))
}

fn spawn_frames(
    ticket: u64,
    frames: Option<Arc<[AnimFrame]>>,
    work: impl FnOnce(Arc<[AnimFrame]>) -> Result<Vec<AnimFrame>, String> + Send + 'static,
) {
    wake::spawn_blocking(move || {
        let result = match frames {
            Some(frames) => work(frames),
            None => Err("there is no image to edit".into()),
        };
        wake::post(Note::Transformed { ticket, result });
    });
}

fn adjust_each(frames: &[AnimFrame], params: &Params) -> Result<Vec<AnimFrame>, String> {
    let Params::Adjust {
        colour,
        contrast,
        brightness,
    } = *params
    else {
        return Err(transforms::TransformError::BadParams.to_string());
    };
    let mut next = Vec::with_capacity(frames.len());
    for frame in frames {
        next.push(AnimFrame {
            image: Arc::new(transforms::apply_adjust(
                &frame.image,
                colour,
                contrast,
                brightness,
            )),
            delay: frame.delay,
        });
    }
    Ok(next)
}

pub(crate) fn focus_page(window: &DesktopContext) {
    if let Err(error) = window.webview.focus() {
        tracing::warn!(%error, "could not focus the page");
    }
    dioxus::document::eval("document.getElementById('root')?.focus()");
}
