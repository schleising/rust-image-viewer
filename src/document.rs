use crate::transforms::{self, Params, TransformError, TransformId};
use crate::viewport::{clamp_rect, PixelRect};
use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{
    AnimationDecoder, DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat,
    ImageReader, RgbaImage,
};
use image_webp::{LoopCount, WebPDecoder};
use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub const ANIMATION_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loop {
    Once,
    Forever,
    Count(u16),
}

#[derive(Clone, Debug)]
pub struct AnimFrame {
    pub image: Arc<RgbaImage>,
    pub delay: Duration,
}

#[derive(Clone, Debug)]
pub struct ImageMetadata {
    pub format_name: String,
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub mode: String,
}

#[derive(Clone, Debug)]
pub struct ImageDocument {
    pub path: PathBuf,
    pub metadata: ImageMetadata,
    pub frames: Arc<[AnimFrame]>,
    pub loop_count: Loop,
    pub undo: Vec<Arc<[AnimFrame]>>,
    pub clean: Arc<[AnimFrame]>,
}

#[derive(Clone, Debug)]
pub struct VideoDocument {
    pub path: PathBuf,
    pub token: u64,
    pub duration: f64,
    pub position: f64,
    pub paused: bool,
    pub volume: f32,
    pub muted: bool,
    pub speed: f32,
    pub loop_in: Option<f64>,
    pub loop_out: Option<f64>,
    pub size: (u32, u32),
}

#[derive(Clone, Debug)]
pub enum Document {
    Image(ImageDocument),
    Video(VideoDocument),
}

#[derive(Debug)]
pub struct DocError {
    pub message: String,
}

impl std::fmt::Display for DocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DocError {}

impl DocError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Document {
    pub fn path(&self) -> &Path {
        match self {
            Self::Image(document) => &document.path,
            Self::Video(document) => &document.path,
        }
    }
}

impl ImageDocument {
    pub fn is_dirty(&self) -> bool {
        !Arc::ptr_eq(&self.frames, &self.clean)
    }

    pub fn is_animation(&self) -> bool {
        self.frames.len() > 1
    }

    pub fn push_undo(&mut self, previous: Arc<[AnimFrame]>) {
        if self.undo.len() >= 16 {
            self.undo.remove(0);
        }
        self.undo.push(previous);
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.frames = previous;
        self.refresh_size();
        true
    }

    pub fn refresh_size(&mut self) {
        if let Some(frame) = self.frames.first() {
            self.metadata.width = frame.image.width();
            self.metadata.height = frame.image.height();
        }
    }
}

impl VideoDocument {
    pub fn loop_span(&self) -> Option<(f64, f64)> {
        let start = self.loop_in?;
        let end = self.loop_out?;
        if !start.is_finite() || !end.is_finite() || end - start < 0.05 {
            return None;
        }
        Some((start, end))
    }

    pub fn contain_in_loop(&mut self) {
        let Some((start, end)) = self.loop_span() else {
            return;
        };
        if self.position >= end {
            self.position = start;
        }
    }
}

pub fn load(path: &Path) -> Result<Document, DocError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let kind = crate::catalog::file_kind(name).ok_or_else(|| DocError::new("unsupported file"))?;
    if kind == crate::catalog::EntryKind::Video {
        return Ok(Document::Video(VideoDocument {
            path: path.to_path_buf(),
            token: 0,
            duration: 0.0,
            position: 0.0,
            paused: false,
            volume: 1.0,
            muted: false,
            speed: 1.0,
            loop_in: None,
            loop_out: None,
            size: (0, 0),
        }));
    }
    let bytes = fs::read(path).map_err(|error| DocError::new(error.to_string()))?;
    let suffix = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let mut document = match suffix.as_str() {
        "gif" => load_gif(path, &bytes)?,
        "webp" => load_webp(path, &bytes)?,
        _ => load_still(path, &bytes)?,
    };
    if let Document::Image(image) = &mut document {
        image.frames = enforce_load_limit(image.frames.to_vec());
        if image.frames.len() <= 1 {
            image.loop_count = Loop::Once;
        }
        image.clean = Arc::clone(&image.frames);
        image.refresh_size();
    }
    Ok(document)
}

