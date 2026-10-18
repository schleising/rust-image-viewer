use super::{
    round_channel, MenuGroup, Params, Shortcut, ShortcutKey, Transform, TransformError, TransformId,
};
use image::{Rgba, RgbaImage};

struct Kernel {
    size: i32,
    weights: &'static [i32],
    scale: f32,
    offset: f32,
}

const SHARPEN: Kernel = Kernel {
    size: 3,
    weights: &[-2, -2, -2, -2, 32, -2, -2, -2, -2],
    scale: 16.0,
    offset: 0.0,
};
const BLUR: Kernel = Kernel {
    size: 5,
    weights: &[
        1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1,
    ],
    scale: 16.0,
    offset: 0.0,
};
const CONTOUR: Kernel = Kernel {
    size: 3,
    weights: &[-1, -1, -1, -1, 8, -1, -1, -1, -1],
    scale: 1.0,
    offset: 255.0,
};
const DETAIL: Kernel = Kernel {
    size: 3,
    weights: &[0, -1, 0, -1, 10, -1, 0, -1, 0],
    scale: 6.0,
    offset: 0.0,
};
const EDGE: Kernel = Kernel {
    size: 3,
    weights: &[-1, -1, -1, -1, 10, -1, -1, -1, -1],
    scale: 2.0,
    offset: 0.0,
};
const EMBOSS: Kernel = Kernel {
    size: 3,
    weights: &[-1, 0, 0, 0, 1, 0, 0, 0, 0],
    scale: 1.0,
    offset: 128.0,
};
const FIND_EDGES: Kernel = Kernel {
    size: 3,
    weights: &[-1, -1, -1, -1, 8, -1, -1, -1, -1],
    scale: 1.0,
    offset: 0.0,
};
const SMOOTH: Kernel = Kernel {
    size: 3,
    weights: &[1, 1, 1, 1, 5, 1, 1, 1, 1],
    scale: 13.0,
    offset: 0.0,
};

fn convolve(image: &RgbaImage, kernel: &Kernel) -> RgbaImage {
    let width = image.width() as i32;
    let height = image.height() as i32;
    let radius = kernel.size / 2;
    let mut out = RgbaImage::new(image.width(), image.height());
    for y in 0..height {
        for x in 0..width {
            let mut sums = [0.0_f32; 3];
            for ky in 0..kernel.size {
                for kx in 0..kernel.size {
                    let sx = x + kx - radius;
                    let sy = y + ky - radius;
                    let weight = kernel.weights[(ky * kernel.size + kx) as usize] as f32;
                    if sx < 0 || sy < 0 || sx >= width || sy >= height {
                        continue;
                    }
                    let pixel = image.get_pixel(sx as u32, sy as u32).0;
                    for channel in 0..3 {
                        sums[channel] += pixel[channel] as f32 * weight;
                    }
                }
            }
            let source = image.get_pixel(x as u32, y as u32).0;
            let mut next = [0_u8; 4];
            for channel in 0..3 {
                next[channel] = round_channel(sums[channel] / kernel.scale + kernel.offset);
            }
            next[3] = source[3];
            out.put_pixel(x as u32, y as u32, Rgba(next));
        }
    }
    out
}

macro_rules! kernel_transform {
    ($name:ident, $id:literal, $label:literal, $order:expr, $kernel:expr, $shortcut:expr) => {
        pub struct $name;

        impl Transform for $name {
            fn id(&self) -> TransformId {
                TransformId::new($id)
            }
            fn label(&self) -> &'static str {
                $label
            }
            fn shortcut(&self) -> Option<Shortcut> {
                Some($shortcut)
            }
            fn group(&self) -> MenuGroup {
                MenuGroup::Filter
            }
            fn order(&self) -> u8 {
                $order
            }
            fn apply(&self, image: &RgbaImage, _: &Params) -> Result<RgbaImage, TransformError> {
                Ok(convolve(image, &$kernel))
            }
        }
    };
}

