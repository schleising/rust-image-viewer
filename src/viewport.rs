#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    pub fitted: bool,
    pub scale: f64,
    pub translation: (f64, f64),
    pub selection: Option<PixelRect>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self::fitted()
    }
}

impl Viewport {
    pub fn fitted() -> Self {
        Self {
            fitted: true,
            scale: 1.0,
            translation: (0.0, 0.0),
            selection: None,
        }
    }

    pub fn refit(&mut self, view: (f64, f64), image: (u32, u32)) {
        self.fitted = true;
        self.scale = 1.0;
        self.translation = centered_translation(view, image, 1.0);
    }

    pub fn image_point(&self, view: (f64, f64), image: (u32, u32), x: f64, y: f64) -> (f64, f64) {
        let display = fit_factor(view, image) * self.scale;
        if display == 0.0 {
            return (0.0, 0.0);
        }
        (
            (x - self.translation.0) / display,
            (y - self.translation.1) / display,
        )
    }

    pub fn zoom_at(&mut self, view: (f64, f64), image: (u32, u32), x: f64, y: f64, factor: f64) {
        let (image_x, image_y) = self.image_point(view, image, x, y);
        self.scale = (self.scale * factor).clamp(0.05, 64.0);
        self.fitted = false;
        let display = fit_factor(view, image) * self.scale;
        self.translation = (x - image_x * display, y - image_y * display);
    }

    pub fn resize(&mut self, old: (f64, f64), new: (f64, f64), image: (u32, u32)) {
        if self.fitted {
            self.refit(new, image);
            return;
        }
        let (image_x, image_y) = self.image_point(old, image, old.0 / 2.0, old.1 / 2.0);
        let display = fit_factor(new, image) * self.scale;
        self.translation = (
            new.0 / 2.0 - image_x * display,
            new.1 / 2.0 - image_y * display,
        );
    }

    pub fn zoom_to_rect(&mut self, view: (f64, f64), image: (u32, u32), rect: PixelRect) {
        if rect.width == 0 || rect.height == 0 || view.0 <= 0.0 || view.1 <= 0.0 {
            return;
        }
        let fit = fit_factor(view, image);
        if fit == 0.0 {
            return;
        }
        self.scale = (view.0 / rect.width as f64).min(view.1 / rect.height as f64) / fit;
        self.fitted = false;
        let display = fit * self.scale;
        let shown_w = rect.width as f64 * display;
        let shown_h = rect.height as f64 * display;
        self.translation = (
            (view.0 - shown_w) / 2.0 - rect.x as f64 * display,
            (view.1 - shown_h) / 2.0 - rect.y as f64 * display,
        );
    }

    pub fn selection_from_drag(
        &self,
        view: (f64, f64),
        image: (u32, u32),
        start: (f64, f64),
        end: (f64, f64),
    ) -> Option<PixelRect> {
        let a = self.image_point(view, image, start.0, start.1);
        let b = self.image_point(view, image, end.0, end.1);
        rect_from_points(a, b, image.0, image.1)
    }
}

pub fn fit_factor(view: (f64, f64), image: (u32, u32)) -> f64 {
    if image.0 == 0 || image.1 == 0 || view.0 <= 0.0 || view.1 <= 0.0 {
        return 1.0;
    }
    (view.0 / image.0 as f64).min(view.1 / image.1 as f64)
}

pub fn centered_translation(view: (f64, f64), image: (u32, u32), scale: f64) -> (f64, f64) {
    let display = fit_factor(view, image) * scale;
    let width = image.0 as f64 * display;
    let height = image.1 as f64 * display;
    ((view.0 - width) / 2.0, (view.1 - height) / 2.0)
}

pub fn clamp_rect(rect: PixelRect, width: u32, height: u32) -> Option<PixelRect> {
    if rect.width == 0 || rect.height == 0 || rect.x >= width || rect.y >= height {
        return None;
    }
    let rect_width = rect.width.min(width - rect.x);
    let rect_height = rect.height.min(height - rect.y);
    if rect_width == 0 || rect_height == 0 {
        return None;
    }
    Some(PixelRect {
        x: rect.x,
        y: rect.y,
        width: rect_width,
        height: rect_height,
    })
}

