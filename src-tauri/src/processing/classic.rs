use std::sync::atomic::{AtomicBool, Ordering};

use image::imageops::FilterType;
use image::RgbImage;

fn apply_unsharp(resized: &mut RgbImage) {
    let blurred = image::imageops::blur(resized, 1.1);
    let amount = 0.55f32;

    for (pixel, blur_pixel) in resized.pixels_mut().zip(blurred.pixels()) {
        for channel in 0..3 {
            let original = pixel[channel] as f32;
            let blur = blur_pixel[channel] as f32;
            pixel[channel] = (original + amount * (original - blur)).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Redimensiona para uma resolução exata (mantendo ou não a proporção é
/// responsabilidade de quem chama), com o mesmo unsharp do motor clássico.
pub fn resize(
    src: &RgbImage,
    width: u32,
    height: u32,
    cancel: &AtomicBool,
) -> Result<RgbImage, String> {
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

    let (width, height) = (width.max(1), height.max(1));
    let mut resized = image::imageops::resize(src, width, height, FilterType::Lanczos3);

    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

    apply_unsharp(&mut resized);
    Ok(resized)
}

/// Upscale clássico: Lanczos3 + unsharp mask.
pub fn upscale(src: &RgbImage, scale: u32, cancel: &AtomicBool) -> Result<RgbImage, String> {
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

    let (width, height) = src.dimensions();
    let new_width = width * scale;
    let new_height = height * scale;

    let mut resized = if scale == 1 {
        src.clone()
    } else {
        image::imageops::resize(src, new_width, new_height, FilterType::Lanczos3)
    };

    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

    apply_unsharp(&mut resized);
    Ok(resized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanczos_upscale_doubles_dimensions() {
        let source = RgbImage::from_pixel(16, 12, image::Rgb([40, 80, 120]));
        let output = upscale(&source, 2, &AtomicBool::new(false)).expect("upscale clássico");
        assert_eq!(output.dimensions(), (32, 24));
    }
}
