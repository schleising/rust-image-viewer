use crate::catalog::{self, EntryKind, FolderView};
use crate::command::{Availability, Command};
use crate::document::{self, loops_remaining, AnimFrame, Document, ImageDocument, VideoDocument};
use crate::transforms::{Params, TransformId};
use crate::transitions::{TransitionContext, TransitionId, TransitionRegistry};
use crate::viewport::{PixelRect, Viewport};
use crate::wake::Note;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const ZOOM_STEP: f64 = 1.05;
pub const WHEEL_PIXELS: f64 = 50.0;
const CLICK_SLOP: f64 = 3.0;
const DEBOUNCE: Duration = Duration::from_millis(20);
const OPEN_FULLSCREEN_GUARD: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Browser,
    Viewer,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    Info,
    Adjust {
        colour: f32,
        contrast: f32,
        brightness: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Transitioning { from: PathBuf, to: PathBuf },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FolderMemory {
    pub highlighted: usize,
    pub scroll_top: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollAction {
    Restore(f64),
    Reveal,
}

#[derive(Clone, Debug)]
pub enum Effect {
    None,
    Quit,
    ListFolder(PathBuf),
    LoadFile {
        ticket: u64,
        path: PathBuf,
    },
    OpenFile {
        folder: PathBuf,
        ticket: u64,
        path: PathBuf,
    },
    Save {
        ticket: u64,
    },
    Transform {
        ticket: u64,
        id: TransformId,
        params: Params,
    },
    Preview {
        ticket: u64,
        colour: f32,
        contrast: f32,
        brightness: f32,
    },
    CommitAdjust {
        ticket: u64,
        colour: f32,
        contrast: f32,
        brightness: f32,
    },
    Crop {
        ticket: u64,
        rect: PixelRect,
    },
    ToggleFullscreen,
    SyncVideo,
    FocusPage,
}

pub struct AppState {
    pub mode: Mode,
    pub folder: FolderView,
    pub history: HashMap<PathBuf, FolderMemory>,
    pub pending_restore: Option<FolderMemory>,
    pub pending_file: Option<PathBuf>,
    pub document: Option<Document>,
    pub pending_document: Option<Document>,
    pub viewport: Viewport,
    pub preview: Option<Arc<image::RgbaImage>>,
    pub preview_ticket: u64,
    pub busy: bool,
    pub transition: TransitionId,
    pub phase: Phase,
    pub frame_index: usize,
    pub loops_left: Option<u16>,
    pub animation_paused: bool,
    pub animation_done: bool,
    pub anim_token: u64,
    pub dialog: Option<Dialog>,
    pub adjust_base: Option<Arc<[AnimFrame]>>,
    pub message: Option<String>,
    busy_note: Option<String>,
    pub folder_generation: u64,
    pub image_generation: u64,
    pub load_ticket: u64,
    pub work_ticket: u64,
    pub pending_navigation: bool,
    pub view_size: (f64, f64),
    pub scale_factor: f64,
    pub fullscreen: bool,
    pub opened_at: Option<Instant>,
    pub scrub_visible: bool,
    pub scrub_token: u64,
    pub drag: Option<Drag>,
    pub last_command: Option<String>,
    pub last_command_at: Option<Instant>,
    pub thumb_epoch: u64,
    pub keyboard_until: Option<Instant>,
    pub video_epoch: u64,
    pub video_layer: FrameLayer,
    pub video_front: Option<u64>,
    pub video_back: Option<u64>,
    pub video_show_front: bool,
    pub scroll_action: Option<ScrollAction>,
    pub show_front: bool,
    pub front: FrameLayer,
    pub back: FrameLayer,
}

pub struct Drag {
    pub start: (f64, f64),
    pub last: (f64, f64),
    pub control: bool,
    pub origin: (f64, f64),
    pub fitted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameLayer {
    pub url: String,
    pub size: Option<(u32, u32)>,
    pub fitted: bool,
    pub scale: f64,
    pub translation: (f64, f64),
}

impl FrameLayer {
    fn blank() -> Self {
        Self {
            url: String::new(),
            size: None,
            fitted: true,
            scale: 1.0,
            translation: (0.0, 0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shell {
    pub title: String,
    pub fullscreen: bool,
    pub availability: Availability,
    pub folder_generation: u64,
    pub view_size: (f64, f64),
    pub scale_factor: f64,
    pub video_epoch: u64,
    pub image_generation: u64,
    pub frame_index: usize,
    pub preview_ticket: u64,
    pub has_preview: bool,
    pub thumb_epoch: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            mode: Mode::Browser,
            folder: FolderView {
                path: PathBuf::new(),
                entries: Vec::new(),
                highlighted: 0,
                scroll_top: 0.0,
                error: None,
            },
            history: HashMap::new(),
            pending_restore: None,
            pending_file: None,
            document: None,
            pending_document: None,
            viewport: Viewport::fitted(),
            preview: None,
            preview_ticket: 0,
            busy: false,
            transition: TransitionId::new("cut"),
            phase: Phase::Idle,
            frame_index: 0,
            loops_left: None,
            animation_paused: false,
            animation_done: false,
            anim_token: 0,
            dialog: None,
            adjust_base: None,
            message: None,
            busy_note: None,
            folder_generation: 0,
            image_generation: 0,
            load_ticket: 0,
            work_ticket: 0,
            pending_navigation: false,
            view_size: (1024.0, 768.0),
            scale_factor: 1.0,
            fullscreen: false,
            opened_at: None,
            scrub_visible: false,
            scrub_token: 0,
            drag: None,
            last_command: None,
            last_command_at: None,
            thumb_epoch: 0,
            keyboard_until: None,
            video_epoch: 0,
            video_layer: FrameLayer::blank(),
            video_front: None,
            video_back: None,
            video_show_front: false,
            scroll_action: None,
            show_front: true,
            front: FrameLayer::blank(),
            back: FrameLayer::blank(),
        }
    }

    pub fn image_url(&self) -> String {
        let preview = if self.preview.is_some() {
            self.preview_ticket
        } else {
            0
        };
        format!(
            "app://image/g/{}/f/{}/p/{preview}",
            self.image_generation, self.frame_index
        )
    }

    fn desired_frame(&self) -> Option<FrameLayer> {
        if self.video().is_some() {
            return None;
        }
        let size = self.pixel_size()?;
        if size.0 == 0 || size.1 == 0 {
            return None;
        }
        Some(FrameLayer {
            url: self.image_url(),
            size: Some(size),
            fitted: self.viewport.fitted,
            scale: self.viewport.scale,
            translation: self.viewport.translation,
        })
    }

    pub fn display_needs_sync(&self) -> bool {
        let Some(desired) = self.desired_frame() else {
            return false;
        };
        let visible = if self.show_front {
            &self.front
        } else {
            &self.back
        };
        if visible.url.is_empty() || visible.url == desired.url {
            return visible != &desired;
        }
        let hidden = if self.show_front {
            &self.back
        } else {
            &self.front
        };
        hidden != &desired
    }

    pub fn sync_display(&mut self) {
        let Some(desired) = self.desired_frame() else {
            return;
        };
        let visible = if self.show_front {
            &self.front
        } else {
            &self.back
        };
        if visible.url.is_empty() || visible.url == desired.url {
            if visible != &desired {
                if self.show_front {
                    self.front = desired;
                } else {
                    self.back = desired;
                }
            }
            return;
        }
        let hidden = if self.show_front {
            &self.back
        } else {
            &self.front
        };
        if hidden != &desired {
            if self.show_front {
                self.back = desired;
            } else {
                self.front = desired;
            }
        }
    }

    pub fn reveal_would_change(&self, url: &str) -> bool {
        if url.is_empty() || self.image_url() != url {
            return false;
        }
        let showing_front = if self.front.url == url {
            true
        } else if self.back.url == url {
            false
        } else {
            return false;
        };
        showing_front != self.show_front
            || (self.video().is_none() && !self.video_layer.url.is_empty())
    }

    pub fn reveal_frame(&mut self, url: &str) {
        if !self.reveal_would_change(url) {
            return;
        }
        if self.front.url == url {
            self.show_front = true;
        } else if self.back.url == url {
            self.show_front = false;
        }
        if self.video().is_none() {
            self.clear_video_slots();
        }
    }

    fn desired_video_layer(&self) -> Option<FrameLayer> {
        let video = self.video()?;
        let size = (video.size.0 > 0 && video.size.1 > 0).then_some(video.size);
        Some(FrameLayer {
            url: video.token.to_string(),
            size,
            fitted: self.viewport.fitted,
            scale: self.viewport.scale,
            translation: self.viewport.translation,
        })
    }

    pub(crate) fn shown_video_token(&self) -> Option<u64> {
        let token = self.video_layer.url.parse().ok()?;
        let slot = if self.video_show_front {
            self.video_front
        } else {
            self.video_back
        };
        (slot == Some(token)).then_some(token)
    }

    fn hidden_video_token(&self) -> Option<u64> {
        if self.video_show_front {
            self.video_back
        } else {
            self.video_front
        }
    }

    pub(crate) fn video_box(&self, token: u64) -> FrameLayer {
        if self.video_layer.url == token.to_string() {
            return self.video_layer.clone();
        }
        if let Some(desired) = self.desired_video_layer() {
            if desired.url == token.to_string() {
                return desired;
            }
        }
        FrameLayer {
            url: token.to_string(),
            size: None,
            fitted: true,
            scale: 1.0,
            translation: (0.0, 0.0),
        }
    }

    pub fn video_frame_needs_sync(&self) -> bool {
        let Some(desired) = self.desired_video_layer() else {
            return false;
        };
        let Some(token) = desired.url.parse::<u64>().ok() else {
            return false;
        };
        match self.shown_video_token() {
            Some(shown) if shown == token => self.video_layer != desired,
            Some(_) => self.hidden_video_token() != Some(token),
            None => self.video_front != Some(token) && self.video_back != Some(token),
        }
    }

    pub fn retain_video_frame(&mut self) {
        let Some(desired) = self.desired_video_layer() else {
            return;
        };
        let Some(token) = desired.url.parse::<u64>().ok() else {
            return;
        };
        match self.shown_video_token() {
            Some(shown) if shown == token => {
                if self.video_layer != desired {
                    self.video_layer = desired;
                }
            }
            Some(_) => self.park_hidden_video(token),
            None => {
                if self.video_front != Some(token) && self.video_back != Some(token) {
                    if self.video_front.is_none() {
                        self.video_front = Some(token);
                    } else if self.video_back.is_none() {
                        self.video_back = Some(token);
                    } else {
                        self.video_front = Some(token);
                    }
                }
            }
        }
    }

    fn park_hidden_video(&mut self, token: u64) {
        if self.video_show_front {
            if self.video_back != Some(token) {
                self.video_back = Some(token);
            }
        } else if self.video_front != Some(token) {
            self.video_front = Some(token);
        }
    }

    pub fn promote_video_would_change(&self, token: u64) -> bool {
        let Some(desired) = self.desired_video_layer() else {
            return false;
        };
        if desired.url != token.to_string() {
            return false;
        }
        if self.video_front != Some(token) && self.video_back != Some(token) {
            return false;
        }
        self.shown_video_token() != Some(token) || self.video_layer != desired
    }

    pub fn promote_video(&mut self, token: u64) {
        if !self.promote_video_would_change(token) {
            return;
        }
        let Some(desired) = self.desired_video_layer() else {
            return;
        };
        let was_shown = self.shown_video_token() == Some(token);
        self.video_show_front = self.video_front == Some(token);
        self.video_layer = desired;
        if !was_shown {
            self.video_epoch = self.video_epoch.wrapping_add(1);
        }
    }

    fn clear_video_slots(&mut self) {
        self.video_layer = FrameLayer::blank();
        self.video_front = None;
        self.video_back = None;
        self.video_show_front = false;
    }

    pub fn title(&self) -> String {
        if self.folder.path.as_os_str().is_empty() {
            return "Image Viewer".to_string();
        }
        let folder = catalog::folder_stem(&self.folder.path);
        let mut title = if self.mode == Mode::Viewer {
            if let Some(document) = &self.document {
                let stem = document
                    .path()
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("image");
                format!("{folder} - {stem}")
            } else {
                folder
            }
        } else {
            folder
        };
        if self.is_dirty() {
            title.push_str(" *");
        }
        title
    }

    pub fn is_dirty(&self) -> bool {
        matches!(&self.document, Some(Document::Image(image)) if image.is_dirty())
    }

    pub fn shell(&self) -> Shell {
        Shell {
            title: self.title(),
            fullscreen: self.fullscreen,
            availability: self.availability(),
            folder_generation: self.folder_generation,
            view_size: self.view_size,
            scale_factor: self.scale_factor,
            video_epoch: self.video_epoch,
            image_generation: self.image_generation,
            frame_index: self.frame_index,
            preview_ticket: self.preview_ticket,
            has_preview: self.preview.is_some(),
            thumb_epoch: self.thumb_epoch,
        }
    }

    pub fn availability(&self) -> Availability {
        let image = self.image();
        let video = self.video().is_some();
        let viewing = self.mode == Mode::Viewer && self.document.is_some();
        let dialog = self.dialog.is_some();
        Availability {
            viewing,
            image: viewing && image.is_some(),
            animated: viewing && image.is_some_and(ImageDocument::is_animation),
            video: viewing && video,
            dirty: self.is_dirty(),
            selection: self.viewport.selection.is_some(),
            zoomed: viewing && !self.viewport.fitted,
            undo: image.is_some_and(|image| !image.undo.is_empty()),
            busy: self.busy,
            dialog,
            adjusting: matches!(self.dialog, Some(Dialog::Adjust { .. })),
            transitioning: matches!(self.phase, Phase::Transitioning { .. }),
        }
    }

    pub fn image(&self) -> Option<&ImageDocument> {
        match &self.document {
            Some(Document::Image(image)) => Some(image),
            _ => None,
        }
    }

    pub fn image_mut(&mut self) -> Option<&mut ImageDocument> {
        match &mut self.document {
            Some(Document::Image(image)) => Some(image),
            _ => None,
        }
    }

    pub fn video(&self) -> Option<&VideoDocument> {
        match &self.document {
            Some(Document::Video(video)) => Some(video),
            _ => None,
        }
    }

    pub fn pixel_size(&self) -> Option<(u32, u32)> {
        match &self.document {
            Some(Document::Image(image)) => Some((image.metadata.width, image.metadata.height)),
            Some(Document::Video(video)) if video.size.0 > 0 && video.size.1 > 0 => {
                Some(video.size)
            }
            _ => None,
        }
    }

    pub fn thumb_px(&self) -> u32 {
        let css = (self.view_size.0 / 8.0).max(1.0);
        (css * self.scale_factor).round().clamp(1.0, 2048.0) as u32
    }

    pub fn open_fullscreen_guard(&self, now: Instant) -> bool {
        self.opened_at
            .is_some_and(|opened| now.saturating_duration_since(opened) < OPEN_FULLSCREEN_GUARD)
    }

    pub fn dispatch(&mut self, command: Command, now: Instant) -> Effect {
        let key = command.id_key();
        if self.last_command.as_deref() == Some(key.as_str()) {
            if let Some(at) = self.last_command_at {
                if now.saturating_duration_since(at) < DEBOUNCE {
                    return Effect::None;
                }
            }
        }
        if !command.available(&self.availability()) {
            return Effect::None;
        }
        self.last_command = Some(key);
        self.last_command_at = Some(now);
        self.apply(command, now)
    }

    fn apply(&mut self, command: Command, now: Instant) -> Effect {
        match command {
            Command::Open(path) => self.open_path(path),
            Command::ReturnToBrowser => {
                self.return_to_browser();
                Effect::FocusPage
            }
            Command::Next => self.step_media(1),
            Command::Previous => self.step_media(-1),
            Command::Quit => Effect::Quit,
            Command::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                if !self.fullscreen {
                    self.keyboard_until = Some(now + Duration::from_millis(1000));
                }
                Effect::ToggleFullscreen
            }
            Command::OpenAdjust => {
                if let Some(image) = self.image() {
                    self.adjust_base = Some(Arc::clone(&image.frames));
                    self.dialog = Some(Dialog::Adjust {
                        colour: 1.0,
                        contrast: 1.0,
                        brightness: 1.0,
                    });
                    self.preview = None;
                    self.bump_anim();
                }
                Effect::None
            }
            Command::Transform { id, params } => self.start_transform(id, params),
            Command::Preview {
                colour,
                contrast,
                brightness,
            } => {
                if let Some(Dialog::Adjust { .. }) = &mut self.dialog {
                    self.dialog = Some(Dialog::Adjust {
                        colour,
                        contrast,
                        brightness,
                    });
                }
                self.preview_ticket += 1;
                Effect::Preview {
                    ticket: self.preview_ticket,
                    colour,
                    contrast,
                    brightness,
                }
            }
            Command::CommitPreview => self.commit_adjust(),
            Command::CancelPreview => {
                self.dialog = None;
                self.preview = None;
                self.adjust_base = None;
                self.bump_anim();
                Effect::None
            }
            Command::Undo => {
                self.undo();
                Effect::None
            }
            Command::Save => {
                self.work_ticket += 1;
                self.busy = true;
                Effect::Save {
                    ticket: self.work_ticket,
                }
            }
            Command::ZoomToSelection => {
                if let (Some(rect), Some(size)) = (self.viewport.selection, self.pixel_size()) {
                    self.viewport.zoom_to_rect(self.view_size, size, rect);
                }
                Effect::None
            }
            Command::ResetZoom => {
                if let Some(size) = self.pixel_size() {
                    self.viewport.refit(self.view_size, size);
                    self.viewport.selection = None;
                }
                Effect::None
            }
            Command::CropToSelection => {
                let Some(rect) = self.viewport.selection else {
                    return Effect::None;
                };
                self.work_ticket += 1;
                self.busy = true;
                Effect::Crop {
                    ticket: self.work_ticket,
                    rect,
                }
            }
            Command::ShowInfo => {
                self.dialog = Some(Dialog::Info);
                Effect::None
            }
            Command::PlayPause => self.play_pause(),
            Command::Skip { delta_ms } => self.skip(delta_ms),
            Command::NudgeVolume { delta } => self.nudge_volume(delta),
            Command::ToggleMute => self.toggle_mute(),
            Command::Seek { fraction } => self.seek(fraction),
        }
    }

    pub fn open_path(&mut self, path: PathBuf) -> Effect {
        if path.is_dir() {
            return self.leave_for(path);
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/"));
        let needs_folder = self.folder.path != parent || self.folder.entries.is_empty();
        if needs_folder {
            self.pending_file = Some(path.clone());
            self.remember();
        } else {
            self.highlight_path(&path);
        }
        self.mode = Mode::Viewer;
        match self.begin_load(path, false) {
            Effect::LoadFile { ticket, path } if needs_folder => Effect::OpenFile {
                folder: parent,
                ticket,
                path,
            },
            effect => effect,
        }
    }

    fn leave_for(&mut self, path: PathBuf) -> Effect {
        self.remember();
        self.message = None;
        Effect::ListFolder(path)
    }

    fn remember(&mut self) {
        if self.folder.path.as_os_str().is_empty() {
            return;
        }
        self.history.insert(
            self.folder.path.clone(),
            FolderMemory {
                highlighted: self.folder.highlighted,
                scroll_top: self.folder.scroll_top,
            },
        );
    }

    fn begin_load(&mut self, path: PathBuf, navigation: bool) -> Effect {
        if navigation && matches!(self.phase, Phase::Transitioning { .. }) {
            return Effect::None;
        }
        self.load_ticket += 1;
        self.pending_navigation = navigation;
        self.busy = true;
        self.message = None;
        Effect::LoadFile {
            ticket: self.load_ticket,
            path,
        }
    }

    fn step_media(&mut self, delta: isize) -> Effect {
        let paths = catalog::media_paths(&self.folder);
        if paths.is_empty() {
            return Effect::None;
        }
        let current = self.document.as_ref().map(Document::path);
        let index = current
            .and_then(|path| paths.iter().position(|item| item == path))
            .unwrap_or(0);
        let len = paths.len() as isize;
        let next = (index as isize + delta).rem_euclid(len) as usize;
        let Some(path) = paths.get(next).cloned() else {
            return Effect::None;
        };
        self.highlight_path(&path);
        self.begin_load(path, true)
    }

    fn highlight_path(&mut self, path: &Path) {
        if let Some(index) = self
            .folder
            .entries
            .iter()
            .position(|entry| entry.path == path)
        {
            self.folder.highlighted = index;
        }
    }

    pub fn open_media(
        &mut self,
        document: Document,
        from_navigation: bool,
        registry: &TransitionRegistry,
    ) -> bool {
        if from_navigation && matches!(self.phase, Phase::Transitioning { .. }) {
            return false;
        }
        if from_navigation {
            let ctx = TransitionContext {
                in_browser: self.mode == Mode::Browser,
                current_is_video: matches!(self.document, Some(Document::Video(_))),
                next_is_video: matches!(document, Document::Video(_)),
            };
            if let Some(transition) = registry.get(self.transition) {
                if transition.applies(&ctx) {
                    tracing::info!(
                        id = self.transition.as_str(),
                        name = transition.label(),
                        ms = transition.duration().as_millis() as u64,
                        style = transition.style() as u8,
                        "playing a transition"
                    );
                    let from = self
                        .document
                        .as_ref()
                        .map(Document::path)
                        .map(Path::to_path_buf)
                        .unwrap_or_default();
                    let to = document.path().to_path_buf();
                    self.pending_document = Some(document);
                    self.phase = Phase::Transitioning { from, to };
                    return true;
                }
            }
        }
        self.install_document(document);
        true
    }

    fn install_document(&mut self, mut document: Document) {
        if let Document::Video(video) = &mut document {
            self.image_generation += 1;
            video.token = self.image_generation;
        } else {
            self.image_generation += 1;
        }
        let size = match &document {
            Document::Image(image) => Some((image.metadata.width, image.metadata.height)),
            Document::Video(video) if video.size.0 > 0 && video.size.1 > 0 => Some(video.size),
            _ => None,
        };
        self.highlight_path(document.path());
        self.loops_left = match &document {
            Document::Image(image) => loops_remaining(&image.loop_count),
            Document::Video(_) => None,
        };
        self.document = Some(document);
        self.pending_document = None;
        self.mode = Mode::Viewer;
        self.viewport = Viewport::fitted();
        if let Some(size) = size {
            self.viewport.refit(self.view_size, size);
        }
        self.frame_index = 0;
        self.animation_paused = false;
        self.animation_done = false;
        self.preview = None;
        self.adjust_base = None;
        self.dialog = None;
        self.phase = Phase::Idle;
        self.opened_at = Some(Instant::now());
        self.message = None;
        self.scrub_visible = false;
        self.bump_anim();
    }

    fn return_to_browser(&mut self) {
        self.mode = Mode::Browser;
        self.document = None;
        self.pending_document = None;
        self.preview = None;
        self.dialog = None;
        self.adjust_base = None;
        self.viewport = Viewport::fitted();
        self.phase = Phase::Idle;
        self.clear_busy();
        self.show_front = true;
        self.front = FrameLayer::blank();
        self.back = FrameLayer::blank();
        self.clear_video_slots();
        self.bump_anim();
    }

    fn start_transform(&mut self, id: TransformId, params: Params) -> Effect {
        self.work_ticket += 1;
        self.busy = true;
        self.viewport.selection = None;
        if crate::transforms::blocking_transform(id) {
            tracing::info!(transform = id.as_str(), "running a long transform");
            self.announce_busy(busy_notice(id, &params));
        }
        Effect::Transform {
            ticket: self.work_ticket,
            id,
            params,
        }
    }

    fn announce_busy(&mut self, note: String) {
        self.busy_note = Some(note.clone());
        self.message = Some(note);
    }

    fn clear_busy(&mut self) {
        self.busy = false;
        if self.busy_note.is_some() && self.message == self.busy_note {
            self.message = None;
        }
        self.busy_note = None;
    }

    fn commit_adjust(&mut self) -> Effect {
        let Some(Dialog::Adjust {
            colour,
            contrast,
            brightness,
        }) = self.dialog.take()
        else {
            return Effect::None;
        };
        self.preview = None;
        self.bump_anim();
        if (colour - 1.0).abs() < f32::EPSILON
            && (contrast - 1.0).abs() < f32::EPSILON
            && (brightness - 1.0).abs() < f32::EPSILON
        {
            self.adjust_base = None;
            return Effect::None;
        }
        self.work_ticket += 1;
        self.busy = true;
        Effect::CommitAdjust {
            ticket: self.work_ticket,
            colour,
            contrast,
            brightness,
        }
    }

    fn undo(&mut self) {
        let Some(image) = self.image_mut() else {
            return;
        };
        if !image.undo() {
            return;
        }
        self.after_edit();
    }

    fn after_edit(&mut self) {
        let size = self.pixel_size();
        self.viewport.selection = None;
        if let Some(size) = size {
            self.viewport.refit(self.view_size, size);
        }
        self.preview = None;
        self.frame_index = 0;
        if let Some(image) = self.image() {
            self.loops_left = loops_remaining(&image.loop_count);
        }
        self.animation_paused = false;
        self.animation_done = false;
        self.image_generation += 1;
        self.bump_anim();
    }

    fn play_pause(&mut self) -> Effect {
        if let Some(Document::Video(video)) = &mut self.document {
            video.paused = !video.paused;
            return Effect::SyncVideo;
        }
        if self.animation_done {
            self.frame_index = 0;
            if let Some(image) = self.image() {
                self.loops_left = loops_remaining(&image.loop_count);
            }
            self.animation_done = false;
            self.animation_paused = false;
        } else {
            self.animation_paused = !self.animation_paused;
        }
        self.bump_anim();
        Effect::None
    }

    fn skip(&mut self, delta_ms: i64) -> Effect {
        if let Some(Document::Video(video)) = &mut self.document {
            let duration = video.duration.max(0.0);
            video.position = (video.position + delta_ms as f64 / 1000.0).clamp(0.0, duration);
        }
        self.poke_scrub();
        Effect::SyncVideo
    }

    fn nudge_volume(&mut self, delta: f32) -> Effect {
        if let Some(Document::Video(video)) = &mut self.document {
            video.volume = (video.volume + delta).clamp(0.0, 1.0);
            video.muted = false;
        }
        Effect::SyncVideo
    }

    fn toggle_mute(&mut self) -> Effect {
        if let Some(Document::Video(video)) = &mut self.document {
            video.muted = !video.muted;
        }
        Effect::SyncVideo
    }

    pub fn seek(&mut self, fraction: f32) -> Effect {
        if let Some(Document::Video(video)) = &mut self.document {
            video.position = video.duration.max(0.0) * f64::from(fraction.clamp(0.0, 1.0));
        }
        self.poke_scrub();
        Effect::SyncVideo
    }

    pub fn set_video_time(&mut self, position: f64, duration: f64) {
        if let Some(Document::Video(video)) = &mut self.document {
            video.position = position.max(0.0);
            if duration.is_finite() && duration > 0.0 {
                video.duration = duration;
            }
        }
    }

    pub fn set_video_size(&mut self, size: (u32, u32), duration: f64) {
        if let Some(Document::Video(video)) = &mut self.document {
            video.size = size;
            if duration.is_finite() && duration > 0.0 {
                video.duration = duration;
            }
            if size.0 > 0 && size.1 > 0 && self.viewport.fitted {
                self.viewport.refit(self.view_size, size);
            }
        }
    }

    pub fn poke_scrub(&mut self) -> u64 {
        self.scrub_visible = true;
        self.scrub_token += 1;
        self.scrub_token
    }

    pub fn hide_scrub(&mut self, token: u64) {
        if self.scrub_token == token {
            self.scrub_visible = false;
        }
    }

    pub fn animation_delay(&self) -> Option<Duration> {
        if !self.animation_running() {
            return None;
        }
        let image = self.image()?;
        let frame = image.frames.get(self.frame_index)?;
        Some(frame.delay.max(Duration::from_millis(1)))
    }

    pub fn animation_running(&self) -> bool {
        self.mode == Mode::Viewer
            && !self.animation_paused
            && !self.animation_done
            && !matches!(self.dialog, Some(Dialog::Adjust { .. }))
            && self.image().is_some_and(ImageDocument::is_animation)
    }

    pub fn advance_animation(&mut self, token: u64) {
        if token != self.anim_token || !self.animation_running() {
            return;
        }
        let count = self.image().map(|image| image.frames.len()).unwrap_or(0);
        match document::advance_frame(self.frame_index, count, &mut self.loops_left) {
            Some(index) => self.frame_index = index,
            None => self.animation_done = true,
        }
        self.bump_anim();
    }

    fn bump_anim(&mut self) {
        self.anim_token = self.anim_token.wrapping_add(1);
    }

    pub fn frames_arc(&self) -> Option<Arc<[AnimFrame]>> {
        self.image().map(|image| Arc::clone(&image.frames))
    }

    pub fn adjust_frames(&self) -> Option<Arc<[AnimFrame]>> {
        self.adjust_base.clone().or_else(|| self.frames_arc())
    }

    pub fn handle_note(&mut self, note: Note, registry: &TransitionRegistry) -> Effect {
        match note {
            Note::Opened(path) | Note::Startup(path) => {
                self.dispatch(Command::Open(path), Instant::now())
            }
            Note::Folder(Ok(folder)) => {
                self.install_folder(folder);
                if let Some(path) = self.pending_file.take() {
                    self.highlight_path(&path);
                }
                Effect::None
            }
            Note::Folder(Err(message)) => {
                tracing::warn!(%message, "could not list the folder");
                self.pending_file = None;
                self.message = Some(message);
                self.clear_busy();
                if self.document.is_none() {
                    self.mode = Mode::Browser;
                }
                Effect::None
            }
            Note::Loaded { ticket, result } => {
                if ticket != self.load_ticket {
                    return Effect::None;
                }
                self.clear_busy();
                match result {
                    Ok(document) => {
                        let navigation = self.pending_navigation;
                        self.open_media(document, navigation, registry);
                        Effect::SyncVideo
                    }
                    Err(message) => {
                        tracing::warn!(%message, "could not open the file");
                        self.message = Some(message);
                        if self.document.is_none() {
                            self.mode = Mode::Browser;
                        }
                        Effect::None
                    }
                }
            }
            Note::Transformed { ticket, result } => self.finish_frames(ticket, result),
            Note::Preview { ticket, image } => {
                if ticket == self.preview_ticket {
                    self.preview = image;
                }
                Effect::None
            }
            Note::Saved { ticket, result } => {
                if ticket != self.work_ticket {
                    return Effect::None;
                }
                self.clear_busy();
                match result {
                    Ok(()) => {
                        if let Some(image) = self.image_mut() {
                            image.clean = Arc::clone(&image.frames);
                        }
                    }
                    Err(message) => {
                        tracing::warn!(%message, "could not save the image");
                        self.message = Some(message);
                    }
                }
                Effect::None
            }
            Note::Thumbs => {
                self.thumb_epoch = self.thumb_epoch.wrapping_add(1);
                Effect::None
            }
        }
    }

    fn finish_frames(&mut self, ticket: u64, result: Result<Vec<AnimFrame>, String>) -> Effect {
        if ticket != self.work_ticket {
            return Effect::None;
        }
        self.clear_busy();
        self.adjust_base = None;
        match result {
            Ok(frames) => {
                let committed = self
                    .image_mut()
                    .map(|image| document::commit_frames(image, frames));
                match committed {
                    Some(Ok(())) => self.after_edit(),
                    Some(Err(error)) => self.message = Some(error.message),
                    None => {}
                }
            }
            Err(message) => {
                tracing::warn!(%message, "the edit was not applied");
                self.message = Some(message);
            }
        }
        Effect::None
    }

    fn install_folder(&mut self, mut folder: FolderView) {
        if let Some(memory) = self.history.get(&folder.path).copied() {
            self.pending_restore = Some(memory);
            folder.highlighted = 0;
            folder.scroll_top = 0.0;
        }
        self.folder = folder;
        self.folder_generation = self.folder_generation.wrapping_add(1);
        self.scroll_action = None;
    }

    pub fn commit_restore(&mut self) {
        let Some(memory) = self.pending_restore.take() else {
            return;
        };
        if self.folder.entries.is_empty() {
            self.folder.highlighted = 0;
        } else {
            let last = self.folder.entries.len() - 1;
            self.folder.highlighted = memory.highlighted.min(last);
        }
        self.folder.scroll_top = memory.scroll_top;
        self.scroll_action = Some(ScrollAction::Restore(memory.scroll_top));
    }

    pub fn move_highlight(&mut self, delta: isize) {
        let len = self.folder.entries.len();
        if len == 0 {
            return;
        }
        let current = self.folder.highlighted.min(len - 1) as isize;
        let next = (current + delta).clamp(0, len as isize - 1) as usize;
        self.folder.highlighted = next;
        self.scroll_action = Some(ScrollAction::Reveal);
    }

    pub fn activate_highlighted(&mut self) -> Effect {
        let Some(entry) = self.folder.entries.get(self.folder.highlighted).cloned() else {
            return Effect::None;
        };
        self.activate(entry)
    }

    pub fn activate_index(&mut self, index: usize) -> Effect {
        let Some(entry) = self.folder.entries.get(index).cloned() else {
            return Effect::None;
        };
        self.folder.highlighted = index;
        self.activate(entry)
    }

    fn activate(&mut self, entry: crate::catalog::Entry) -> Effect {
        match entry.kind {
            EntryKind::Parent | EntryKind::Directory => self.leave_for(entry.path),
            EntryKind::Image | EntryKind::Video => self.begin_load(entry.path, false),
        }
    }

    pub fn set_scroll(&mut self, top: f64) {
        self.folder.scroll_top = top;
    }

    pub fn set_view_size(&mut self, size: (f64, f64), scale: f64) {
        self.scale_factor = if scale > 0.0 { scale } else { 1.0 };
        if (size.0 - self.view_size.0).abs() < 0.5 && (size.1 - self.view_size.1).abs() < 0.5 {
            return;
        }
        let old = self.view_size;
        self.view_size = size;
        if let Some(image) = self.pixel_size() {
            self.viewport.resize(old, size, image);
        }
    }

    pub fn on_wheel(&mut self, lines: bool, delta_y: f64, at: (f64, f64)) {
        if self.mode != Mode::Viewer || self.dialog.is_some() {
            return;
        }
        let Some(size) = self.pixel_size() else {
            return;
        };
        let exponent = wheel_exponent(lines, delta_y);
        if exponent == 0.0 {
            return;
        }
        let factor = ZOOM_STEP.powf(-exponent);
        self.viewport
            .zoom_at(self.view_size, size, at.0, at.1, factor);
    }

    pub fn pointer_down(&mut self, at: (f64, f64), control: bool) {
        if self.mode != Mode::Viewer || self.dialog.is_some() {
            return;
        }
        self.drag = Some(Drag {
            start: at,
            last: at,
            control,
            origin: self.viewport.translation,
            fitted: self.viewport.fitted,
        });
    }

    pub fn pointer_move(&mut self, at: (f64, f64)) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        let control = drag.control;
        let start = drag.start;
        let origin = drag.origin;
        if control {
            if let Some(size) = self.pixel_size() {
                self.viewport.selection =
                    self.viewport
                        .selection_from_drag(self.view_size, size, start, at);
            }
        } else {
            self.viewport.fitted = false;
            self.viewport.translation = (origin.0 + (at.0 - start.0), origin.1 + (at.1 - start.1));
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.last = at;
        }
    }

    pub fn pointer_up(&mut self, at: (f64, f64)) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let moved = (at.0 - drag.start.0).hypot(at.1 - drag.start.1) >= CLICK_SLOP;
        if drag.control {
            if let Some(size) = self.pixel_size() {
                self.viewport.selection =
                    self.viewport
                        .selection_from_drag(self.view_size, size, drag.start, at);
            }
        } else if !moved {
            self.viewport.translation = drag.origin;
            self.viewport.fitted = drag.fitted;
            self.viewport.selection = None;
        }
    }

    pub fn dismiss_message(&mut self) {
        self.message = None;
    }
}

pub fn wheel_exponent(lines: bool, delta_y: f64) -> f64 {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return 0.0;
    }
    if lines {
        delta_y
    } else {
        delta_y / WHEEL_PIXELS
    }
}

fn busy_notice(id: TransformId, params: &Params) -> String {
    let label = crate::transforms::find_transform(id)
        .map(|transform| transform.label().to_string())
        .unwrap_or_else(|| "This edit".to_string());
    match params {
        Params::Scale(scale) => format!("{label} is working, {scale}×"),
        _ => format!("{label} is working"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Entry, EntryKind};
    use crate::document::{AnimFrame, Document, ImageDocument, ImageMetadata, Loop, VideoDocument};
    use crate::transitions::{ImageTransition, TransitionRegistry, TransitionStyle};
    use image::{Rgba, RgbaImage};
    use std::time::Duration;

    struct Hold;

    impl ImageTransition for Hold {
        fn id(&self) -> TransitionId {
            TransitionId::new("hold")
        }
        fn label(&self) -> &'static str {
            "Hold"
        }
        fn duration(&self) -> Duration {
            Duration::from_millis(200)
        }
        fn style(&self) -> TransitionStyle {
            TransitionStyle::Crossfade
        }
        fn applies(&self, ctx: &TransitionContext) -> bool {
            !ctx.in_browser && !ctx.current_is_video && !ctx.next_is_video
        }
    }

    fn frame() -> AnimFrame {
        AnimFrame {
            image: Arc::new(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]))),
            delay: Duration::from_millis(100),
        }
    }

    fn video_doc(name: &str, token: u64) -> Document {
        Document::Video(VideoDocument {
            path: PathBuf::from(name),
            token,
            duration: 10.0,
            position: 1.0,
            paused: false,
            volume: 1.0,
            muted: false,
            size: (320, 180),
        })
    }

    #[test]
    fn opening_a_file_shows_the_viewer_before_the_folder_is_listed() {
        let mut state = AppState::new();
        assert_eq!(state.mode, Mode::Browser);
        let effect = state.open_path(PathBuf::from("/pictures/a.png"));
        assert_eq!(state.mode, Mode::Viewer);
        assert!(matches!(effect, Effect::OpenFile { ticket: 1, .. }));
        let folder = FolderView {
            path: PathBuf::from("/pictures"),
            entries: vec![Entry {
                path: PathBuf::from("/pictures/a.png"),
                kind: EntryKind::Image,
                label: "a".into(),
            }],
            highlighted: 0,
            scroll_top: 0.0,
            error: None,
        };
        let after = state.handle_note(Note::Folder(Ok(folder)), &TransitionRegistry::builtin());
        assert!(matches!(after, Effect::None));
        assert_eq!(state.load_ticket, 1);
        assert_eq!(state.mode, Mode::Viewer);
        assert_eq!(state.folder.highlighted, 0);
        assert!(state.pending_file.is_none());
    }

    fn image_doc(name: &str) -> Document {
        let frames: Arc<[AnimFrame]> = Arc::from(vec![frame()]);
        Document::Image(ImageDocument {
            path: PathBuf::from(name),
            metadata: ImageMetadata {
                format_name: "PNG".into(),
                description: "Portable network graphics".into(),
                width: 2,
                height: 2,
                mode: "RGB".into(),
            },
            frames: Arc::clone(&frames),
            loop_count: Loop::Once,
            undo: Vec::new(),
            clean: frames,
        })
    }

    fn folder() -> FolderView {
        FolderView {
            path: PathBuf::from("/pictures"),
            entries: vec![
                Entry {
                    path: PathBuf::from("/pictures/a.png"),
                    kind: EntryKind::Image,
                    label: "a".into(),
                },
                Entry {
                    path: PathBuf::from("/pictures/b.png"),
                    kind: EntryKind::Image,
                    label: "b".into(),
                },
            ],
            highlighted: 0,
            scroll_top: 0.0,
            error: None,
        }
    }

    #[test]
    fn a_holding_transition_swallows_the_next_navigation() {
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.folder = folder();
        state.document = Some(image_doc("/pictures/a.png"));
        state.transition = TransitionId::new("hold");
        let registry = TransitionRegistry::with(vec![Box::new(Hold)]);
        assert!(state.open_media(image_doc("/pictures/b.png"), true, &registry));
        assert!(matches!(state.phase, Phase::Transitioning { .. }));
        assert_eq!(
            state.document.as_ref().map(Document::path),
            Some(Path::new("/pictures/a.png"))
        );
        assert!(state.pending_document.is_some());
        assert_eq!(state.transition.as_str(), "hold");
        let hold = Hold;
        assert_eq!(hold.label(), "Hold");
        assert_eq!(hold.duration(), Duration::from_millis(200));
        assert!(matches!(hold.style(), TransitionStyle::Crossfade));
        assert!(!state.open_media(image_doc("/pictures/a.png"), true, &registry));
        assert_eq!(
            state.document.as_ref().map(Document::path),
            Some(Path::new("/pictures/a.png"))
        );
    }

    #[test]
    fn cut_replaces_the_open_file_immediately() {
        let mut state = AppState::new();
        state.folder = folder();
        state.document = Some(image_doc("/pictures/a.png"));
        let registry = TransitionRegistry::builtin();
        assert!(state.open_media(image_doc("/pictures/b.png"), true, &registry));
        assert!(matches!(state.phase, Phase::Idle));
        assert_eq!(
            state.document.as_ref().map(Document::path),
            Some(Path::new("/pictures/b.png"))
        );
        assert!(state.viewport.fitted);
    }

    #[test]
    fn folder_memory_is_restored_after_commit() {
        let root =
            std::env::temp_dir().join(format!("image-viewer-folders-{}", std::process::id()));
        let pictures = root.join("pictures");
        let holiday = pictures.join("holiday");
        let _ = std::fs::create_dir_all(&holiday);
        let mut state = AppState::new();
        state.folder = FolderView {
            path: pictures.clone(),
            entries: folder().entries,
            highlighted: 1,
            scroll_top: 48.0,
            error: None,
        };
        let effect = state.open_path(holiday.clone());
        assert!(matches!(effect, Effect::ListFolder(_)));
        let child = FolderView {
            path: holiday,
            entries: vec![Entry {
                path: pictures.join("holiday/c.png"),
                kind: EntryKind::Image,
                label: "c".into(),
            }],
            highlighted: 0,
            scroll_top: 0.0,
            error: None,
        };
        assert!(matches!(
            state.handle_note(Note::Folder(Ok(child)), &TransitionRegistry::builtin()),
            Effect::None
        ));
        let back = state.open_path(pictures.clone());
        assert!(matches!(back, Effect::ListFolder(_)));
        let returned = FolderView {
            path: pictures,
            entries: folder().entries,
            highlighted: 0,
            scroll_top: 0.0,
            error: None,
        };
        assert!(matches!(
            state.handle_note(Note::Folder(Ok(returned)), &TransitionRegistry::builtin()),
            Effect::None
        ));
        assert!(state.pending_restore.is_some());
        state.commit_restore();
        assert_eq!(state.folder.highlighted, 1);
        assert_eq!(state.folder.scroll_top, 48.0);
        assert_eq!(state.scroll_action, Some(ScrollAction::Restore(48.0)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_visible_picture_stays_until_the_next_one_has_loaded() {
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.view_size = (200.0, 100.0);
        state.document = Some(image_doc("/pictures/a.png"));
        state.image_generation = 1;
        let size = state.pixel_size().unwrap_or((0, 0));
        state.viewport.refit(state.view_size, size);
        state.sync_display();
        assert!(!state.display_needs_sync());
        assert!(!state.video_frame_needs_sync());
        assert!(state.show_front);
        assert!(state.front.url.contains("/g/1/"));
        assert!(state.front.url.contains("/p/0"));
        assert!(state.back.url.is_empty());
        let held = state.front.clone();

        state.image_generation = 2;
        state.viewport.translation = (30.0, 40.0);
        state.viewport.fitted = false;
        assert!(state.display_needs_sync());
        state.sync_display();
        assert!(!state.display_needs_sync());
        assert!(state.show_front);
        assert_eq!(state.front, held);
        assert!(state.back.url.contains("/g/2/"));
        assert_eq!(state.back.translation, (30.0, 40.0));

        state.viewport.translation = (9.0, 9.0);
        state.sync_display();
        assert!(!state.display_needs_sync());
        assert_eq!(state.front, held);
        assert_eq!(state.back.translation, (9.0, 9.0));

        let loaded = state.back.url.clone();
        state.reveal_frame(&loaded);
        assert!(!state.show_front);
        state.reveal_frame(&held.url);
        assert!(!state.show_front);
    }

    #[test]
    fn a_shown_video_stays_up_across_pause_and_the_next_video() {
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.view_size = (200.0, 100.0);
        state.document = Some(video_doc("/pictures/a.mp4", 1));
        assert!(state.video_frame_needs_sync());
        state.retain_video_frame();
        assert!(!state.video_frame_needs_sync());
        assert!(state.video_layer.url.is_empty());
        assert_eq!(state.video_front, Some(1));
        assert!(state.video_back.is_none());
        assert!(!state.promote_video_would_change(2));
        assert!(state.promote_video_would_change(1));
        state.promote_video(1);
        assert_eq!(state.shown_video_token(), Some(1));
        assert!(state.video_show_front);
        assert!(!state.promote_video_would_change(1));
        assert!(!state.video_frame_needs_sync());
        let epoch = state.video_epoch;

        if let Some(Document::Video(video)) = &mut state.document {
            video.paused = true;
            video.position = 4.0;
        }
        assert!(!state.video_frame_needs_sync());
        state.retain_video_frame();
        assert_eq!(state.shown_video_token(), Some(1));
        assert_eq!(state.video_epoch, epoch);

        state.document = Some(video_doc("/pictures/b.mp4", 2));
        assert!(state.video_frame_needs_sync());
        state.retain_video_frame();
        assert!(!state.video_frame_needs_sync());
        assert_eq!(state.shown_video_token(), Some(1));
        assert_eq!(state.video_back, Some(2));
        assert!(state.video_show_front);
        state.promote_video(1);
        assert_eq!(state.shown_video_token(), Some(1));
        state.promote_video(2);
        assert_eq!(state.shown_video_token(), Some(2));
        assert!(!state.video_show_front);
        assert!(state.video_epoch > epoch);
        assert_eq!(state.video_front, Some(1));

        state.document = Some(image_doc("/pictures/c.png"));
        state.image_generation = 3;
        state.sync_display();
        let loaded = state.image_url();
        state.reveal_frame(&loaded);
        assert!(state.shown_video_token().is_none());
        assert!(state.video_front.is_none());
        assert!(state.video_back.is_none());
        assert!(state.video_layer.url.is_empty());
    }

    #[test]
    fn wheel_tracks_the_pointer_without_waiting_for_a_full_step() {
        assert_eq!(wheel_exponent(true, -1.0), -1.0);
        assert_eq!(wheel_exponent(false, -25.0), -0.5);
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.view_size = (200.0, 100.0);
        state.document = Some(image_doc("/pictures/a.png"));
        let size = state.pixel_size().unwrap_or((0, 0));
        state.viewport.refit(state.view_size, size);
        let at = (40.0, 30.0);
        let before = state
            .viewport
            .image_point(state.view_size, size, at.0, at.1);
        state.on_wheel(false, -25.0, at);
        let after = state
            .viewport
            .image_point(state.view_size, size, at.0, at.1);
        assert!((before.0 - after.0).abs() < 1e-6);
        assert!((before.1 - after.1).abs() < 1e-6);
        let expected = ZOOM_STEP.powf(0.5);
        assert!((state.viewport.scale - expected).abs() < 1e-9);
    }

    #[test]
    fn pan_follows_the_pointer_and_a_click_returns() {
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.view_size = (200.0, 100.0);
        state.viewport.translation = (12.0, 8.0);
        state.viewport.fitted = true;
        state.pointer_down((10.0, 10.0), false);
        state.pointer_move((18.0, 14.0));
        assert_eq!(state.viewport.translation, (20.0, 12.0));
        assert!(!state.viewport.fitted);
        state.pointer_up((18.0, 14.0));
        assert_eq!(state.viewport.translation, (20.0, 12.0));

        state.pointer_down((10.0, 10.0), false);
        state.pointer_move((11.0, 10.0));
        assert_eq!(state.viewport.translation, (21.0, 12.0));
        state.pointer_up((11.0, 10.0));
        assert_eq!(state.viewport.translation, (20.0, 12.0));
        assert!(!state.viewport.fitted);
    }

    #[test]
    fn undo_back_to_the_saved_list_clears_the_star() {
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.folder = folder();
        state.document = Some(image_doc("/pictures/a.png"));
        let edited = vec![AnimFrame {
            image: Arc::new(RgbaImage::from_pixel(2, 2, Rgba([9, 9, 9, 255]))),
            delay: Duration::ZERO,
        }];
        state
            .image_mut()
            .and_then(|image| document::commit_frames(image, edited).ok());
        assert!(state.is_dirty());
        assert_eq!(state.title(), "pictures - a *");
        state.undo();
        assert!(!state.is_dirty());
        assert!(state.viewport.fitted);
    }

    #[test]
    fn a_long_edit_says_it_is_working_until_it_finishes() -> Result<(), String> {
        let mut state = viewing_picture();
        let effect = state.dispatch(
            Command::Transform {
                id: TransformId::new("denoise"),
                params: Params::None,
            },
            Instant::now(),
        );
        assert!(matches!(effect, Effect::Transform { ticket: 1, .. }));
        assert!(state.busy);
        assert_eq!(state.message.as_deref(), Some("Denoise is working"));

        state.dismiss_message();
        assert!(state.message.is_none());

        let finished = state.handle_note(
            Note::Transformed {
                ticket: 1,
                result: Ok(vec![frame()]),
            },
            &TransitionRegistry::builtin(),
        );
        assert!(matches!(finished, Effect::None));
        assert!(!state.busy);
        assert!(state.message.is_none());
        Ok(())
    }

    #[test]
    fn a_failed_long_edit_replaces_the_busy_note() -> Result<(), String> {
        let mut state = viewing_picture();
        let started = state.dispatch(
            Command::Transform {
                id: TransformId::new("super-resolution"),
                params: Params::Scale(4),
            },
            Instant::now(),
        );
        assert!(matches!(started, Effect::Transform { .. }));
        assert_eq!(
            state.message.as_deref(),
            Some("Super Resolution is working, 4×")
        );
        let finished = state.handle_note(
            Note::Transformed {
                ticket: 1,
                result: Err("super-resolution model is missing".into()),
            },
            &TransitionRegistry::builtin(),
        );
        assert!(matches!(finished, Effect::None));
        assert!(!state.busy);
        assert_eq!(
            state.message.as_deref(),
            Some("super-resolution model is missing")
        );
        Ok(())
    }

    #[test]
    fn a_quick_edit_does_not_announce_itself() -> Result<(), String> {
        let mut state = viewing_picture();
        let effect = state.dispatch(
            Command::Transform {
                id: TransformId::new("sharpen"),
                params: Params::None,
            },
            Instant::now(),
        );
        assert!(matches!(effect, Effect::Transform { .. }));
        assert!(state.busy);
        assert!(state.message.is_none());
        Ok(())
    }

    fn viewing_picture() -> AppState {
        let mut state = AppState::new();
        state.mode = Mode::Viewer;
        state.document = Some(image_doc("/pictures/a.png"));
        state
    }
}
