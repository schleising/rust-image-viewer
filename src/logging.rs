use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
pub fn init() {
    let path = log_path();
    match open_log(&path) {
        Some(writer) => {
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_max_level(tracing::Level::INFO)
                .with_target(false)
                .with_writer(writer)
                .init();
        }
        None => {
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_max_level(tracing::Level::INFO)
                .with_target(false)
                .with_writer(io::stderr)
                .init();
            tracing::warn!("could not create {}; logging to stderr", path.display());
        }
    }
}

fn log_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Library/Logs/ImageViewer/ImageViewer.log")
}

fn open_log(
    path: &std::path::Path,
) -> Option<impl for<'a> tracing_subscriber::fmt::MakeWriter<'a> + Send + Sync + 'static> {
    let parent = path.parent()?;
    std::fs::create_dir_all(parent).ok()?;
    let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        FileRotate::new(
            path,
            AppendCount::new(4),
            ContentLimit::Bytes(1024 * 1024),
            Compression::None,
            None,
        )
    }));
    let Ok(rotate) = opened else {
        return None;
    };
    Some(Mutex::new(rotate))
}
