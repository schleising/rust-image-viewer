use crate::document::{self, AnimFrame};
use crate::resources::icon_path;
use crate::state::AppState;
use crate::wake;
use dioxus::desktop::wry::http::{self, Request, Response};
use image::RgbaImage;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub enum ThumbStatus {
    Missing,
    Ready { animated: bool },
    Failed(String),
}

struct Thumb {
    png: Vec<u8>,
    error: Option<String>,
    animated: bool,
}

struct Store {
    folder_gen: u64,
    image_gen: u64,
    retained_gen: u64,
    frame: usize,
    frames: Arc<[AnimFrame]>,
    preview: Option<Arc<RgbaImage>>,
    preview_ticket: u64,
    video: HashMap<u64, PathBuf>,
    thumbs: HashMap<(u64, usize, u32), Thumb>,
    encoded: HashMap<(u64, usize, u64), (Vec<u8>, &'static str)>,
    near: HashMap<u64, Arc<[AnimFrame]>>,
    near_encoded: HashMap<(u64, usize), (Vec<u8>, &'static str)>,
    preload_epoch: u64,
}

static STORE: std::sync::LazyLock<Mutex<Store>> = std::sync::LazyLock::new(|| {
    Mutex::new(Store {
        folder_gen: 0,
        image_gen: 0,
        retained_gen: 0,
        frame: 0,
        frames: Arc::from([]),
        preview: None,
        preview_ticket: 0,
        video: HashMap::new(),
        thumbs: HashMap::new(),
        encoded: HashMap::new(),
        near: HashMap::new(),
        near_encoded: HashMap::new(),
        preload_epoch: 0,
    })
});

pub fn publish(state: &AppState) {
    let generation = state.image_generation;
    let ticket = if state.preview.is_some() {
        state.preview_ticket
    } else {
        0
    };
    let frame = state.frame_index;
    let folder_gen = state.folder_generation;
    let has_preview = state.preview.is_some();
    let preload_epoch = state.preload_epoch;
    let mut store = wake::lock(&*STORE);
    if store.image_gen == generation
        && store.frame == frame
        && store.preview_ticket == ticket
        && store.folder_gen == folder_gen
        && store.preview.is_some() == has_preview
        && store.preload_epoch == preload_epoch
    {
        return;
    }
    if store.image_gen != generation {
        store.retained_gen = store.image_gen;
    }
    let retained = store.retained_gen;
    store
        .encoded
        .retain(|(gen, _, _), _| *gen == generation || *gen == retained);
    store.folder_gen = state.folder_generation;
    store.image_gen = generation;
    store.frame = state.frame_index;
    store.frames = state.frames_arc().unwrap_or_else(|| Arc::from([]));
    store.preview = state.preview.clone();
    store.preview_ticket = ticket;
    if let Some(video) = state.video() {
        store.video.insert(video.token, video.path.clone());
    }
    store.preload_epoch = preload_epoch;
    refresh_near(&mut store, state);
    store
        .thumbs
        .retain(|(gen, _, _), _| *gen == state.folder_generation);
}

pub fn thumb_status(generation: u64, index: usize, px: u32) -> ThumbStatus {
    match wake::lock(&*STORE).thumbs.get(&(generation, index, px)) {
        None => ThumbStatus::Missing,
        Some(thumb) => match &thumb.error {
            Some(message) => ThumbStatus::Failed(message.clone()),
            None => ThumbStatus::Ready {
                animated: thumb.animated,
            },
        },
    }
}

pub fn store_thumb(
    generation: u64,
    index: usize,
    px: u32,
    result: Result<(Vec<u8>, bool), String>,
) {
    let mut store = wake::lock(&*STORE);
    if generation != store.folder_gen {
        return;
    }
    let thumb = match result {
        Ok((png, animated)) => Thumb {
            png,
            error: None,
            animated,
        },
        Err(message) => Thumb {
            png: Vec::new(),
            error: Some(message),
            animated: false,
        },
    };
    store.thumbs.insert((generation, index, px), thumb);
}

pub fn handle(request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = request_path(request.uri());
    let query = request.uri().query().unwrap_or("");
    if let Some(name) = path.strip_prefix("/icon/") {
        return icon_response(name);
    }
    if let Some(rest) = path.strip_prefix("/thumb/") {
        return thumb_response(rest, query);
    }
    if let Some(spec) = path.strip_prefix("/image/") {
        return image_response(spec);
    }
    if let Some(token) = path.strip_prefix("/video/") {
        return video_response(token, request.headers().get(http::header::RANGE));
    }
    if let Some(spec) = path.strip_prefix("/near/") {
        return near_response(spec);
    }
    empty(404)
}

fn request_path(uri: &http::Uri) -> String {
    let path = uri.path();
    match uri.host() {
        Some(host) if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") => {
            format!("/{host}{path}")
        }
        _ => path.to_string(),
    }
}

fn icon_response(name: &str) -> Response<Cow<'static, [u8]>> {
    let Some(path) = icon_path(name) else {
        return empty(404);
    };
    match std::fs::read(path) {
        Ok(bytes) => bytes_response(200, "image/png", bytes, &[]),
        Err(error) => {
            tracing::warn!(%error, "could not read an icon");
            empty(404)
        }
    }
}

