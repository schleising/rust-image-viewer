use super::{MenuGroup, Params, Shortcut, ShortcutKey, Transform, TransformError, TransformId};
use image::{Rgba, RgbaImage};

const PATCH: i32 = 3;
const SEARCH: i32 = 10;
const STRENGTH: f32 = 3.0;

pub struct Denoise;

impl Transform for Denoise {
    fn id(&self) -> TransformId {
        TransformId::new("denoise")
    }
    fn label(&self) -> &'static str {
        "Denoise"
    }
    fn shortcut(&self) -> Option<Shortcut> {
        Some(Shortcut::new(ShortcutKey::Char('n')).option())
    }
    fn group(&self) -> MenuGroup {
        MenuGroup::Advanced
    }
    fn order(&self) -> u8 {
        14
    }
    fn apply(&self, image: &RgbaImage, _: &Params) -> Result<RgbaImage, TransformError> {
        Ok(denoise(image))
    }
}

pub fn denoise(image: &RgbaImage) -> RgbaImage {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut labs = Vec::with_capacity(width * height);
    let mut alpha = Vec::with_capacity(width * height);
    for pixel in image.pixels() {
        labs.push(rgb_to_lab(pixel[0], pixel[1], pixel[2]));
        alpha.push(pixel[3]);
    }
    let mut planes = [Vec::new(), Vec::new(), Vec::new()];
    for lab in &labs {
        planes[0].push(lab.0);
        planes[1].push(lab.1);
        planes[2].push(lab.2);
    }
    for plane in &mut planes {
        *plane = nlmeans(plane, width, height, STRENGTH);
    }
    let mut out = RgbaImage::new(image.width(), image.height());
    for index in 0..width * height {
        let (red, green, blue) = lab_to_rgb(planes[0][index], planes[1][index], planes[2][index]);
        let x = (index % width) as u32;
        let y = (index / width) as u32;
        out.put_pixel(x, y, Rgba([red, green, blue, alpha[index]]));
    }
    out
}

fn nlmeans(src: &[f32], width: usize, height: usize, strength: f32) -> Vec<f32> {
    let h2 = strength * strength;
    if h2 == 0.0 {
        return src.to_vec();
    }
    let mut out = vec![0.0; src.len()];
    let mut weights = vec![0.0; src.len()];
    for dy in -SEARCH..=SEARCH {
        for dx in -SEARCH..=SEARCH {
            let mut diff = vec![0.0_f32; src.len()];
            for y in 0..height {
                for x in 0..width {
                    let other_x = clamp_index(x as i32 + dx, width);
                    let other_y = clamp_index(y as i32 + dy, height);
                    let delta = src[y * width + x] - src[other_y * width + other_x];
                    diff[y * width + x] = delta * delta;
                }
            }
            let integral = integral_image(&diff, width, height);
            for y in 0..height {
                for x in 0..width {
                    let (sum, count) = window_sum(&integral, width, height, x, y, PATCH);
                    if count == 0 {
                        continue;
                    }
                    let distance = sum / count as f32;
                    let weight = (-((distance - 2.0 * h2).max(0.0)) / h2).exp();
                    let other_x = clamp_index(x as i32 + dx, width);
                    let other_y = clamp_index(y as i32 + dy, height);
                    let index = y * width + x;
                    out[index] += weight * src[other_y * width + other_x];
                    weights[index] += weight;
                }
            }
        }
    }
    for index in 0..out.len() {
        if weights[index] > 0.0 {
            out[index] /= weights[index];
        } else {
            out[index] = src[index];
        }
    }
    out
}

fn clamp_index(value: i32, length: usize) -> usize {
    value.clamp(0, length as i32 - 1) as usize
}

fn integral_image(samples: &[f32], width: usize, height: usize) -> Vec<f32> {
    let stride = width + 1;
    let mut integral = vec![0.0; stride * (height + 1)];
    for y in 0..height {
        let mut row = 0.0;
        for x in 0..width {
            row += samples[y * width + x];
            integral[(y + 1) * stride + x + 1] = integral[y * stride + x + 1] + row;
        }
    }
    integral
}

fn window_sum(
    integral: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    radius: i32,
) -> (f32, usize) {
    let x0 = (x as i32 - radius).max(0) as usize;
    let y0 = (y as i32 - radius).max(0) as usize;
    let x1 = (x as i32 + radius).min(width as i32 - 1) as usize;
    let y1 = (y as i32 + radius).min(height as i32 - 1) as usize;
    let stride = width + 1;
    let sum = integral[(y1 + 1) * stride + x1 + 1]
        - integral[y0 * stride + x1 + 1]
        - integral[(y1 + 1) * stride + x0]
        + integral[y0 * stride + x0];
    let count = (x1 - x0 + 1) * (y1 - y0 + 1);
    (sum, count)
}

fn srgb_to_linear(channel: f32) -> f32 {
    let value = channel / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(channel: f32) -> u8 {
    let value = if channel <= 0.0031308 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    };
    super::round_channel(value * 255.0)
}

fn f_lab(value: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    if value > DELTA * DELTA * DELTA {
        value.cbrt()
    } else {
        value / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

fn f_lab_inv(value: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    if value > DELTA {
        value.powi(3)
    } else {
        3.0 * DELTA * DELTA * (value - 4.0 / 29.0)
    }
}

#[allow(clippy::excessive_precision)]
fn rgb_to_lab(red: u8, green: u8, blue: u8) -> (f32, f32, f32) {
    let red = srgb_to_linear(red as f32);
    let green = srgb_to_linear(green as f32);
    let blue = srgb_to_linear(blue as f32);
    let x = (0.4124564 * red + 0.3575761 * green + 0.1804375 * blue) / 0.95047;
    let y = 0.2126729 * red + 0.7151522 * green + 0.0721750 * blue;
    let z = (0.0193339 * red + 0.1191920 * green + 0.9503041 * blue) / 1.08883;
    let fx = f_lab(x);
    let fy = f_lab(y);
    let fz = f_lab(z);
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

#[allow(clippy::excessive_precision)]
fn lab_to_rgb(l: f32, a: f32, b: f32) -> (u8, u8, u8) {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let x = f_lab_inv(fx) * 0.95047;
    let y = f_lab_inv(fy);
    let z = f_lab_inv(fz) * 1.08883;
    let red = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let green = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let blue = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    (
        linear_to_srgb(red),
        linear_to_srgb(green),
        linear_to_srgb(blue),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denoise_output_is_stable() {
        let mut image = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                let noise = ((x * 17 + y * 13) % 11) as u8;
                image.put_pixel(x, y, Rgba([40 + noise, 90, 140, 255]));
            }
        }
        let out = denoise(&image);
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for pixel in out.pixels() {
            for channel in pixel.0 {
                hash ^= channel as u64;
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        }
        assert_eq!(
            hash, 0x1316_8433_283f_c650,
            "denoise fixture drifted; recorded hash {hash:#x}"
        );
    }
}
