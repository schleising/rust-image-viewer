use crate::catalog::FolderView;
use crate::document::{AnimFrame, Document};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;

static CLOCK: Notify = Notify::const_new();

pub fn poke_clock() {
    CLOCK.notify_one();
}

pub fn clock_listener() -> tokio::sync::futures::Notified<'static> {
    CLOCK.notified()
}

pub enum Note {
    Opened(PathBuf),
    Startup(PathBuf),
    Folder(Result<FolderView, String>),
    Loaded {
        ticket: u64,
        result: Result<Document, String>,
    },
    Prepared {
        token: u64,
        result: Result<Document, String>,
    },
    Adopt {
        token: u64,
        document: Document,
    },
    Transformed {
        ticket: u64,
        result: Result<Vec<AnimFrame>, String>,
    },
    Preview {
        ticket: u64,
        image: Option<std::sync::Arc<image::RgbaImage>>,
    },
    Saved {
        ticket: u64,
        result: Result<(), String>,
    },
    Thumbs,
    Swipe(i8),
}

static EARLY: Mutex<Vec<Note>> = Mutex::new(Vec::new());
static TX: OnceLock<UnboundedSender<Note>> = OnceLock::new();
static TOKIO: OnceLock<tokio::runtime::Handle> = OnceLock::new();

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub fn post(note: Note) {
    let mut early = lock(&EARLY);
    if let Some(tx) = TX.get() {
        let _ = tx.send(note);
    } else {
        early.push(note);
    }
}

pub fn install() -> UnboundedReceiver<Note> {
    let (tx, rx) = unbounded_channel();
    let mut early = lock(&EARLY);
    let _ = TX.set(tx.clone());
    for note in std::mem::take(&mut *early) {
        let _ = tx.send(note);
    }
    rx
}

pub fn bind_runtime() {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let _ = TOKIO.set(handle);
    }
}

pub fn spawn_blocking(work: impl FnOnce() + Send + 'static) {
    if let Some(handle) = TOKIO.get() {
        handle.spawn_blocking(work);
        return;
    }
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let _ = TOKIO.set(handle.clone());
        handle.spawn_blocking(work);
        return;
    }
    std::thread::spawn(work);
}
