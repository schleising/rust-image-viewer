use crate::catalog::pictures_or_home;
use crate::wake::{self, Note};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppleKind {
    Oapp,
    Odoc,
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupPlan {
    Open(PathBuf),
    WaitForDocument,
    Defer,
}

static SAW_OPENED: AtomicBool = AtomicBool::new(false);
static READ_ONCE: AtomicBool = AtomicBool::new(false);

pub fn plan_startup(
    kind: AppleKind,
    argv: Option<PathBuf>,
    pictures: PathBuf,
    home: PathBuf,
) -> StartupPlan {
    match kind {
        AppleKind::Oapp => StartupPlan::Open(startup_path(argv, &pictures, &home)),
        AppleKind::Odoc => StartupPlan::WaitForDocument,
        AppleKind::Unreadable => StartupPlan::Defer,
    }
}

pub fn plan_deferred(
    opened_already: bool,
    argv: Option<PathBuf>,
    fallback: PathBuf,
) -> Option<PathBuf> {
    if opened_already {
        None
    } else {
        Some(argv.unwrap_or(fallback))
    }
}

pub fn startup_path(argv: Option<PathBuf>, pictures: &Path, home: &Path) -> PathBuf {
    if let Some(path) = argv {
        return path;
    }
    pictures_or_home(home, pictures.is_dir())
}

pub fn argv_path() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next()?;
    if args.next().is_some() {
        return None;
    }
    Some(PathBuf::from(first))
}

pub fn on_tao_event<T>(event: &dioxus::desktop::tao::event::Event<'_, T>) {
    if let dioxus::desktop::tao::event::Event::Opened { urls } = event {
        SAW_OPENED.store(true, Ordering::SeqCst);
        if let Some(url) = urls.first() {
            let path = if url.scheme() == "file" {
                url.to_file_path().ok()
            } else {
                Some(PathBuf::from(url.path()))
            };
            if let Some(path) = path {
                wake::post(Note::Opened(path));
            }
        }
    }
    if READ_ONCE.swap(true, Ordering::SeqCst) {
        return;
    }
    let home = home_dir();
    let pictures = home.join("Pictures");
    match plan_startup(read_apple_kind(), argv_path(), pictures, home) {
        StartupPlan::Open(path) => {
            if !SAW_OPENED.load(Ordering::SeqCst) {
                wake::post(Note::Startup(path));
            }
        }
        StartupPlan::WaitForDocument => {}
        StartupPlan::Defer => schedule_deferred(),
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(target_os = "macos")]
fn read_apple_kind() -> AppleKind {
    unsafe { apple_kind() }
}

#[cfg(not(target_os = "macos"))]
fn read_apple_kind() -> AppleKind {
    AppleKind::Oapp
}

#[cfg(target_os = "macos")]
unsafe fn apple_kind() -> AppleKind {
    use objc2::runtime::{AnyClass, AnyObject};
    let Some(class) = AnyClass::get(c"NSAppleEventManager") else {
        return AppleKind::Unreadable;
    };
    let manager: *mut AnyObject = objc2::msg_send![class, sharedAppleEventManager];
    if manager.is_null() {
        return AppleKind::Unreadable;
    }
    let event: *mut AnyObject = objc2::msg_send![manager, currentAppleEvent];
    if event.is_null() {
        return AppleKind::Unreadable;
    }
    let class_code: u32 = objc2::msg_send![event, eventClass];
    let event_id: u32 = objc2::msg_send![event, eventID];
    if class_code != 0x6165_7674 {
        return AppleKind::Unreadable;
    }
    match event_id {
        0x6f61_7070 => AppleKind::Oapp,
        0x6f64_6f63 => AppleKind::Odoc,
        _ => AppleKind::Unreadable,
    }
}

fn schedule_deferred() {
    #[cfg(target_os = "macos")]
    dispatch2::DispatchQueue::main().exec_async(deferred_open);
    #[cfg(not(target_os = "macos"))]
    deferred_open();
}

fn deferred_open() {
    let home = home_dir();
    let fallback = pictures_or_home(&home, home.join("Pictures").is_dir());
    if let Some(path) = plan_deferred(SAW_OPENED.load(Ordering::SeqCst), argv_path(), fallback) {
        wake::post(Note::Startup(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oapp_uses_the_argument_or_pictures_and_odoc_waits() {
        let home = std::env::temp_dir().join(format!("image-viewer-home-{}", std::process::id()));
        let pictures = home.join("Pictures");
        let _ = std::fs::create_dir_all(&pictures);
        let argv = Some(PathBuf::from("/tmp/one.png"));
        assert_eq!(
            plan_startup(
                AppleKind::Oapp,
                argv.clone(),
                pictures.clone(),
                home.clone()
            ),
            StartupPlan::Open(PathBuf::from("/tmp/one.png"))
        );
        assert_eq!(
            plan_startup(AppleKind::Oapp, None, pictures.clone(), home.clone()),
            StartupPlan::Open(pictures.clone())
        );
        let missing = home.join("NoSuchPictures");
        assert_eq!(
            plan_startup(AppleKind::Oapp, None, missing, home.clone()),
            StartupPlan::Open(home.clone())
        );
        assert_eq!(
            plan_startup(AppleKind::Odoc, argv.clone(), pictures.clone(), home),
            StartupPlan::WaitForDocument
        );
        assert_eq!(plan_deferred(true, argv.clone(), pictures.clone()), None);
        assert_eq!(
            plan_deferred(false, argv, pictures.clone()),
            Some(PathBuf::from("/tmp/one.png"))
        );
        assert_eq!(plan_deferred(false, None, pictures.clone()), Some(pictures));
    }
}
