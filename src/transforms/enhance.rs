use super::{
    round_channel, MenuGroup, Params, Shortcut, ShortcutKey, Transform, TransformError, TransformId,
};
use image::{Rgba, RgbaImage};

fn map_rgb(image: &RgbaImage, mut pixel: impl FnMut(f32, f32, f32) -> [f32; 3]) -> RgbaImage {
    let mut out = RgbaImage::new(image.width(), image.height());
    for (source, dest) in image.pixels().zip(out.pixels_mut()) {
        let mapped = pixel(source[0] as f32, source[1] as f32, source[2] as f32);
        *dest = Rgba([
            round_channel(mapped[0]),
            round_channel(mapped[1]),
            round_channel(mapped[2]),
            source[3],
        ]);
    }
    out
}

fn luminance(red: f32, green: f32, blue: f32) -> f32 {
    0.299 * red + 0.587 * green + 0.114 * blue
}

fn blend(original: f32, degenerate: f32, factor: f32) -> f32 {
    degenerate * (1.0 - factor) + original * factor
}

pub fn apply_colour(image: &RgbaImage, factor: f32) -> RgbaImage {
    map_rgb(image, |red, green, blue| {
        let gray = luminance(red, green, blue);
        [
            blend(red, gray, factor),
            blend(green, gray, factor),
            blend(blue, gray, factor),
        ]
    })
}

pub fn apply_contrast(image: &RgbaImage, factor: f32) -> RgbaImage {
    let mut sums = [0.0_f32; 3];
    let count = image.pixels().len().max(1) as f32;
    for pixel in image.pixels() {
        for channel in 0..3 {
            sums[channel] += pixel[channel] as f32;
        }
    }
    let mean = [sums[0] / count, sums[1] / count, sums[2] / count];
    map_rgb(image, |red, green, blue| {
        [
            blend(red, mean[0], factor),
            blend(green, mean[1], factor),
            blend(blue, mean[2], factor),
        ]
    })
}

pub fn apply_brightness(image: &RgbaImage, factor: f32) -> RgbaImage {
    map_rgb(image, |red, green, blue| {
        [
            blend(red, 0.0, factor),
            blend(green, 0.0, factor),
            blend(blue, 0.0, factor),
        ]
    })
}

pub fn apply_adjust(image: &RgbaImage, colour: f32, contrast: f32, brightness: f32) -> RgbaImage {
    let coloured = apply_colour(image, colour);
    let contrasted = apply_contrast(&coloured, contrast);
    apply_brightness(&contrasted, brightness)
}

fn channel_planes(image: &RgbaImage) -> [Vec<f32>; 3] {
    let mut planes = [Vec::new(), Vec::new(), Vec::new()];
    for pixel in image.pixels() {
        for channel in 0..3 {
            planes[channel].push(pixel[channel] as f32);
        }
    }
    planes
}

fn write_planes(image: &RgbaImage, planes: [Vec<f32>; 3]) -> RgbaImage {
    let mut out = image.clone();
    for (index, pixel) in out.pixels_mut().enumerate() {
        for channel in 0..3 {
            pixel[channel] = round_channel(planes[channel][index]);
        }
    }
    out
}

pub fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let radius = (3.0 * sigma).ceil() as i32;
    let mut kernel = Vec::new();
    let mut sum = 0.0;
    for offset in -radius..=radius {
        let weight = (-0.5 * (offset as f32 / sigma).powi(2)).exp();
        kernel.push(weight);
        sum += weight;
    }
    if sum > 0.0 {
        for weight in &mut kernel {
            *weight /= sum;
        }
    }
    kernel
}

pub fn gaussian_blur_channel(samples: &[f32], width: usize, height: usize, sigma: f32) -> Vec<f32> {
    let kernel = gaussian_kernel(sigma);
    let radius = (kernel.len() / 2) as i32;
    let mut horizontal = vec![0.0; samples.len()];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            for (index, weight) in kernel.iter().enumerate() {
                let sx = x as i32 + index as i32 - radius;
                if sx >= 0 && (sx as usize) < width {
                    sum += samples[y * width + sx as usize] * weight;
                }
            }
            horizontal[y * width + x] = sum;
        }
    }
    let mut vertical = vec![0.0; samples.len()];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            for (index, weight) in kernel.iter().enumerate() {
                let sy = y as i32 + index as i32 - radius;
                if sy >= 0 && (sy as usize) < height {
                    sum += horizontal[sy as usize * width + x] * weight;
                }
            }
            vertical[y * width + x] = sum;
        }
    }
    vertical
}

