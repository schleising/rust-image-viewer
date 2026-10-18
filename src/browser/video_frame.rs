use image::RgbaImage;
use objc2::rc::autoreleasepool;
use objc2_av_foundation::{AVAsset, AVAssetImageGenerator};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextGetData, CGColorSpace, CGContext, CGImage,
    CGImageAlphaInfo, CGImageByteOrderInfo,
};
use objc2_core_media::CMTime;
use objc2_foundation::{NSString, NSURL};
use std::path::Path;
use std::ptr;

const SAMPLE_FRACTION: f64 = 0.15;

pub(crate) fn sample_seconds(duration: f64) -> f64 {
    if duration.is_finite() && duration > 0.0 {
        duration * SAMPLE_FRACTION
    } else {
        0.0
    }
}

pub(crate) fn seek_tolerance(duration: f64) -> f64 {
    if duration.is_finite() && duration > 0.0 {
        (duration * 0.05).clamp(0.25, 2.0)
    } else {
        0.5
    }
}

pub(crate) fn frame(path: &Path, max_px: u32) -> Result<RgbaImage, String> {
    let Some(text) = path.to_str() else {
        return Err("the video path is not valid text".into());
    };
    let text = text.to_string();
    autoreleasepool(|_| read_frame(&text, max_px))
}

fn read_frame(path: &str, max_px: u32) -> Result<RgbaImage, String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    let image = unsafe {
        let asset = AVAsset::assetWithURL(&url);
        let duration = asset.duration().seconds();
        let at = CMTime::with_seconds(sample_seconds(duration), 600);
        let tolerance = CMTime::with_seconds(seek_tolerance(duration), 600);
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        let side = max_px.max(1) as f64;
        generator.setMaximumSize(CGSize::new(side, side));
        generator.setRequestedTimeToleranceBefore(tolerance);
        generator.setRequestedTimeToleranceAfter(tolerance);
        copy_frame(&generator, at)?
    };
    rgba_image(&image)
}

#[allow(deprecated)]
unsafe fn copy_frame(
    generator: &AVAssetImageGenerator,
    at: CMTime,
) -> Result<objc2::rc::Retained<CGImage>, String> {
    unsafe { generator.copyCGImageAtTime_actualTime_error(at, ptr::null_mut()) }
        .map_err(|error| error.localizedDescription().to_string())
}

fn rgba_image(image: &CGImage) -> Result<RgbaImage, String> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    if width == 0 || height == 0 || width > u32::MAX as usize || height > u32::MAX as usize {
        return Err("the video frame is empty".into());
    }
    let row = width
        .checked_mul(4)
        .ok_or_else(|| "the video frame is too large".to_string())?;
    let space = CGColorSpace::new_device_rgb()
        .ok_or_else(|| "could not make a colour space".to_string())?;
    let info = CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0;
    let context = unsafe {
        CGBitmapContextCreate(ptr::null_mut(), width, height, 8, row, Some(&space), info)
    }
    .ok_or_else(|| "could not draw the video frame".to_string())?;
    let rect = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(width as f64, height as f64),
    );
    CGContext::draw_image(Some(&context), rect, Some(image));
    let data = CGBitmapContextGetData(Some(&context));
    if data.is_null() {
        return Err("could not read the video frame".into());
    }
    let len = row
        .checked_mul(height)
        .ok_or_else(|| "the video frame is too large".to_string())?;
    let mut pixels = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) }.to_vec();
    unpremultiply(&mut pixels);
    RgbaImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| "could not store the video frame".to_string())
}

fn unpremultiply(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3];
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u16 * 255) / alpha as u16) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sample_seconds, seek_tolerance};

    #[test]
    fn sample_is_fifteen_percent_of_a_real_duration() {
        assert_eq!(sample_seconds(100.0), 15.0);
        assert_eq!(sample_seconds(0.0), 0.0);
        assert_eq!(sample_seconds(f64::NAN), 0.0);
        assert_eq!(sample_seconds(f64::INFINITY), 0.0);
    }

    #[test]
    fn tolerance_stays_near_the_sample() {
        assert_eq!(seek_tolerance(10.0), 0.5);
        assert_eq!(seek_tolerance(100.0), 2.0);
        assert_eq!(seek_tolerance(1.0), 0.25);
        assert_eq!(seek_tolerance(f64::NAN), 0.5);
    }

    #[test]
    fn frame_is_taken_fifteen_percent_through() -> Result<(), String> {
        let ffmpeg = matches!(
            std::process::Command::new("ffmpeg").arg("-version").output(),
            Ok(output) if output.status.success()
        );
        if !ffmpeg {
            return Ok(());
        }
        let dir = std::env::temp_dir().join(format!("image-viewer-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let path = dir.join("clip.mp4");
        let built = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=40x40:r=25:d=1",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=40x40:r=25:d=9,geq=r='if(lt(Y,H/2),255,0)':g=0:b='if(lt(Y,H/2),0,255)'",
                "-filter_complex",
                "[0:v][1:v]concat=n=2:v=1:a=0",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&path)
            .status()
            .map_err(|error| error.to_string())?;
        if !built.success() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err("ffmpeg could not build the sample".into());
        }
        let image = match super::frame(&path, 40) {
            Ok(image) => image,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(error);
            }
        };
        let _ = std::fs::remove_dir_all(&dir);
        let top = image.get_pixel(image.width() / 2, image.height() / 8);
        let bottom = image.get_pixel(image.width() / 2, image.height() * 7 / 8);
        if top[0] < 180 || top[2] > 80 {
            return Err(format!("top of the frame was {top:?}"));
        }
        if bottom[2] < 180 || bottom[0] > 80 {
            return Err(format!("bottom of the frame was {bottom:?}"));
        }
        Ok(())
    }
}
