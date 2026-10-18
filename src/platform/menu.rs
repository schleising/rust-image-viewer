use crate::command::{Availability, Command};
use crate::transforms::{self, MenuGroup, Params, Shortcut, ShortcutKey};
use dioxus::desktop::muda::accelerator::{Accelerator, Code, Modifiers};
use dioxus::desktop::muda::{AboutMetadata, Icon, Menu, MenuItem, PredefinedMenuItem, Submenu};
use std::cell::RefCell;

struct Hub {
    file: Submenu,
    show: Submenu,
    image: Submenu,
    video: Submenu,
    items: Vec<(String, MenuItem, Command)>,
}

thread_local! {
    static HUB: RefCell<Option<Hub>> = const { RefCell::new(None) };
}

pub fn native_menu() -> Menu {
    let menu = Menu::new();
    let app = Submenu::new("Image Viewer", true);
    let file = Submenu::new("File", false);
    let show = Submenu::new("Show", false);
    let image = Submenu::new("Image", false);
    let video = Submenu::new("Video", false);
    let mut items = Vec::new();

    let _ = app.append(&PredefinedMenuItem::about(
        Some("About Image Viewer"),
        Some(about_metadata()),
    ));
    let _ = app.append(&PredefinedMenuItem::separator());
    let _ = app.append(&PredefinedMenuItem::hide(None));
    let _ = app.append(&PredefinedMenuItem::hide_others(None));
    let _ = app.append(&PredefinedMenuItem::show_all(None));
    let _ = app.append(&PredefinedMenuItem::separator());
    push_item(
        &app,
        &mut items,
        "quit",
        "Quit",
        accel(Modifiers::META, Code::KeyQ),
        Command::Quit,
        true,
    );

    push_item(
        &file,
        &mut items,
        "save",
        "Save",
        accel(Modifiers::META, Code::KeyS),
        Command::Save,
        false,
    );

    push_item(
        &show,
        &mut items,
        "return",
        "Return to Browser",
        accel(Modifiers::empty(), Code::ArrowUp),
        Command::ReturnToBrowser,
        false,
    );
    push_item(
        &show,
        &mut items,
        "next",
        "Next",
        accel(Modifiers::empty(), Code::ArrowRight),
        Command::Next,
        false,
    );
    push_item(
        &show,
        &mut items,
        "previous",
        "Previous",
        accel(Modifiers::empty(), Code::ArrowLeft),
        Command::Previous,
        false,
    );
    let _ = show.append(&PredefinedMenuItem::separator());
    push_item(
        &show,
        &mut items,
        "adjust",
        "Adjust Image",
        accel(Modifiers::CONTROL, Code::KeyA),
        Command::OpenAdjust,
        false,
    );
    let _ = show.append(&PredefinedMenuItem::separator());
    push_item(
        &show,
        &mut items,
        "zoom-rect",
        "Zoom to Rect",
        accel(Modifiers::CONTROL, Code::KeyZ),
        Command::ZoomToSelection,
        false,
    );
    push_item(
        &show,
        &mut items,
        "reset-zoom",
        "Reset Zoom",
        accel(Modifiers::CONTROL, Code::KeyR),
        Command::ResetZoom,
        false,
    );
    let _ = show.append(&PredefinedMenuItem::separator());
    push_item(
        &show,
        &mut items,
        "info",
        "Image Information",
        accel(Modifiers::META, Code::KeyI),
        Command::ShowInfo,
        false,
    );

    let mut transforms = transforms::builtin_transforms();
    transforms.sort_by_key(|item| item.order());
    let mut last_group: Option<MenuGroup> = None;
    for transform in transforms {
        if last_group.is_some_and(|group| group != transform.group()) {
            let _ = image.append(&PredefinedMenuItem::separator());
        }
        last_group = Some(transform.group());
        let id = format!("transform:{}", transform.id().as_str());
        let accelerator = transform.shortcut().and_then(shortcut_accel);
        push_item(
            &image,
            &mut items,
            &id,
            transform.label(),
            accelerator,
            Command::Transform {
                id: transform.id(),
                params: params_for(transform.id().as_str()),
            },
            false,
        );
    }
    let _ = image.append(&PredefinedMenuItem::separator());
    push_item(
        &image,
        &mut items,
        "undo",
        "Undo",
        accel(Modifiers::META, Code::KeyZ),
        Command::Undo,
        false,
    );

    push_item(
        &video,
        &mut items,
        "play",
        "Play / Pause",
        accel(Modifiers::empty(), Code::Space),
        Command::PlayPause,
        false,
    );
    push_item(
        &video,
        &mut items,
        "skip-forward",
        "Skip Forwards",
        accel(Modifiers::SHIFT, Code::ArrowRight),
        Command::Skip { delta_ms: 5_000 },
        false,
    );
    push_item(
        &video,
        &mut items,
        "skip-back",
        "Skip Backwards",
        accel(Modifiers::SHIFT, Code::ArrowLeft),
        Command::Skip { delta_ms: -5_000 },
        false,
    );
    push_item(
        &video,
        &mut items,
        "volume-up",
        "Volume Up",
        accel(Modifiers::SHIFT, Code::ArrowUp),
        Command::NudgeVolume { delta: 0.1 },
        false,
    );
    push_item(
        &video,
        &mut items,
        "volume-down",
        "Volume Down",
        accel(Modifiers::SHIFT, Code::ArrowDown),
        Command::NudgeVolume { delta: -0.1 },
        false,
    );
    push_item(
        &video,
        &mut items,
        "mute",
        "Toggle Mute",
        accel(Modifiers::SHIFT, Code::KeyM),
        Command::ToggleMute,
        false,
    );
    let _ = video.append(&PredefinedMenuItem::separator());
    push_item(
        &video,
        &mut items,
        "speed-half",
        "Speed 0.5×",
        None,
        Command::SetSpeed { rate: 0.5 },
        false,
    );
    push_item(
        &video,
        &mut items,
        "speed-normal",
        "Normal Speed",
        None,
        Command::SetSpeed { rate: 1.0 },
        false,
    );
    push_item(
        &video,
        &mut items,
        "speed-double",
        "Speed 2×",
        None,
        Command::SetSpeed { rate: 2.0 },
        false,
    );
    let _ = video.append(&PredefinedMenuItem::separator());
    push_item(
        &video,
        &mut items,
        "loop-in",
        "Set Loop In",
        accel(Modifiers::empty(), Code::KeyI),
        Command::SetLoopIn,
        false,
    );
    push_item(
        &video,
        &mut items,
        "loop-out",
        "Set Loop Out",
        accel(Modifiers::empty(), Code::KeyO),
        Command::SetLoopOut,
        false,
    );
    push_item(
        &video,
        &mut items,
        "loop-clear",
        "Clear Loop",
        None,
        Command::ClearLoop,
        false,
    );

    let _ = menu.append(&app);
    let _ = menu.append(&file);
    let _ = menu.append(&show);
    let _ = menu.append(&image);
    let _ = menu.append(&video);
    HUB.with(|hub| {
        *hub.borrow_mut() = Some(Hub {
            file,
            show,
            image,
            video,
            items,
        });
    });
    menu
}