pub fn frame_bytes(frames: &[AnimFrame]) -> u64 {
    frames
        .iter()
        .map(|frame| frame.image.as_raw().len() as u64)
        .fold(0_u64, |total, bytes| total.saturating_add(bytes))
}

pub fn enforce_load_limit(frames: Vec<AnimFrame>) -> Arc<[AnimFrame]> {
    trim_frames(frames, ANIMATION_LIMIT)
}

pub fn trim_frames(frames: Vec<AnimFrame>, limit: u64) -> Arc<[AnimFrame]> {
    if frames.len() <= 1 {
        return Arc::from(frames);
    }
    let mut total = 0_u64;
    for frame in &frames {
        let bytes = frame.image.as_raw().len() as u64;
        if total.saturating_add(bytes) > limit {
            tracing::warn!(
                frames = frames.len(),
                "animation exceeds 256 MiB; showing the first frame as a still"
            );
            return Arc::from(vec![frames[0].clone()]);
        }
        total += bytes;
    }
    Arc::from(frames)
}

pub fn commit_frames(document: &mut ImageDocument, frames: Vec<AnimFrame>) -> Result<(), DocError> {
    if frame_bytes(&frames) > ANIMATION_LIMIT {
        tracing::warn!("transform would exceed 256 MiB; the image was left unchanged");
        return Err(DocError::new(TransformError::TooLarge.to_string()));
    }
    let previous = Arc::clone(&document.frames);
    let next: Arc<[AnimFrame]> = Arc::from(frames);
    document.push_undo(previous);
    document.frames = next;
    document.refresh_size();
    Ok(())
}

pub fn apply_transform_all(
    frames: &[AnimFrame],
    id: TransformId,
    params: &Params,
) -> Result<Vec<AnimFrame>, TransformError> {
    let transform = transforms::find_transform(id).ok_or(TransformError::BadParams)?;
    let mut next = Vec::with_capacity(frames.len());
    for frame in frames {
        let image = transform.apply(&frame.image, params)?;
        next.push(AnimFrame {
            image: Arc::new(image),
            delay: frame.delay,
        });
    }
    Ok(next)
}

pub fn crop_all(frames: &[AnimFrame], rect: PixelRect) -> Result<Vec<AnimFrame>, DocError> {
    let Some(first) = frames.first() else {
        return Err(DocError::new("there is no image to crop"));
    };
    let Some(rect) = clamp_rect(rect, first.image.width(), first.image.height()) else {
        return Err(DocError::new("the selection is empty"));
    };
    let mut next = Vec::with_capacity(frames.len());
    for frame in frames {
        let cropped = image::imageops::crop_imm(
            frame.image.as_ref(),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        )
        .to_image();
        next.push(AnimFrame {
            image: Arc::new(cropped),
            delay: frame.delay,
        });
    }
    Ok(next)
}

