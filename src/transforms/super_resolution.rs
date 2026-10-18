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
    let attempts = [
        (vec![1usize, height, width, 3], true),
        (vec![1usize, 3, height, width], false),
    ];
    let mut last = String::from("the super-resolution model rejected the image");
    for (shape, nhwc) in attempts {
        match run_model(path, image, &shape, nhwc, scale) {
            Ok(upscaled) => return Ok(upscaled),
            Err(error) => last = error,
        }
    }
    Err(TransformError::Failed(last))
}

fn run_model(
    path: &std::path::Path,
    image: &RgbaImage,
    shape: &[usize],
    nhwc: bool,
    scale: u8,
) -> Result<RgbaImage, String> {
    let tensor = input_tensor(image, shape, nhwc)?;
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|error| error.to_string())?
        .with_input_fact(0, f32::fact(shape).into())
        .map_err(|error| error.to_string())?
        .into_optimized()
        .map_err(|error| error.to_string())?
        .into_runnable()
        .map_err(|error| error.to_string())?;
    let outputs = model
        .run(tvec!(tensor.into()))
        .map_err(|error| error.to_string())?;
    let output = outputs
        .into_iter()
        .next()
        .ok_or_else(|| "the model returned no tensors".to_string())?;
    tensor_to_image(&output, image, scale, nhwc)
}

fn input_tensor(image: &RgbaImage, shape: &[usize], nhwc: bool) -> Result<Tensor, String> {
    let mut data = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    if nhwc {
        for pixel in image.pixels() {
            data.push(pixel[2] as f32);
            data.push(pixel[1] as f32);
            data.push(pixel[0] as f32);
        }
    } else {
        for channel in [2_usize, 1, 0] {
            for pixel in image.pixels() {
                data.push(pixel[channel] as f32);
            }
        }
    }
    Tensor::from_shape(shape, &data).map_err(|error| error.to_string())
}

fn tensor_to_image(
    tensor: &Tensor,
    source: &RgbaImage,
    scale: u8,
    nhwc_hint: bool,
) -> Result<RgbaImage, String> {
    let view = tensor
        .to_array_view::<f32>()
        .map_err(|error| error.to_string())?;
    let shape = view.shape();
    let (height, width, nhwc) = match shape {
        [_, h, w, 3] => (*h, *w, true),
        [_, 3, h, w] => (*h, *w, false),
        _ => {
            return Err(format!("unexpected model output shape {shape:?}"));
        }
    };
    let expected_w = source.width() as usize * scale as usize;
    let expected_h = source.height() as usize * scale as usize;
    if width != expected_w || height != expected_h {
        return Err(format!(
            "model output {width}x{height} is not {expected_w}x{expected_h}"
        ));
    }
    let _ = nhwc_hint;
    let mut max = 0.0_f32;
    view.iter().for_each(|sample| max = max.max(*sample));
    let gain = if max <= 1.5 { 255.0 } else { 1.0 };
    let mut out = RgbaImage::new(width as u32, height as u32);
    for y in 0..height {
        for x in 0..width {
            let (blue, green, red) = if nhwc {
                (view[[0, y, x, 0]], view[[0, y, x, 1]], view[[0, y, x, 2]])
            } else {
                (view[[0, 0, y, x]], view[[0, 1, y, x]], view[[0, 2, y, x]])
            };
            let alpha = nearest_alpha(source, x, y, width, height);
            out.put_pixel(
                x as u32,
                y as u32,
                Rgba([
                    super::round_channel(red * gain),
                    super::round_channel(green * gain),
                    super::round_channel(blue * gain),
                    alpha,
                ]),
            );
        }
    }
    Ok(out)
}

fn nearest_alpha(source: &RgbaImage, x: usize, y: usize, width: usize, height: usize) -> u8 {
    let sx = ((x as f64 + 0.5) * source.width() as f64 / width as f64).floor() as u32;
    let sy = ((y as f64 + 0.5) * source.height() as f64 / height as f64).floor() as u32;
    let sx = sx.min(source.width().saturating_sub(1));
    let sy = sy.min(source.height().saturating_sub(1));
    source.get_pixel(sx, sy)[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_leaves_the_caller_with_an_error() {
        let image = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 4]));
        let error = SuperResolution
            .apply(&image, &Params::Scale(4))
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        let path = model_path(4);
        if path.is_file() {
            return;
        }
        assert!(error.contains("missing"));
    }
}