pub fn sync(flags: &Availability) {
    HUB.with(|hub| {
        let hub = hub.borrow();
        let Some(hub) = hub.as_ref() else {
            return;
        };
        hub.file.set_enabled(flags.viewing);
        hub.show.set_enabled(flags.viewing);
        hub.image.set_enabled(flags.viewing);
        hub.video.set_enabled(flags.viewing);
        for (_, item, command) in &hub.items {
            item.set_enabled(command.available(flags));
        }
    });
}

pub fn command_for(id: &str) -> Option<Command> {
    HUB.with(|hub| {
        hub.borrow().as_ref().and_then(|hub| {
            hub.items
                .iter()
                .find(|(item_id, _, _)| item_id == id)
                .map(|(_, _, command)| command.clone())
        })
    })
}

fn push_item(
    menu: &Submenu,
    items: &mut Vec<(String, MenuItem, Command)>,
    id: &str,
    label: &str,
    accelerator: Option<Accelerator>,
    command: Command,
    enabled: bool,
) {
    let item = MenuItem::with_id(id, label, enabled, accelerator);
    let _ = menu.append(&item);
    items.push((id.to_string(), item, command));
}

fn accel(mods: Modifiers, code: Code) -> Option<Accelerator> {
    Some(Accelerator::new(mods, code))
}

fn shortcut_accel(shortcut: Shortcut) -> Option<Accelerator> {
    let mut mods = Modifiers::empty();
    if shortcut.command {
        mods |= Modifiers::META;
    }
    if shortcut.option {
        mods |= Modifiers::ALT;
    }
    if shortcut.control {
        mods |= Modifiers::CONTROL;
    }
    if shortcut.shift {
        mods |= Modifiers::SHIFT;
    }
    let code = match shortcut.key {
        ShortcutKey::Left => Code::ArrowLeft,
        ShortcutKey::Right => Code::ArrowRight,
        ShortcutKey::Up => Code::ArrowUp,
        ShortcutKey::Down => Code::ArrowDown,
        ShortcutKey::Space => Code::Space,
        ShortcutKey::Enter => Code::Enter,
        ShortcutKey::Char(ch) => char_code(ch)?,
    };
    Some(Accelerator::new(mods, code))
}

fn char_code(ch: char) -> Option<Code> {
    Some(match ch {
        'a' => Code::KeyA,
        'b' => Code::KeyB,
        'c' => Code::KeyC,
        'd' => Code::KeyD,
        'e' => Code::KeyE,
        'f' => Code::KeyF,
        'i' => Code::KeyI,
        'm' => Code::KeyM,
        'n' => Code::KeyN,
        'o' => Code::KeyO,
        'q' => Code::KeyQ,
        'r' => Code::KeyR,
        's' => Code::KeyS,
        'u' => Code::KeyU,
        'w' => Code::KeyW,
        'z' => Code::KeyZ,
        _ => return None,
    })
}

fn params_for(id: &str) -> Params {
    match id {
        "colour" | "contrast" | "brightness" => Params::Factor(1.1),
        "black-and-white" => Params::Factor(0.0),
        "super-resolution" => Params::Scale(4),
        _ => Params::None,
    }
}

fn about_metadata() -> AboutMetadata {
    AboutMetadata {
        name: Some("Image Viewer".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        copyright: Some("Copyright © 2026 Steve".into()),
        icon: about_icon(),
        ..AboutMetadata::default()
    }
}

fn about_icon() -> Option<Icon> {
    let image = image::load_from_memory(include_bytes!("../../assets/AppIcon.png")).ok()?;
    let image = image
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}
