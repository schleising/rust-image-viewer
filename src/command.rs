use crate::transforms::{Params, Shortcut, ShortcutKey, TransformId};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Open(PathBuf),
    ReturnToBrowser,
    Next,
    Previous,
    Quit,
    ToggleFullscreen,
    OpenAdjust,
    Transform {
        id: TransformId,
        params: Params,
    },
    Preview {
        colour: f32,
        contrast: f32,
        brightness: f32,
    },
    CommitPreview,
    CancelPreview,
    Undo,
    Save,
    ZoomToSelection,
    ResetZoom,
    CropToSelection,
    ShowInfo,
    PlayPause,
    Skip {
        delta_ms: i64,
    },
    NudgeVolume {
        delta: f32,
    },
    ToggleMute,
    Seek {
        fraction: f32,
    },
    SetSpeed {
        rate: f32,
    },
    SetLoopIn,
    SetLoopOut,
    ClearLoop,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub command: bool,
    pub option: bool,
    pub control: bool,
    pub shift: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Availability {
    pub viewing: bool,
    pub image: bool,
    pub animated: bool,
    pub video: bool,
    pub dirty: bool,
    pub selection: bool,
    pub zoomed: bool,
    pub undo: bool,
    pub busy: bool,
    pub dialog: bool,
    pub adjusting: bool,
    pub transitioning: bool,
    pub locked: bool,
}

impl Command {
    pub fn available(&self, flags: &Availability) -> bool {
        match self {
            Self::CommitPreview | Self::CancelPreview => return flags.adjusting && !flags.locked,
            Self::Quit => return !flags.dialog && !flags.locked,
            _ if flags.dialog || flags.locked => return false,
            _ => {}
        }
        match self {
            Self::Open(_) => !flags.busy,
            Self::ReturnToBrowser | Self::ToggleFullscreen => flags.viewing,
            Self::Next | Self::Previous => flags.viewing && !flags.transitioning,
            Self::Quit | Self::CommitPreview | Self::CancelPreview => false,
            Self::OpenAdjust => flags.image && !flags.busy,
            Self::Transform { .. } => flags.image && !flags.busy,
            Self::Preview { .. } => flags.image && !flags.busy,
            Self::Undo => flags.image && flags.undo && !flags.busy,
            Self::Save => flags.image && flags.dirty && !flags.busy,
            Self::ZoomToSelection => (flags.image || flags.video) && flags.selection,
            Self::ResetZoom => flags.viewing && flags.zoomed,
            Self::CropToSelection => flags.image && flags.selection && !flags.busy,
            Self::ShowInfo => flags.image,
            Self::PlayPause => flags.video || flags.animated,
            Self::Skip { .. } | Self::NudgeVolume { .. } | Self::ToggleMute => flags.video,
            Self::Seek { .. }
            | Self::SetSpeed { .. }
            | Self::SetLoopIn
            | Self::SetLoopOut
            | Self::ClearLoop => flags.video || flags.animated,
        }
    }

    pub fn id_key(&self) -> String {
        match self {
            Self::Open(path) => format!("open:{}", path.display()),
            Self::Transform { id, params } => format!("transform:{}:{params:?}", id.as_str()),
            Self::Preview {
                colour,
                contrast,
                brightness,
            } => {
                format!("preview:{colour}:{contrast}:{brightness}")
            }
            Self::Skip { delta_ms } => format!("skip:{delta_ms}"),
            Self::NudgeVolume { delta } => format!("volume:{delta}"),
            Self::Seek { fraction } => format!("seek:{fraction}"),
            Self::SetSpeed { rate } => format!("speed:{rate}"),
            other => format!("{other:?}"),
        }
    }
}