pub fn unsharp(image: &RgbaImage) -> RgbaImage {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let planes = channel_planes(image);
    let mut next = planes.clone();
    for channel in 0..3 {
        let blurred = gaussian_blur_channel(&planes[channel], width, height, 2.0);
        for index in 0..planes[channel].len() {
            let original = planes[channel][index];
            let delta = original - blurred[index];
            next[channel][index] = if delta.abs() < 3.0 {
                original
            } else {
                original + delta * 1.5
            };
        }
    }
    write_planes(image, next)
}

pub fn auto_contrast(image: &RgbaImage) -> RgbaImage {
    let planes = channel_planes(image);
    let mut next = planes.clone();
    for channel in 0..3 {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for sample in &planes[channel] {
            min = min.min(*sample);
            max = max.max(*sample);
        }
        if max > min {
            for (dest, sample) in next[channel].iter_mut().zip(&planes[channel]) {
                *dest = (sample - min) * 255.0 / (max - min);
            }
        }
    }
    write_planes(image, next)
}

macro_rules! factor_transform {
    ($name:ident, $id:literal, $label:literal, $order:expr, $group:expr, $shortcut:expr, $call:path) => {
        pub struct $name;

        impl Transform for $name {
            fn id(&self) -> TransformId {
                TransformId::new($id)
            }
            fn label(&self) -> &'static str {
                $label
            }
            fn shortcut(&self) -> Option<Shortcut> {
                $shortcut
            }
            fn group(&self) -> MenuGroup {
                $group
            }
            fn order(&self) -> u8 {
                $order
            }
            fn apply(
                &self,
                image: &RgbaImage,
                params: &Params,
            ) -> Result<RgbaImage, TransformError> {
                let Params::Factor(factor) = params else {
                    return Err(TransformError::BadParams);
                };
                Ok($call(image, *factor))
            }
        }
    };
}

factor_transform!(
    Colour,
    "colour",
    "Colour",
    0,
    MenuGroup::Adjust,
    Some(Shortcut::new(ShortcutKey::Right).shift()),
    apply_colour
);
factor_transform!(
    Contrast,
    "contrast",
    "Contrast",
    1,
    MenuGroup::Adjust,
    Some(Shortcut::new(ShortcutKey::Right).option()),
    apply_contrast
);
factor_transform!(
    Brightness,
    "brightness",
    "Brightness",
    2,
    MenuGroup::Adjust,
    Some(Shortcut::new(ShortcutKey::Right).command()),
    apply_brightness
);

pub struct BlackAndWhite;

impl Transform for BlackAndWhite {
    fn id(&self) -> TransformId {
        TransformId::new("black-and-white")
    }
    fn label(&self) -> &'static str {
        "Black and White"
    }
    fn shortcut(&self) -> Option<Shortcut> {
        Some(Shortcut::new(ShortcutKey::Char('w')).option())
    }
    fn group(&self) -> MenuGroup {
        MenuGroup::Adjust
    }
    fn order(&self) -> u8 {
        3
    }
    fn apply(&self, image: &RgbaImage, _: &Params) -> Result<RgbaImage, TransformError> {
        Ok(apply_colour(image, 0.0))
    }
}

pub struct UnsharpMask;

impl Transform for UnsharpMask {
    fn id(&self) -> TransformId {
        TransformId::new("unsharp-mask")
    }
    fn label(&self) -> &'static str {
        "Unsharp Mask"
    }
    fn shortcut(&self) -> Option<Shortcut> {
        Some(Shortcut::new(ShortcutKey::Char('u')).option())
    }
    fn group(&self) -> MenuGroup {
        MenuGroup::Filter
    }
    fn order(&self) -> u8 {
        12
    }
    fn apply(&self, image: &RgbaImage, _: &Params) -> Result<RgbaImage, TransformError> {
        Ok(unsharp(image))
    }
}

pub struct AutoContrast;

