use super::{MenuGroup, Params, Shortcut, ShortcutKey, Transform, TransformError, TransformId};
use crate::resources::model_path;
use image::{Rgba, RgbaImage};
use tract_onnx::prelude::*;

pub struct SuperResolution;

impl Transform for SuperResolution {
    fn id(&self) -> TransformId {
        TransformId::new("super-resolution")
    }
    fn label(&self) -> &'static str {
        "Super Resolution"
    }
    fn shortcut(&self) -> Option<Shortcut> {
        Some(Shortcut::new(ShortcutKey::Char('s')).control())
    }
    fn group(&self) -> MenuGroup {
        MenuGroup::Advanced
    }
    fn order(&self) -> u8 {
        15
    }
    fn apply(&self, image: &RgbaImage, params: &Params) -> Result<RgbaImage, TransformError> {
        let Params::Scale(scale) = params else {
            return Err(TransformError::BadParams);
        };
        if !matches!(scale, 2..=4) {
            return Err(TransformError::BadParams);
        }
        let path = model_path(*scale);
        if !path.is_file() {
            return Err(TransformError::MissingModel(path.display().to_string()));
        }
        upscale(&path, image, *scale)
    }
}

fn upscale(
    path: &std::path::Path,
    image: &RgbaImage,
    scale: u8,
) -> Result<RgbaImage, TransformError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    if width == 0 || height == 0 {
        return Err(TransformError::BadParams);
    }
    let (luma, cr, cb) = ycrcb_planes(image);
    let shape = [1usize, 1, height, width];
    let tensor = Tensor::from_shape(&shape, &luma)
        .map_err(|error| TransformError::Failed(error.to_string()))?;
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(failure)?
        .with_input_fact(0, f32::fact(shape).into())
        .map_err(failure)?
        .into_optimized()
        .map_err(failure)?
        .into_runnable()
        .map_err(failure)?;
    let outputs = model.run(tvec!(tensor.into())).map_err(failure)?;
    let output = outputs
        .into_iter()
        .next()
        .ok_or_else(|| TransformError::Failed("the model returned no tensors".into()))?;
    let luma = output_luma(&output, width, height, scale)?;
    let out_width = width * scale as usize;
    let out_height = height * scale as usize;
    let cr = resize_bilinear(&cr, width, height, out_width, out_height);
    let cb = resize_bilinear(&cb, width, height, out_width, out_height);
    let mut out = RgbaImage::new(out_width as u32, out_height as u32);
    for y in 0..out_height {
        for x in 0..out_width {
            let index = y * out_width + x;
            let [red, green, blue] = ycrcb_to_rgb(luma[index] * 255.0, cr[index], cb[index]);
            let alpha = nearest_alpha(image, x, y, out_width, out_height);
            out.put_pixel(x as u32, y as u32, Rgba([red, green, blue, alpha]));
        }
    }
    Ok(out)
}

fn ycrcb_planes(image: &RgbaImage) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut luma = Vec::with_capacity(image.len());
    let mut cr = Vec::with_capacity(image.len());
    let mut cb = Vec::with_capacity(image.len());
    for pixel in image.pixels() {
        let red = pixel[0] as f32;
        let green = pixel[1] as f32;
        let blue = pixel[2] as f32;
        let y = 0.299 * red + 0.587 * green + 0.114 * blue;
        luma.push(y / 255.0);
        cr.push((red - y) * 0.713 + 128.0);
        cb.push((blue - y) * 0.564 + 128.0);
    }
    (luma, cr, cb)
}

fn output_luma(
    tensor: &Tensor,
    width: usize,
    height: usize,
    scale: u8,
) -> Result<Vec<f32>, TransformError> {
    let view = tensor
        .to_array_view::<f32>()
        .map_err(|error| TransformError::Failed(error.to_string()))?;
    let shape = view.shape();
    let (out_height, out_width) = match shape {
        [1, 1, h, w] => (*h, *w),
        _ => {
            return Err(TransformError::Failed(format!(
                "unexpected model output shape {shape:?}"
            )));
        }
    };
    let expected_w = width * scale as usize;
    let expected_h = height * scale as usize;
    if out_width != expected_w || out_height != expected_h {
        return Err(TransformError::Failed(format!(
            "model output {out_width}x{out_height} is not {expected_w}x{expected_h}"
        )));
    }
    Ok(view.iter().copied().collect())
}

fn ycrcb_to_rgb(y: f32, cr: f32, cb: f32) -> [u8; 3] {
    let cr = cr - 128.0;
    let cb = cb - 128.0;
    [
        super::round_channel(y + 1.403 * cr),
        super::round_channel(y - 0.714 * cr - 0.344 * cb),
        super::round_channel(y + 1.773 * cb),
    ]
}

fn resize_bilinear(
    src: &[f32],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<f32> {
    let mut dst = Vec::with_capacity(dst_w * dst_h);
    let x_scale = src_w as f32 / dst_w as f32;
    let y_scale = src_h as f32 / dst_h as f32;
    for y in 0..dst_h {
        let sy = (y as f32 + 0.5) * y_scale - 0.5;
        for x in 0..dst_w {
            let sx = (x as f32 + 0.5) * x_scale - 0.5;
            dst.push(sample(src, src_w, src_h, sx, sy));
        }
    }
    dst
}

fn sample(src: &[f32], width: usize, height: usize, x: f32, y: f32) -> f32 {
    let x = x.clamp(0.0, (width - 1) as f32);
    let y = y.clamp(0.0, (height - 1) as f32);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let p00 = src[y0 * width + x0];
    let p10 = src[y0 * width + x1];
    let p01 = src[y1 * width + x0];
    let p11 = src[y1 * width + x1];
    let top = p00 + (p10 - p00) * tx;
    let bottom = p01 + (p11 - p01) * tx;
    top + (bottom - top) * ty
}

fn nearest_alpha(source: &RgbaImage, x: usize, y: usize, width: usize, height: usize) -> u8 {
    let sx = ((x as f64 + 0.5) * source.width() as f64 / width as f64).floor() as u32;
    let sy = ((y as f64 + 0.5) * source.height() as f64 / height as f64).floor() as u32;
    let sx = sx.min(source.width().saturating_sub(1));
    let sy = sy.min(source.height().saturating_sub(1));
    source.get_pixel(sx, sy)[3]
}

fn failure(error: impl std::fmt::Display) -> TransformError {
    TransformError::Failed(format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsrcnn_quadruples_a_colour_when_the_model_is_present() -> Result<(), TransformError> {
        let image = RgbaImage::from_fn(8, 8, |x, y| {
            Rgba([
                40 + x as u8 * 20,
                30 + y as u8 * 15,
                180,
                if x + y > 8 { 200 } else { 10 },
            ])
        });
        let path = model_path(4);
        if !path.is_file() {
            let error = SuperResolution
                .apply(&image, &Params::Scale(4))
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            assert!(error.contains("missing"));
            return Ok(());
        }
        let out = SuperResolution.apply(&image, &Params::Scale(4))?;
        assert_eq!(out.dimensions(), (32, 32));
        assert_eq!(out.get_pixel(0, 0)[3], 10);
        assert_eq!(out.get_pixel(31, 31)[3], 200);
        let source = image.get_pixel(4, 4);
        let centre = out.get_pixel(18, 18);
        for channel in 0..3 {
            let delta = source[channel] as i16 - centre[channel] as i16;
            assert!(delta.abs() <= 24, "{channel}: {source:?} -> {centre:?}");
        }
        Ok(())
    }
}