fn thumb_response(rest: &str, query: &str) -> Response<Cow<'static, [u8]>> {
    let mut parts = rest.split('/');
    let generation = parts.next().and_then(|text| text.parse().ok()).unwrap_or(0);
    let index = parts.next().and_then(|text| text.parse().ok()).unwrap_or(0);
    let px = query_u32(query, "s").unwrap_or(128);
    let bytes = wake::lock(&*STORE)
        .thumbs
        .get(&(generation, index, px))
        .and_then(|thumb| thumb.error.is_none().then(|| thumb.png.clone()));
    match bytes {
        Some(bytes) => bytes_response(200, "image/png", bytes, &[]),
        None => empty(404),
    }
}

fn image_response(spec: &str) -> Response<Cow<'static, [u8]>> {
    let (requested_gen, requested_frame, requested_preview) = image_spec(spec);
    let cache_key = (requested_gen, requested_frame, requested_preview);
    let cached = wake::lock(&*STORE)
        .encoded
        .get(&cache_key)
        .map(|(bytes, mime)| (bytes.clone(), *mime));
    if let Some((bytes, mime)) = cached {
        return bytes_response(200, mime, bytes, &[]);
    }
    let (image, preview) = {
        let store = wake::lock(&*STORE);
        if store.image_gen != requested_gen || store.preview_ticket != requested_preview {
            return empty(404);
        }
        if store.preview.is_some() {
            (None, store.preview.clone())
        } else {
            (
                store
                    .frames
                    .get(requested_frame)
                    .map(|item| Arc::clone(&item.image)),
                None,
            )
        }
    };
    let shown = preview.or(image);
    let Some(shown) = shown else {
        return empty(404);
    };
    let preview_encode = requested_preview != 0;
    match document::encode_for_view(&shown, preview_encode) {
        Ok((bytes, mime)) => {
            wake::lock(&*STORE)
                .encoded
                .insert(cache_key, (bytes.clone(), mime));
            bytes_response(200, mime, bytes, &[])
        }
        Err(error) => {
            tracing::warn!(%error, "could not encode the frame");
            empty(500)
        }
    }
}

fn refresh_near(store: &mut Store, state: &crate::state::AppState) {
    let mut near = HashMap::new();
    if let Some(token) = state.warm_token() {
        if let Some(frames) = state.image().map(|image| Arc::clone(&image.frames)) {
            near.insert(token, frames);
        }
    }
    for (token, frames) in state.near_frames() {
        near.insert(token, frames);
    }
    store
        .near_encoded
        .retain(|(token, _), _| near.contains_key(token));
    store.near = near;
}

fn near_response(spec: &str) -> Response<Cow<'static, [u8]>> {
    let mut parts = spec.split('/');
    let Some(token) = parts.next().and_then(|text| text.parse().ok()) else {
        return empty(404);
    };
    let frame = match (parts.next(), parts.next()) {
        (Some("f"), Some(text)) => text.parse().unwrap_or(0),
        _ => 0,
    };
    let cached = wake::lock(&*STORE)
        .near_encoded
        .get(&(token, frame))
        .map(|(bytes, mime)| (bytes.clone(), *mime));
    if let Some((bytes, mime)) = cached {
        return bytes_response(200, mime, bytes, &[]);
    }
    let image = wake::lock(&*STORE)
        .near
        .get(&token)
        .and_then(|frames| frames.get(frame))
        .map(|item| Arc::clone(&item.image));
    let Some(image) = image else {
        return empty(404);
    };
    match document::encode_for_view(&image, false) {
        Ok((bytes, mime)) => {
            wake::lock(&*STORE)
                .near_encoded
                .insert((token, frame), (bytes.clone(), mime));
            bytes_response(200, mime, bytes, &[])
        }
        Err(error) => {
            tracing::warn!(%error, "could not encode the frame");
            empty(500)
        }
    }
}

