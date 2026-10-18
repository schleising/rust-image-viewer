use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionId(&'static str);

impl TransitionId {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TransitionStyle {
    Cut = 0,
    #[allow(dead_code)]
    Crossfade = 1,
    #[allow(dead_code)]
    Procedural = 2,
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct TransitionContext {
    pub in_browser: bool,
    pub current_is_video: bool,
    pub next_is_video: bool,
}

pub trait ImageTransition: Send + Sync {
    fn id(&self) -> TransitionId;
    fn label(&self) -> &'static str;
    fn duration(&self) -> Duration;
    fn style(&self) -> TransitionStyle;
    fn applies(&self, ctx: &TransitionContext) -> bool;
}

pub struct Cut;

impl ImageTransition for Cut {
    fn id(&self) -> TransitionId {
        TransitionId::new("cut")
    }
    fn label(&self) -> &'static str {
        "Cut"
    }
    fn duration(&self) -> Duration {
        Duration::ZERO
    }
    fn style(&self) -> TransitionStyle {
        TransitionStyle::Cut
    }
    fn applies(&self, _: &TransitionContext) -> bool {
        false
    }
}

pub struct TransitionRegistry {
    items: Vec<Box<dyn ImageTransition>>,
}

impl TransitionRegistry {
    pub fn builtin() -> Self {
        Self {
            items: vec![Box::new(Cut)],
        }
    }

    #[cfg(test)]
    pub fn with(items: Vec<Box<dyn ImageTransition>>) -> Self {
        Self { items }
    }

    pub fn get(&self, id: TransitionId) -> Option<&dyn ImageTransition> {
        self.items
            .iter()
            .find(|item| item.id() == id)
            .map(|item| item.as_ref())
    }
}

impl Default for TransitionRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}
