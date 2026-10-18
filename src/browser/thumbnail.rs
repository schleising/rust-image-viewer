use crate::catalog::{EntryKind, FolderView};
use crate::document::{self, Document};
use crate::protocol;
use crate::wake;
use fast_image_resize::images::Image;
use fast_image_resize::{PixelType, Resizer};
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;

struct Job {
    generation: u64,
    index: usize,
    path: PathBuf,
    px: u32,
    video: bool,
}

static CURRENT: AtomicU64 = AtomicU64::new(0);
static TX: OnceLock<Sender<Job>> = OnceLock::new();
static QUEUED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<(u64, usize, u32)>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

pub fn ensure(folder: &FolderView, generation: u64, px: u32) {
    if px == 0 {
        return;
    }
    CURRENT.store(generation, Ordering::SeqCst);
    let tx = TX.get_or_init(start_pool);
    let mut queued = match QUEUED.lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner(),
    };
    queued.retain(|item| item.0 == generation);
    for (index, entry) in folder.entries.iter().enumerate() {
        let video = entry.kind == EntryKind::Video;
        if entry.kind != EntryKind::Image && !video {
            continue;
        }
        if !matches!(
            protocol::thumb_status(generation, index, px),
            protocol::ThumbStatus::Missing
        ) {
            continue;
        }
        if !queued.insert((generation, index, px)) {
            continue;
        }
        let _ = tx.send(Job {
            generation,
            index,
            path: entry.path.clone(),
            px,
            video,
        });
    }
}

fn start_pool() -> Sender<Job> {
    let (tx, rx) = mpsc::channel();
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2)
        .clamp(2, 8);
    let shared = std::sync::Arc::new(std::sync::Mutex::new(rx));
    for _ in 0..workers {
        let shared = std::sync::Arc::clone(&shared);
        std::thread::spawn(move || worker(shared));
    }
    tx
}

fn worker(rx: std::sync::Arc<std::sync::Mutex<Receiver<Job>>>) {
    loop {
        let job = {
            let guard = match rx.lock() {
                Ok(guard) => guard,
                Err(poison) => poison.into_inner(),
            };
            guard.recv()
        };
        let Ok(job) = job else {
            return;
        };
        if job.generation != CURRENT.load(Ordering::SeqCst) {
            continue;
        }
        let result = if job.video {
            decode_video_thumb(&job.path, job.px)
        } else {
            decode_thumb(&job.path, job.px)
        };
        if let Err(error) = &result {
            if job.video {
                tracing::warn!(%error, path = %job.path.display(), "could not read a video thumbnail");
            }
        }
        if job.generation != CURRENT.load(Ordering::SeqCst) {
            continue;
        }
        protocol::store_thumb(job.generation, job.index, job.px, result);
        wake::post(wake::Note::Thumbs);
    }
}

fn decode_video_thumb(path: &std::path::Path, px: u32) -> Result<(Vec<u8>, bool), String> {
    let frame = super::video_frame::frame(path, px)?;
    let contained = contain(&frame, px)?;
    Ok((encode_png(&contained)?, true))
}

fn decode_thumb(path: &std::path::Path, px: u32) -> Result<(Vec<u8>, bool), String> {
    let document = document::load(path).map_err(|error| error.message)?;
    let Document::Image(image) = document else {
        return Err("this file is not an image".into());
    };
    let frame = image
        .frames
        .first()
        .ok_or_else(|| "the image has no frames".to_string())?;
    let contained = contain(&frame.image, px)?;
    Ok((encode_png(&contained)?, image.is_animation()))
}

fn contain(image: &RgbaImage, px: u32) -> Result<RgbaImage, String> {
    let width = image.width().max(1);
    let height = image.height().max(1);
    let scale = (px as f64 / width as f64).min(px as f64 / height as f64);
    let dest_w = ((width as f64 * scale).round() as u32).clamp(1, px);
    let dest_h = ((height as f64 * scale).round() as u32).clamp(1, px);
    let resized = resize(image, dest_w, dest_h)?;
    let mut canvas = image::RgbaImage::from_pixel(px, px, image::Rgba([0, 0, 0, 0]));
    let left = (px - dest_w) / 2;
    let top = (px - dest_h) / 2;
    for y in 0..dest_h {
        for x in 0..dest_w {
            canvas.put_pixel(left + x, top + y, *resized.get_pixel(x, y));
        }
    }
    Ok(canvas)
}

fn resize(image: &RgbaImage, width: u32, height: u32) -> Result<RgbaImage, String> {
    let src = Image::from_vec_u8(
        image.width(),
        image.height(),
        image.as_raw().clone(),
        PixelType::U8x4,
    )
    .map_err(|error| error.to_string())?;
    let mut dst = Image::new(width, height, PixelType::U8x4);
    Resizer::new()
        .resize(&src, &mut dst, None)
        .map_err(|error| error.to_string())?;
    RgbaImage::from_raw(width, height, dst.buffer().to_vec())
        .ok_or_else(|| "could not build the thumbnail".to_string())
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    use image::{ExtendedColorType, ImageEncoder};
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::codecs::png::PngEncoder::new(&mut cursor)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(cursor.into_inner())
}