fn image_spec(spec: &str) -> (u64, usize, u64) {
    let mut generation = 0_u64;
    let mut frame = 0_usize;
    let mut preview = 0_u64;
    let mut parts = spec.split('/');
    while let Some(label) = parts.next() {
        let Some(value) = parts.next() else {
            break;
        };
        match label {
            "g" => generation = value.parse().unwrap_or(0),
            "f" => frame = value.parse().unwrap_or(0),
            "p" => preview = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    (generation, frame, preview)
}

fn video_response(token: &str, range: Option<&http::HeaderValue>) -> Response<Cow<'static, [u8]>> {
    let Ok(token) = token.parse::<u64>() else {
        return empty(404);
    };
    let path = {
        let store = wake::lock(&*STORE);
        store.video.get(&token).cloned()
    };
    let Some(path) = path else {
        return empty(404);
    };
    let mime = if path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("mov")
    {
        "video/quicktime"
    } else {
        "video/mp4"
    };
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "could not open the video");
            return empty(404);
        }
    };
    let length = match file.metadata() {
        Ok(meta) => meta.len(),
        Err(error) => {
            tracing::warn!(%error, "could not read the video length");
            return empty(404);
        }
    };
    let range = range
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_range(value, length));
    if let Some((start, end)) = range {
        if file.seek(SeekFrom::Start(start)).is_err() {
            return empty(416);
        }
        let size = (end - start + 1) as usize;
        let mut bytes = vec![0; size];
        if file.read_exact(&mut bytes).is_err() {
            return empty(416);
        }
        bytes_response(
            206,
            mime,
            bytes,
            &[
                ("Content-Range", format!("bytes {start}-{end}/{length}")),
                ("Accept-Ranges", "bytes".into()),
            ],
        )
    } else {
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() {
            return empty(404);
        }
        bytes_response(200, mime, bytes, &[("Accept-Ranges", "bytes".into())])
    }
}

fn parse_range(header: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let spec = header.trim().strip_prefix("bytes=")?;
    let (start_text, end_text) = spec.split_once('-')?;
    if start_text.is_empty() {
        let suffix: u64 = end_text.parse().ok()?;
        let start = length.saturating_sub(suffix.max(1));
        return Some((start, length - 1));
    }
    let start: u64 = start_text.parse().ok()?;
    if start >= length {
        return None;
    }
    let end = if end_text.is_empty() {
        length - 1
    } else {
        end_text.parse::<u64>().ok()?.min(length - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

fn query_u32(query: &str, key: &str) -> Option<u32> {
    query_value(query, key)?.parse().ok()
}

fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then_some(value)
    })
}

fn bytes_response(
    status: u16,
    mime: &str,
    body: Vec<u8>,
    extra: &[(&str, String)],
) -> Response<Cow<'static, [u8]>> {
    let mut builder = Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, mime);
    for (name, value) in extra {
        builder = builder.header(*name, value);
    }
    match builder.body(Cow::Owned(body)) {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "could not build a protocol response");
            empty(500)
        }
    }
}

fn empty(status: u16) -> Response<Cow<'static, [u8]>> {
    match Response::builder()
        .status(status)
        .body(Cow::Borrowed(&[][..]))
    {
        Ok(response) => response,
        Err(_) => Response::new(Cow::Borrowed(&[][..])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_scheme_urls_keep_the_first_segment() {
        let icon: http::Uri = "app://icon/folder.png"
            .parse()
            .unwrap_or_else(|_| http::Uri::from_static("/"));
        assert_eq!(request_path(&icon), "/icon/folder.png");
        let thumb: http::Uri = "app://thumb/3/4?s=128"
            .parse()
            .unwrap_or_else(|_| http::Uri::from_static("/"));
        assert_eq!(request_path(&thumb), "/thumb/3/4");
        assert_eq!(thumb.query(), Some("s=128"));
        let rooted: http::Uri = "app://localhost/image/current"
            .parse()
            .unwrap_or_else(|_| http::Uri::from_static("/"));
        assert_eq!(request_path(&rooted), "/image/current");
    }

    #[test]
    fn ranges_cover_a_suffix_and_an_open_end() {
        assert_eq!(parse_range("bytes=0-3", 10), Some((0, 3)));
        assert_eq!(parse_range("bytes=4-", 10), Some((4, 9)));
        assert_eq!(parse_range("bytes=-2", 10), Some((8, 9)));
        assert_eq!(parse_range("bytes=10-11", 10), None);
    }
}