pub fn save_path(source: &Path, animated: bool, stamp: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image");
    let ext = if animated { "gif" } else { "png" };
    let dir = source.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = dir.join(format!("{stem} - Modified {stamp}.{ext}"));
    if !candidate.exists() {
        return candidate;
    }
    for index in 2..10_000 {
        candidate = dir.join(format!("{stem} - Modified {stamp} {index}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    candidate
}

pub fn local_stamp(time: chrono::DateTime<chrono::Local>) -> String {
    time.format("%y-%m-%d %H.%M.%S").to_string()
}

pub fn save_document(document: &ImageDocument, stamp: &str) -> Result<PathBuf, DocError> {
    let animated = document.is_animation();
    let path = save_path(&document.path, animated, stamp);
    if animated {
        save_gif(&path, &document.frames, &document.loop_count)?;
    } else {
        let Some(frame) = document.frames.first() else {
            return Err(DocError::new("there is no image to save"));
        };
        save_png(&path, &frame.image)?;
    }
    Ok(path)
}

pub fn display_delay_gif(stored_centiseconds: u16) -> Duration {
    if stored_centiseconds <= 1 {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(u64::from(stored_centiseconds) * 10)
    }
}

pub fn display_delay_webp(milliseconds: u32) -> Duration {
    if milliseconds == 0 {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(u64::from(milliseconds))
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GifDispose {
    Keep,
    Background,
    Previous,
}

#[cfg(test)]
pub struct RawFrame {
    pub image: RgbaImage,
    pub left: u32,
    pub top: u32,
    pub delay: Duration,
    pub dispose: GifDispose,
}

#[cfg(test)]
pub fn composite_gif(width: u32, height: u32, frames: &[RawFrame]) -> Vec<AnimFrame> {
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    let mut shown = Vec::new();
    for frame in frames {
        let before = canvas.clone();
        blit(&mut canvas, &frame.image, frame.left, frame.top, true);
        shown.push(AnimFrame {
            image: Arc::new(canvas.clone()),
            delay: frame.delay,
        });
        match frame.dispose {
            GifDispose::Keep => {}
            GifDispose::Background => {
                clear_rect(
                    &mut canvas,
                    frame.left,
                    frame.top,
                    frame.image.width(),
                    frame.image.height(),
                );
            }
            GifDispose::Previous => canvas = before,
        }
    }
    shown
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebpFrameInfo {
    pub blend: bool,
    pub dispose_background: bool,
}

pub fn composite_webp(
    width: u32,
    height: u32,
    frames: &[(RgbaImage, u32, u32, Duration, WebpFrameInfo)],
) -> Vec<AnimFrame> {
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    let mut shown = Vec::new();
    for (image, left, top, delay, info) in frames {
        blit(&mut canvas, image, *left, *top, info.blend);
        shown.push(AnimFrame {
            image: Arc::new(canvas.clone()),
            delay: *delay,
        });
        if info.dispose_background {
            clear_rect(&mut canvas, *left, *top, image.width(), image.height());
        }
    }
    shown
}

pub fn animation_duration(frames: &[AnimFrame]) -> f64 {
    frames.iter().map(|frame| frame.delay.as_secs_f64()).sum()
}

pub fn frame_start(frames: &[AnimFrame], index: usize) -> f64 {
    frames
        .iter()
        .take(index)
        .map(|frame| frame.delay.as_secs_f64())
        .sum()
}

pub fn frame_at_time(frames: &[AnimFrame], time: f64) -> usize {
    if frames.is_empty() {
        return 0;
    }
    let time = if time.is_finite() { time } else { 0.0 };
    let mut elapsed = 0.0;
    for (index, frame) in frames.iter().enumerate() {
        let delay = frame.delay.as_secs_f64();
        if time < elapsed + delay || index + 1 == frames.len() {
            return index;
        }
        elapsed += delay;
    }
    frames.len() - 1
}

pub fn advance_frame(
    index: usize,
    frame_count: usize,
    loops_left: &mut Option<u16>,
) -> Option<usize> {
    if frame_count == 0 {
        return None;
    }
    if index + 1 < frame_count {
        return Some(index + 1);
    }
    match loops_left {
        None => Some(0),
        Some(left) if *left > 1 => {
            *left -= 1;
            Some(0)
        }
        Some(_) => None,
    }
}

pub fn loops_remaining(loop_count: &Loop) -> Option<u16> {
    match loop_count {
        Loop::Forever => None,
        Loop::Once => Some(1),
        Loop::Count(count) => Some((*count).max(1)),
    }
}

fn load_gif(path: &Path, bytes: &[u8]) -> Result<Document, DocError> {
    let loop_count = gif_loop(bytes);
    let decoder =
        GifDecoder::new(Cursor::new(bytes)).map_err(|error| DocError::new(error.to_string()))?;
    let (width, height) = decoder.dimensions();
    let frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|error| DocError::new(error.to_string()))?;
    if frames.is_empty() {
        return Err(DocError::new("the GIF has no frames"));
    }
    let frames = frames
        .into_iter()
        .map(|frame| {
            let (numer, denom) = frame.delay().numer_denom_ms();
            let millis = numer.checked_div(denom).unwrap_or(0);
            let centiseconds = millis / 10;
            AnimFrame {
                image: Arc::new(frame.into_buffer()),
                delay: display_delay_gif(centiseconds.min(u16::MAX as u32) as u16),
            }
        })
        .collect::<Vec<_>>();
    Ok(Document::Image(image_document(
        path,
        meta(
            "GIF",
            "GIF (Graphics Interchange Format)",
            "P",
            width,
            height,
        ),
        frames,
        loop_count,
    )))
}

fn load_webp(path: &Path, bytes: &[u8]) -> Result<Document, DocError> {
    let decoder =
        WebPDecoder::new(Cursor::new(bytes)).map_err(|error| DocError::new(error.to_string()))?;
    if decoder.is_animated() {
        return load_animated_webp(path, bytes, &decoder);
    }
    let image = decode_webp_rgba(decoder)?;
    let mode = if image.pixels().any(|pixel| pixel[3] < 255) {
        "RGBA"
    } else {
        "RGB"
    };
    let width = image.width();
    let height = image.height();
    Ok(Document::Image(image_document(
        path,
        meta("WebP", "WebP", mode, width, height),
        vec![AnimFrame {
            image: Arc::new(image),
            delay: Duration::ZERO,
        }],
        Loop::Once,
    )))
}

fn load_animated_webp(
    path: &Path,
    bytes: &[u8],
    decoder: &WebPDecoder<Cursor<&[u8]>>,
) -> Result<Document, DocError> {
    let (width, height) = decoder.dimensions();
    let loop_count = match decoder.loop_count() {
        LoopCount::Forever => Loop::Forever,
        LoopCount::Times(count) => Loop::Count(count.get()),
    };
    let raw = parse_webp_frames(bytes)?;
    let mut prepared = Vec::new();
    for frame in raw {
        let image = decode_still_webp(&frame.payload)?;
        prepared.push((
            image,
            frame.x,
            frame.y,
            display_delay_webp(frame.duration_ms),
            WebpFrameInfo {
                blend: frame.blend,
                dispose_background: frame.dispose_background,
            },
        ));
    }
    if prepared.is_empty() {
        return Err(DocError::new("the WebP animation has no frames"));
    }
    let frames = composite_webp(width, height, &prepared);
    let mode = if frames
        .iter()
        .any(|frame| frame.image.pixels().any(|pixel| pixel[3] < 255))
    {
        "RGBA"
    } else {
        "RGB"
    };
    Ok(Document::Image(image_document(
        path,
        meta("WebP", "WebP", mode, width, height),
        frames,
        loop_count,
    )))
}

fn load_still(path: &Path, bytes: &[u8]) -> Result<Document, DocError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| DocError::new(error.to_string()))?;
    reader.limits(image::Limits::default());
    let format = reader.format();
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| DocError::new(error.to_string()))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let color = decoder.original_color_type();
    let image =
        DynamicImage::from_decoder(decoder).map_err(|error| DocError::new(error.to_string()))?;
    let image = orient(image.to_rgba8(), orientation);
    let (name, description) = format_names(format);
    let mode = if png_indexed(bytes) {
        "P".to_string()
    } else {
        mode_name(color)
    };
    let width = image.width();
    let height = image.height();
    Ok(Document::Image(image_document(
        path,
        meta(name, description, &mode, width, height),
        vec![AnimFrame {
            image: Arc::new(image),
            delay: Duration::ZERO,
        }],
        Loop::Once,
    )))
}

fn meta(
    format_name: &str,
    description: &str,
    mode: &str,
    width: u32,
    height: u32,
) -> ImageMetadata {
    ImageMetadata {
        format_name: format_name.to_string(),
        description: description.to_string(),
        width,
        height,
        mode: mode.to_string(),
    }
}

fn image_document(
    path: &Path,
    metadata: ImageMetadata,
    frames: Vec<AnimFrame>,
    loop_count: Loop,
) -> ImageDocument {
    let frames: Arc<[AnimFrame]> = Arc::from(frames);
    ImageDocument {
        path: path.to_path_buf(),
        metadata,
        frames: Arc::clone(&frames),
        loop_count,
        undo: Vec::new(),
        clean: frames,
    }
}

fn format_names(format: Option<ImageFormat>) -> (&'static str, &'static str) {
    match format {
        Some(ImageFormat::Jpeg) => ("JPEG", "JPEG (ISO 10918)"),
        Some(ImageFormat::Png) => ("PNG", "Portable network graphics"),
        Some(ImageFormat::Gif) => ("GIF", "GIF (Graphics Interchange Format)"),
        Some(ImageFormat::WebP) => ("WebP", "WebP"),
        _ => ("Image", "Image"),
    }
}

fn mode_name(color: ExtendedColorType) -> String {
    match color {
        ExtendedColorType::L1
        | ExtendedColorType::L2
        | ExtendedColorType::L4
        | ExtendedColorType::L8
        | ExtendedColorType::L16 => "L".into(),
        ExtendedColorType::La1
        | ExtendedColorType::La2
        | ExtendedColorType::La4
        | ExtendedColorType::La8
        | ExtendedColorType::La16 => "LA".into(),
        ExtendedColorType::Rgb8
        | ExtendedColorType::Rgb16
        | ExtendedColorType::Rgb32F
        | ExtendedColorType::Bgr8 => "RGB".into(),
        ExtendedColorType::Rgba8
        | ExtendedColorType::Rgba16
        | ExtendedColorType::Rgba32F
        | ExtendedColorType::Bgra8 => "RGBA".into(),
        ExtendedColorType::Cmyk8 => "CMYK".into(),
        _ => "Unknown".into(),
    }
}

fn png_indexed(bytes: &[u8]) -> bool {
    bytes.len() > 25 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes[25] == 3
}

fn orient(image: RgbaImage, orientation: image::metadata::Orientation) -> RgbaImage {
    use image::imageops;
    use image::metadata::Orientation::*;
    match orientation {
        NoTransforms => image,
        Rotate90 => imageops::rotate90(&image),
        Rotate180 => imageops::rotate180(&image),
        Rotate270 => imageops::rotate270(&image),
        FlipHorizontal => imageops::flip_horizontal(&image),
        FlipVertical => imageops::flip_vertical(&image),
        Rotate90FlipH => imageops::flip_horizontal(&imageops::rotate90(&image)),
        Rotate270FlipH => imageops::flip_horizontal(&imageops::rotate270(&image)),
    }
}

fn gif_loop(bytes: &[u8]) -> Loop {
    let marker = b"NETSCAPE2.0";
    let Some(pos) = find_slice(bytes, marker) else {
        return Loop::Once;
    };
    let index = pos + marker.len();
    if bytes.get(index) != Some(&3) || bytes.get(index + 1) != Some(&1) {
        return Loop::Once;
    }
    let lo = u16::from(bytes.get(index + 2).copied().unwrap_or(0));
    let hi = u16::from(bytes.get(index + 3).copied().unwrap_or(0));
    let count = lo | (hi << 8);
    if count == 0 {
        Loop::Forever
    } else {
        Loop::Count(count)
    }
}

fn find_slice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

struct WebpRawFrame {
    x: u32,
    y: u32,
    duration_ms: u32,
    blend: bool,
    dispose_background: bool,
    payload: Vec<u8>,
}

fn parse_webp_frames(bytes: &[u8]) -> Result<Vec<WebpRawFrame>, DocError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(DocError::new("this is not a WebP file"));
    }
    let mut offset = 12;
    let mut frames = Vec::new();
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size =
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap_or([0; 4])) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size).min(bytes.len());
        if id == b"ANMF" && end - start >= 16 {
            let payload = &bytes[start..end];
            let x = read_u24(payload, 0) * 2;
            let y = read_u24(payload, 3) * 2;
            let duration_ms = read_u24(payload, 12);
            let flags = payload[15];
            let blend = flags & 0b0000_0010 == 0;
            let dispose_background = flags & 0b0000_0001 != 0;
            let chunk = &payload[16..];
            frames.push(WebpRawFrame {
                x,
                y,
                duration_ms,
                blend,
                dispose_background,
                payload: still_webp(chunk),
            });
        }
        offset = end + (size % 2);
    }
    Ok(frames)
}