kernel_transform!(
    Sharpen,
    "sharpen",
    "Sharpen",
    4,
    SHARPEN,
    Shortcut::new(ShortcutKey::Char('s')).option()
);
kernel_transform!(
    Blur,
    "blur",
    "Blur",
    5,
    BLUR,
    Shortcut::new(ShortcutKey::Char('b')).option()
);
kernel_transform!(
    Contour,
    "contour",
    "Contour",
    6,
    CONTOUR,
    Shortcut::new(ShortcutKey::Char('c')).option()
);
kernel_transform!(
    Detail,
    "detail",
    "Detail",
    7,
    DETAIL,
    Shortcut::new(ShortcutKey::Char('d')).option()
);
kernel_transform!(
    EdgeEnhance,
    "edge-enhance",
    "Edge Enhance",
    8,
    EDGE,
    Shortcut::new(ShortcutKey::Char('e')).option()
);
kernel_transform!(
    Emboss,
    "emboss",
    "Emboss",
    9,
    EMBOSS,
    Shortcut::new(ShortcutKey::Char('m')).option()
);
kernel_transform!(
    FindEdges,
    "find-edges",
    "Find Edges",
    10,
    FIND_EDGES,
    Shortcut::new(ShortcutKey::Char('f')).option()
);
kernel_transform!(
    Smooth,
    "smooth",
    "Smooth",
    11,
    SMOOTH,
    Shortcut::new(ShortcutKey::Char('o')).option()
);

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(value: u8) -> RgbaImage {
        RgbaImage::from_pixel(1, 1, Rgba([value, value, value, 200]))
    }

    #[test]
    fn one_pixel_kernels_match_the_formula() {
        let image = solid(10);
        let sharpen = convolve(&image, &SHARPEN);
        assert_eq!(sharpen.get_pixel(0, 0).0, [20, 20, 20, 200]);
        let contour = convolve(&image, &CONTOUR);
        assert_eq!(contour.get_pixel(0, 0).0, [255, 255, 255, 200]);
        let emboss = convolve(&image, &EMBOSS);
        assert_eq!(emboss.get_pixel(0, 0).0, [138, 138, 138, 200]);
    }

    fn pattern() -> RgbaImage {
        let mut image = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                let red = (x * 30) as u8;
                let green = (y * 20) as u8;
                let blue = ((x + y) * 10) as u8;
                image.put_pixel(x, y, Rgba([red, green, blue, 255]));
            }
        }
        image
    }

    fn reference(image: &RgbaImage, kernel: &Kernel) -> Vec<u8> {
        let width = image.width() as i32;
        let height = image.height() as i32;
        let radius = kernel.size / 2;
        let mut out = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let mut sums = [0.0_f32; 3];
                for ky in 0..kernel.size {
                    for kx in 0..kernel.size {
                        let sx = x + kx - radius;
                        let sy = y + ky - radius;
                        let weight = kernel.weights[(ky * kernel.size + kx) as usize] as f32;
                        if sx >= 0 && sy >= 0 && sx < width && sy < height {
                            let pixel = image.get_pixel(sx as u32, sy as u32).0;
                            for channel in 0..3 {
                                sums[channel] += pixel[channel] as f32 * weight;
                            }
                        }
                    }
                }
                for sum in sums {
                    out.push(round_channel(sum / kernel.scale + kernel.offset));
                }
            }
        }
        out
    }

    #[test]
    fn eight_by_eight_kernels_match_the_formula() {
        let image = pattern();
        for kernel in [
            &SHARPEN,
            &BLUR,
            &CONTOUR,
            &DETAIL,
            &EDGE,
            &EMBOSS,
            &FIND_EDGES,
            &SMOOTH,
        ] {
            let got = convolve(&image, kernel);
            let mut bytes = Vec::new();
            for pixel in got.pixels() {
                bytes.extend_from_slice(&pixel.0[..3]);
            }
            assert_eq!(bytes, reference(&image, kernel));
        }
    }
}