impl Transform for AutoContrast {
    fn id(&self) -> TransformId {
        TransformId::new("auto-contrast")
    }
    fn label(&self) -> &'static str {
        "Auto Contrast"
    }
    fn shortcut(&self) -> Option<Shortcut> {
        Some(Shortcut::new(ShortcutKey::Char('a')).option())
    }
    fn group(&self) -> MenuGroup {
        MenuGroup::Filter
    }
    fn order(&self) -> u8 {
        13
    }
    fn apply(&self, image: &RgbaImage, _: &Params) -> Result<RgbaImage, TransformError> {
        Ok(auto_contrast(image))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> RgbaImage {
        let mut image = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                image.put_pixel(
                    x,
                    y,
                    Rgba([(x * 30) as u8, (y * 20) as u8, ((x + y) * 10) as u8, 128]),
                );
            }
        }
        image
    }

    fn reference_blend(
        image: &RgbaImage,
        degenerate: impl Fn(f32, f32, f32, [f32; 3]) -> [f32; 3],
        factor: f32,
    ) -> Vec<u8> {
        let mut sums = [0.0; 3];
        let count = image.pixels().len() as f32;
        for pixel in image.pixels() {
            for channel in 0..3 {
                sums[channel] += pixel[channel] as f32;
            }
        }
        let mean = [sums[0] / count, sums[1] / count, sums[2] / count];
        let mut out = Vec::new();
        for pixel in image.pixels() {
            let sample = [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32];
            let base = degenerate(sample[0], sample[1], sample[2], mean);
            for channel in 0..3 {
                out.push(round_channel(
                    base[channel] * (1.0 - factor) + sample[channel] * factor,
                ));
            }
        }
        out
    }

    fn rgb_bytes(image: &RgbaImage) -> Vec<u8> {
        image
            .pixels()
            .flat_map(|pixel| pixel.0[..3].to_vec())
            .collect()
    }

    #[test]
    fn enhancers_match_the_formula() {
        let source = image();
        let colour = apply_colour(&source, 0.5);
        let expected = reference_blend(
            &source,
            |red, green, blue, _| {
                let gray = luminance(red, green, blue);
                [gray, gray, gray]
            },
            0.5,
        );
        assert_eq!(rgb_bytes(&colour), expected);
        assert!(colour.pixels().all(|pixel| pixel[3] == 128));

        let contrast = apply_contrast(&source, 1.1);
        let expected = reference_blend(&source, |_r, _g, _b, mean| mean, 1.1);
        assert_eq!(rgb_bytes(&contrast), expected);

        let bright = apply_brightness(&source, 0.9);
        let expected = reference_blend(&source, |_, _, _, _| [0.0, 0.0, 0.0], 0.9);
        assert_eq!(rgb_bytes(&bright), expected);
    }

    #[test]
    fn auto_contrast_stretches_and_leaves_flat_channels() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([10, 40, 7, 255]));
        source.put_pixel(1, 0, Rgba([20, 40, 9, 255]));
        let out = auto_contrast(&source);
        assert_eq!(out.get_pixel(0, 0).0, [0, 40, 0, 255]);
        assert_eq!(out.get_pixel(1, 0).0, [255, 40, 255, 255]);
    }

    #[test]
    fn unsharp_uses_sigma_two_and_the_threshold() {
        let mut source = RgbaImage::new(1, 1);
        source.put_pixel(0, 0, Rgba([80, 10, 200, 255]));
        let blurred = gaussian_blur_channel(&[80.0], 1, 1, 2.0)[0];
        let delta = 80.0 - blurred;
        let expected = if delta.abs() < 3.0 {
            80.0
        } else {
            80.0 + delta * 1.5
        };
        let out = unsharp(&source);
        assert_eq!(out.get_pixel(0, 0).0[0], round_channel(expected));
        assert_eq!(out.get_pixel(0, 0).0[3], 255);
    }

    #[test]
    fn a_slider_payload_is_not_a_factor_step() {
        let image = RgbaImage::new(1, 1);
        let error = Colour.apply(
            &image,
            &Params::Adjust {
                colour: 1.0,
                contrast: 1.0,
                brightness: 1.0,
            },
        );
        assert!(error.is_err());
    }
}