fn read_u24(bytes: &[u8], offset: usize) -> u32 {
    let b0 = u32::from(bytes.get(offset).copied().unwrap_or(0));
    let b1 = u32::from(bytes.get(offset + 1).copied().unwrap_or(0));
    let b2 = u32::from(bytes.get(offset + 2).copied().unwrap_or(0));
    b0 | (b1 << 8) | (b2 << 16)
}

fn still_webp(chunk: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + chunk.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((4 + chunk.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(chunk);
    out
}

fn decode_still_webp(bytes: &[u8]) -> Result<RgbaImage, DocError> {
    let decoder =
        WebPDecoder::new(Cursor::new(bytes)).map_err(|error| DocError::new(error.to_string()))?;
    decode_webp_rgba(decoder)
}

fn decode_webp_rgba<R: std::io::BufRead + std::io::Seek>(
    mut decoder: WebPDecoder<R>,
) -> Result<RgbaImage, DocError> {
    let (width, height) = decoder.dimensions();
    let len = decoder
        .output_buffer_size()
        .ok_or_else(|| DocError::new("the WebP frame is empty"))?;
    let mut buffer = vec![0_u8; len];
    decoder
        .read_image(&mut buffer)
        .map_err(|error| DocError::new(error.to_string()))?;
    let rgba = if decoder.has_alpha() {
        buffer
    } else {
        let mut expanded = Vec::with_capacity(width as usize * height as usize * 4);
        for pixel in buffer.chunks_exact(3) {
            expanded.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
        expanded
    };
    RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| DocError::new("could not build the WebP frame"))
}

fn blit(canvas: &mut RgbaImage, frame: &RgbaImage, left: u32, top: u32, blend: bool) {
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            let dx = left + x;
            let dy = top + y;
            if dx >= canvas.width() || dy >= canvas.height() {
                continue;
            }
            let source = frame.get_pixel(x, y).0;
            if blend {
                if source[3] == 0 {
                    continue;
                }
                let dest = canvas.get_pixel(dx, dy).0;
                let src_a = source[3] as f32 / 255.0;
                let dst_a = dest[3] as f32 / 255.0;
                let out_a = src_a + dst_a * (1.0 - src_a);
                let mut next = [0_u8; 4];
                if out_a > 0.0 {
                    for channel in 0..3 {
                        let value = (source[channel] as f32 * src_a
                            + dest[channel] as f32 * dst_a * (1.0 - src_a))
                            / out_a;
                        next[channel] = transforms::round_channel(value);
                    }
                }
                next[3] = transforms::round_channel(out_a * 255.0);
                canvas.put_pixel(dx, dy, image::Rgba(next));
            } else {
                canvas.put_pixel(dx, dy, image::Rgba(source));
            }
        }
    }
}