fn rect_from_points(a: (f64, f64), b: (f64, f64), width: u32, height: u32) -> Option<PixelRect> {
    let left = a.0.min(b.0).floor().max(0.0);
    let top = a.1.min(b.1).floor().max(0.0);
    let right = a.0.max(b.0).ceil().min(width as f64);
    let bottom = a.1.max(b.1).ceil().min(height as f64);
    if right <= left || bottom <= top {
        return None;
    }
    clamp_rect(
        PixelRect {
            x: left as u32,
            y: top as u32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        },
        width,
        height,
    )
}

pub fn fitted_box(view: (f64, f64), image: (u32, u32)) -> (f64, f64) {
    let factor = fit_factor(view, image);
    (image.0 as f64 * factor, image.1 as f64 * factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_keeps_the_image_point_under_the_pointer() {
        let view = (200.0, 100.0);
        let image = (100, 100);
        let mut viewport = Viewport::fitted();
        viewport.refit(view, image);
        let pointer = (100.0, 50.0);
        let before = viewport.image_point(view, image, pointer.0, pointer.1);
        viewport.zoom_at(view, image, pointer.0, pointer.1, 1.05);
        let after = viewport.image_point(view, image, pointer.0, pointer.1);
        assert!((before.0 - after.0).abs() < 1e-9);
        assert!((before.1 - after.1).abs() < 1e-9);
        assert!(!viewport.fitted);
    }

    #[test]
    fn zoom_to_rect_preserves_aspect_and_frames_the_selection() {
        let view = (200.0, 100.0);
        let image = (100, 50);
        let mut viewport = Viewport::fitted();
        viewport.refit(view, image);
        let rect = PixelRect {
            x: 10,
            y: 10,
            width: 20,
            height: 10,
        };
        viewport.zoom_to_rect(view, image, rect);
        assert!(!viewport.fitted);
        let display = fit_factor(view, image) * viewport.scale;
        let shown_w = rect.width as f64 * display;
        let shown_h = rect.height as f64 * display;
        assert!((shown_w - view.0).abs() < 1e-6 || (shown_h - view.1).abs() < 1e-6);
        assert!(shown_w <= view.0 + 1e-6);
        assert!(shown_h <= view.1 + 1e-6);
        let origin = viewport.image_point(view, image, view.0 / 2.0, view.1 / 2.0);
        assert!((origin.0 - 20.0).abs() < 1e-6);
        assert!((origin.1 - 15.0).abs() < 1e-6);
    }

    #[test]
    fn crop_rect_is_clamped_to_the_image() {
        let clamped = clamp_rect(
            PixelRect {
                x: 80,
                y: 90,
                width: 50,
                height: 40,
            },
            100,
            100,
        );
        assert_eq!(
            clamped,
            Some(PixelRect {
                x: 80,
                y: 90,
                width: 20,
                height: 10,
            })
        );
        assert_eq!(
            clamp_rect(
                PixelRect {
                    x: 100,
                    y: 0,
                    width: 10,
                    height: 10,
                },
                100,
                100,
            ),
            None
        );
    }

    #[test]
    fn resize_refits_only_while_fitted_and_keeps_the_centre_when_zoomed() {
        let image = (100, 100);
        let old = (200.0, 100.0);
        let mut fitted = Viewport::fitted();
        fitted.refit(old, image);
        fitted.resize(old, (400.0, 200.0), image);
        assert!(fitted.fitted);
        assert_eq!(fitted.scale, 1.0);
        assert_eq!(
            fitted.translation,
            centered_translation((400.0, 200.0), image, 1.0)
        );

        let mut zoomed = Viewport::fitted();
        zoomed.refit(old, image);
        zoomed.zoom_at(old, image, 100.0, 50.0, 1.05);
        let centre = zoomed.image_point(old, image, old.0 / 2.0, old.1 / 2.0);
        let new = (400.0, 200.0);
        zoomed.resize(old, new, image);
        let after = zoomed.image_point(new, image, new.0 / 2.0, new.1 / 2.0);
        assert!(!zoomed.fitted);
        assert!((centre.0 - after.0).abs() < 1e-6);
        assert!((centre.1 - after.1).abs() < 1e-6);
    }
}