pub fn commands_for_key(mods: Mods, key: ShortcutKey) -> Vec<Command> {
    let mut commands = Vec::new();
    if mods == Mods::default() {
        match key {
            ShortcutKey::Left => commands.push(Command::Previous),
            ShortcutKey::Right => commands.push(Command::Next),
            ShortcutKey::Up => commands.push(Command::ReturnToBrowser),
            ShortcutKey::Space => commands.push(Command::PlayPause),
            ShortcutKey::Enter => commands.push(Command::ToggleFullscreen),
            ShortcutKey::Char('i') => commands.push(Command::SetLoopIn),
            ShortcutKey::Char('o') => commands.push(Command::SetLoopOut),
            _ => {}
        }
    }
    if mods.command && !mods.option && !mods.control && !mods.shift && key == ShortcutKey::Char('s')
    {
        commands.push(Command::Save);
    }
    if mods.command && !mods.option && !mods.control && !mods.shift && key == ShortcutKey::Char('z')
    {
        commands.push(Command::Undo);
    }
    if mods.command && !mods.option && !mods.control && !mods.shift && key == ShortcutKey::Char('i')
    {
        commands.push(Command::ShowInfo);
    }
    if mods.control && !mods.command && !mods.option && !mods.shift && key == ShortcutKey::Char('a')
    {
        commands.push(Command::OpenAdjust);
    }
    if mods.control && !mods.command && !mods.option && !mods.shift && key == ShortcutKey::Char('z')
    {
        commands.push(Command::ZoomToSelection);
    }
    if mods.control && !mods.command && !mods.option && !mods.shift && key == ShortcutKey::Char('r')
    {
        commands.push(Command::ResetZoom);
    }
    if mods.control && !mods.command && !mods.option && !mods.shift && key == ShortcutKey::Char('c')
    {
        commands.push(Command::CropToSelection);
    }
    if mods.shift && !mods.command && !mods.option && !mods.control && key == ShortcutKey::Char('m')
    {
        commands.push(Command::ToggleMute);
    }
    if mods.shift && !mods.command && !mods.option && !mods.control {
        match key {
            ShortcutKey::Left => {
                commands.push(Command::Transform {
                    id: TransformId::new("colour"),
                    params: Params::Factor(0.9),
                });
                commands.push(Command::Skip { delta_ms: -5_000 });
            }
            ShortcutKey::Right => {
                commands.push(Command::Transform {
                    id: TransformId::new("colour"),
                    params: Params::Factor(1.1),
                });
                commands.push(Command::Skip { delta_ms: 5_000 });
            }
            ShortcutKey::Up => commands.push(Command::NudgeVolume { delta: 0.1 }),
            ShortcutKey::Down => commands.push(Command::NudgeVolume { delta: -0.1 }),
            _ => {}
        }
    }
    if mods.option && !mods.command && !mods.control && !mods.shift {
        match key {
            ShortcutKey::Left => commands.push(Command::Transform {
                id: TransformId::new("contrast"),
                params: Params::Factor(0.9),
            }),
            ShortcutKey::Right => commands.push(Command::Transform {
                id: TransformId::new("contrast"),
                params: Params::Factor(1.1),
            }),
            _ => {}
        }
    }
    if mods.command && !mods.option && !mods.control && !mods.shift {
        match key {
            ShortcutKey::Left => commands.push(Command::Transform {
                id: TransformId::new("brightness"),
                params: Params::Factor(0.9),
            }),
            ShortcutKey::Right => commands.push(Command::Transform {
                id: TransformId::new("brightness"),
                params: Params::Factor(1.1),
            }),
            _ => {}
        }
    }
    for transform in crate::transforms::builtin_transforms() {
        if let Some(shortcut) = transform.shortcut() {
            if shortcut_matches(shortcut, mods, key) {
                let params = match transform.id().as_str() {
                    "colour" => Params::Factor(1.1),
                    "contrast" => Params::Factor(1.1),
                    "brightness" => Params::Factor(1.1),
                    "super-resolution" => Params::Scale(4),
                    _ => Params::None,
                };
                let command = Command::Transform {
                    id: transform.id(),
                    params,
                };
                if !commands.contains(&command) {
                    commands.push(command);
                }
            }
        }
    }
    commands
}

fn shortcut_matches(shortcut: Shortcut, mods: Mods, key: ShortcutKey) -> bool {
    shortcut.command == mods.command
        && shortcut.option == mods.option
        && shortcut.control == mods.control
        && shortcut.shift == mods.shift
        && shortcut.key == key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags() -> Availability {
        Availability {
            viewing: false,
            image: false,
            animated: false,
            video: false,
            dirty: false,
            selection: false,
            zoomed: false,
            undo: false,
            busy: false,
            dialog: false,
            adjusting: false,
            transitioning: false,
            locked: false,
        }
    }

    #[test]
    fn availability_follows_the_mode() {
        let browser = flags();
        assert!(!Command::Save.available(&browser));
        assert!(!Command::Next.available(&browser));
        assert!(Command::Quit.available(&browser));

        let mut image = flags();
        image.viewing = true;
        image.image = true;
        image.dirty = true;
        image.selection = true;
        image.undo = true;
        image.zoomed = true;
        assert!(Command::Save.available(&image));
        assert!(Command::CropToSelection.available(&image));
        assert!(!Command::Skip { delta_ms: 5_000 }.available(&image));
        assert!(Command::PlayPause.available(&image) == image.animated);
        assert!(!Command::SetSpeed { rate: 2.0 }.available(&image));

        image.animated = true;
        assert!(Command::PlayPause.available(&image));
        assert!(Command::SetSpeed { rate: 2.0 }.available(&image));
        assert!(Command::SetLoopIn.available(&image));
        assert!(!Command::Skip { delta_ms: 5_000 }.available(&image));

        let enter = commands_for_key(Mods::default(), ShortcutKey::Enter);
        assert_eq!(enter, vec![Command::ToggleFullscreen]);
        assert!(enter[0].available(&image));
        assert!(!enter[0].available(&browser));

        let mut video = flags();
        video.viewing = true;
        video.video = true;
        video.selection = true;
        assert!(Command::Skip { delta_ms: -5_000 }.available(&video));
        assert!(Command::ZoomToSelection.available(&video));
        assert!(!Command::Save.available(&video));
        assert!(!Command::CropToSelection.available(&video));

        image.dialog = true;
        assert!(!Command::Quit.available(&image));
        assert!(!Command::Save.available(&image));
        image.adjusting = true;
        assert!(Command::CancelPreview.available(&image));
    }
}