fn clear_rect(canvas: &mut RgbaImage, left: u32, top: u32, width: u32, height: u32) {
    for y in top..top.saturating_add(height).min(canvas.height()) {
        for x in left..left.saturating_add(width).min(canvas.width()) {
            canvas.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        }
    }
}

fn save_png(path: &Path, image: &RgbaImage) -> Result<(), DocError> {
    let file = File::create(path).map_err(|error| DocError::new(error.to_string()))?;
    let encoder = PngEncoder::new(file);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| DocError::new(error.to_string()))
}

fn save_gif(path: &Path, frames: &[AnimFrame], loop_count: &Loop) -> Result<(), DocError> {
    if frames.is_empty() {
        return Err(DocError::new("there is no animation to save"));
    }
    let file = File::create(path).map_err(|error| DocError::new(error.to_string()))?;
    let mut encoder = GifEncoder::new(file);
    match loop_count {
        Loop::Forever => encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|error| DocError::new(error.to_string()))?,
        Loop::Count(count) => encoder
            .set_repeat(Repeat::Finite(*count))
            .map_err(|error| DocError::new(error.to_string()))?,
        Loop::Once => {}
    }
    for frame in frames {
        let delay = image::Delay::from_saturating_duration(frame.delay);
        let image_frame = image::Frame::from_parts((*frame.image).clone(), 0, 0, delay);
        encoder
            .encode_frame(image_frame)
            .map_err(|error| DocError::new(error.to_string()))?;
    }
    Ok(())
}

