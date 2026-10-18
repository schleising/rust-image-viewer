mod denoise;
mod enhance;
mod kernels;
mod super_resolution;

use image::RgbaImage;

pub use denoise::Denoise;
pub use enhance::{
    apply_adjust, AutoContrast, BlackAndWhite, Brightness, Colour, Contrast, UnsharpMask,
};
pub use kernels::{Blur, Contour, Detail, EdgeEnhance, Emboss, FindEdges, Sharpen, Smooth};
pub use super_resolution::SuperResolution;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransformId(&'static str);

impl TransformId {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuGroup {
    Adjust,
    Filter,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Params {
    None,
    Factor(f32),
    Adjust {
        colour: f32,
        contrast: f32,
        brightness: f32,
    },
    Scale(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    Left,
    Right,
    Up,
    Down,
    Space,
    Char(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub command: bool,
    pub option: bool,
    pub control: bool,
    pub shift: bool,
    pub key: ShortcutKey,
}

impl Shortcut {
    pub const fn new(key: ShortcutKey) -> Self {
        Self {
            command: false,
            option: false,
            control: false,
            shift: false,
            key,
        }
    }

    pub const fn command(mut self) -> Self {
        self.command = true;
        self
    }

    pub const fn option(mut self) -> Self {
        self.option = true;
        self
    }

    pub const fn control(mut self) -> Self {
        self.control = true;
        self
    }

    pub const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }
}

#[derive(Debug)]
pub enum TransformError {
    BadParams,
    TooLarge,
    MissingModel(String),
    Failed(String),
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadParams => f.write_str("this adjustment does not apply"),
            Self::TooLarge => f.write_str("the result would exceed the animation size limit"),
            Self::MissingModel(path) => write!(f, "super-resolution model is missing ({path})"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for TransformError {}

pub trait Transform: Send + Sync {
    fn id(&self) -> TransformId;
    fn label(&self) -> &'static str;
    fn shortcut(&self) -> Option<Shortcut>;
    fn group(&self) -> MenuGroup;
    fn order(&self) -> u8;
    fn apply(&self, image: &RgbaImage, params: &Params) -> Result<RgbaImage, TransformError>;
}

pub fn builtin_transforms() -> Vec<Box<dyn Transform>> {
    vec![
        Box::new(Colour),
        Box::new(Contrast),
        Box::new(Brightness),
        Box::new(BlackAndWhite),
        Box::new(Sharpen),
        Box::new(Blur),
        Box::new(Contour),
        Box::new(Detail),
        Box::new(EdgeEnhance),
        Box::new(Emboss),
        Box::new(FindEdges),
        Box::new(Smooth),
        Box::new(UnsharpMask),
        Box::new(AutoContrast),
        Box::new(Denoise),
        Box::new(SuperResolution),
    ]
}

pub fn find_transform(id: TransformId) -> Option<Box<dyn Transform>> {
    builtin_transforms()
        .into_iter()
        .find(|item| item.id() == id)
}

pub fn round_channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub fn blocking_transform(id: TransformId) -> bool {
    matches!(id.as_str(), "denoise" | "super-resolution")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_oversized_result_has_its_own_error() {
        let message = TransformError::TooLarge.to_string();
        assert!(message.contains("animation"));
    }
}