pub fn encode_for_view(
    image: &RgbaImage,
    preview: bool,
) -> Result<(Vec<u8>, &'static str), DocError> {
    if image.pixels().any(|pixel| pixel[3] < 255) {
        let mut cursor = Cursor::new(Vec::new());
        PngEncoder::new(&mut cursor)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|error| DocError::new(error.to_string()))?;
        return Ok((cursor.into_inner(), "image/png"));
    }
    let rgb = DynamicImage::ImageRgba8(image.clone()).into_rgb8();
    let mut cursor = Cursor::new(Vec::new());
    let quality = if preview { 80 } else { 90 };
    let encoder = JpegEncoder::new_with_quality(&mut cursor, quality);
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|error| DocError::new(error.to_string()))?;
    Ok((cursor.into_inner(), "image/jpeg"))
}

pub fn info_lines(document: &ImageDocument) -> Vec<(String, String)> {
    let mut lines = vec![
        ("Format".into(), document.metadata.format_name.clone()),
        (
            "Format Description".into(),
            document.metadata.description.clone(),
        ),
        ("Width".into(), document.metadata.width.to_string()),
        ("Height".into(), document.metadata.height.to_string()),
        ("Mode".into(), document.metadata.mode.clone()),
    ];
    if document.is_animation() {
        let duration: f64 = document
            .frames
            .iter()
            .map(|frame| frame.delay.as_secs_f64())
            .sum();
        let loop_label = match document.loop_count {
            Loop::Once => "Once".to_string(),
            Loop::Forever => "Forever".to_string(),
            Loop::Count(count) => count.to_string(),
        };
        lines.push(("Frame Count".into(), document.frames.len().to_string()));
        lines.push(("Loop".into(), loop_label));
        lines.push(("Duration".into(), format!("{duration:.1}")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn delays_and_loops_follow_the_browser_rule() {
        assert_eq!(display_delay_gif(0), Duration::from_millis(100));
        assert_eq!(display_delay_gif(1), Duration::from_millis(100));
        assert_eq!(display_delay_gif(2), Duration::from_millis(20));
        assert_eq!(display_delay_webp(0), Duration::from_millis(100));
        assert_eq!(display_delay_webp(40), Duration::from_millis(40));
        assert_eq!(loops_remaining(&Loop::Once), Some(1));
        assert_eq!(loops_remaining(&Loop::Forever), None);
        assert_eq!(loops_remaining(&Loop::Count(3)), Some(3));
        let mut left = Some(2_u16);
        assert_eq!(advance_frame(0, 2, &mut left), Some(1));
        assert_eq!(advance_frame(1, 2, &mut left), Some(0));
        assert_eq!(left, Some(1));
        assert_eq!(advance_frame(1, 2, &mut left), None);
    }

    #[test]
    fn gif_background_and_previous_disposal() {
        let red = RgbaImage::from_pixel(2, 1, Rgba([255, 0, 0, 255]));
        let green = RgbaImage::from_pixel(1, 1, Rgba([0, 255, 0, 255]));
        let clear = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));
        let frames = composite_gif(
            2,
            1,
            &[
                RawFrame {
                    image: red,
                    left: 0,
                    top: 0,
                    delay: Duration::from_millis(100),
                    dispose: GifDispose::Keep,
                },
                RawFrame {
                    image: green,
                    left: 0,
                    top: 0,
                    delay: Duration::from_millis(100),
                    dispose: GifDispose::Previous,
                },
                RawFrame {
                    image: clear,
                    left: 0,
                    top: 0,
                    delay: Duration::from_millis(100),
                    dispose: GifDispose::Background,
                },
            ],
        );
        assert_eq!(frames[0].image.get_pixel(1, 0).0, [255, 0, 0, 255]);
        assert_eq!(frames[1].image.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(frames[1].image.get_pixel(1, 0).0, [255, 0, 0, 255]);
        assert_eq!(frames[2].image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }

    #[test]
    fn webp_blend_and_dispose_to_transparent() {
        let red = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let blue = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 255, 255]));
        let frames = composite_webp(
            2,
            1,
            &[
                (
                    red,
                    0,
                    0,
                    Duration::from_millis(100),
                    WebpFrameInfo {
                        blend: true,
                        dispose_background: false,
                    },
                ),
                (
                    blue,
                    0,
                    0,
                    Duration::ZERO,
                    WebpFrameInfo {
                        blend: false,
                        dispose_background: true,
                    },
                ),
            ],
        );
        assert_eq!(frames[1].image.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(display_delay_webp(0), Duration::from_millis(100));
    }

    #[test]
    fn an_edit_changes_every_frame_and_the_limit_keeps_a_still() {
        let frames = vec![
            AnimFrame {
                image: Arc::new(RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]))),
                delay: Duration::from_millis(100),
            },
            AnimFrame {
                image: Arc::new(RgbaImage::from_pixel(2, 2, Rgba([40, 50, 60, 255]))),
                delay: Duration::from_millis(100),
            },
        ];
        let edited = apply_transform_all(
            &frames,
            TransformId::new("brightness"),
            &Params::Factor(0.5),
        )
        .ok()
        .unwrap_or_default();
        assert_eq!(edited.len(), 2);
        assert_eq!(edited[0].delay, Duration::from_millis(100));
        assert_ne!(edited[0].image.get_pixel(0, 0).0[0], 10);
        assert_ne!(edited[1].image.get_pixel(0, 0).0[0], 40);

        let oversized = trim_frames(
            vec![
                AnimFrame {
                    image: Arc::new(
                        RgbaImage::from_raw(2, 2, vec![0; 16])
                            .unwrap_or_else(|| RgbaImage::new(1, 1)),
                    ),
                    delay: Duration::ZERO,
                },
                AnimFrame {
                    image: Arc::new(RgbaImage::new(1, 1)),
                    delay: Duration::ZERO,
                },
            ],
            16,
        );
        assert_eq!(oversized.len(), 1);
        assert_eq!(ANIMATION_LIMIT, 256 * 1024 * 1024);
        assert!(frame_bytes(&frames) < ANIMATION_LIMIT);
    }

    #[test]
    fn save_name_uses_the_local_stamp() {
        let source = Path::new("/tmp/Holiday.jpg");
        let still = save_path(source, false, "26-10-18 14.03.09");
        assert_eq!(
            still.file_name().and_then(|name| name.to_str()),
            Some("Holiday - Modified 26-10-18 14.03.09.png")
        );
        let animation = save_path(source, true, "26-10-18 14.03.09");
        assert_eq!(
            animation.file_name().and_then(|name| name.to_str()),
            Some("Holiday - Modified 26-10-18 14.03.09.gif")
        );
    }

    #[test]
    fn undo_cap_and_dirty_follow_the_saved_list() {
        let frames: Arc<[AnimFrame]> = Arc::from(vec![AnimFrame {
            image: Arc::new(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255]))),
            delay: Duration::ZERO,
        }]);
        let mut document = ImageDocument {
            path: PathBuf::from("a.png"),
            metadata: ImageMetadata {
                format_name: "PNG".into(),
                description: "Portable network graphics".into(),
                width: 1,
                height: 1,
                mode: "RGB".into(),
            },
            frames: Arc::clone(&frames),
            loop_count: Loop::Once,
            undo: Vec::new(),
            clean: Arc::clone(&frames),
        };
        assert!(!document.is_dirty());
        for index in 0..20 {
            let next = vec![AnimFrame {
                image: Arc::new(RgbaImage::from_pixel(1, 1, Rgba([index, 0, 0, 255]))),
                delay: Duration::ZERO,
            }];
            commit_frames(&mut document, next).ok();
        }
        assert_eq!(document.undo.len(), 16);
        assert!(document.is_dirty());
        document.clean = Arc::clone(&document.frames);
        assert!(!document.is_dirty());
        assert!(document.undo());
        assert!(document.is_dirty());
        while document.undo() {}
        assert!(document.undo.is_empty());
        assert_eq!(document.frames[0].image.get_pixel(0, 0).0[0], 3);
        assert!(document.is_dirty());
    }
}
