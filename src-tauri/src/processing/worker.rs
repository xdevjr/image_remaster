use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use image::{DynamicImage, GenericImageView, RgbImage};
use ort::environment::Environment;
use ort::session::Session;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::{UpscaleRequest, UpscaleResult};

use super::{classic, esrgan};

/// Limite de megapixels da imagem de entrada (o resultado 4x consome bastante memória).
const MAX_INPUT_MEGAPIXELS: u64 = 6;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub completed: u32,
    pub total: u32,
    pub percent: f32,
    pub elapsed_ms: u64,
}

pub fn emit_progress(app: &AppHandle, progress: Progress) {
    let _ = app.emit("upscale:progress", progress);
}

fn composite_on_white(img: &DynamicImage) -> RgbImage {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut out = RgbImage::new(width, height);

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as f32 / 255.0;
        let blend = |channel: u8| -> u8 {
            (channel as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8
        };
        out.put_pixel(x, y, image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]));
    }

    out
}

pub fn run_job(
    app: &AppHandle,
    request: &UpscaleRequest,
    cache_dir: &Path,
    env: Environment,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    model_id: &str,
    model_path: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<UpscaleResult, String> {
    let started = Instant::now();
    let scale = match request.scale {
        1 => 1,
        2 => 2,
        _ => 4,
    };

    emit_progress(
        app,
        Progress {
            stage: "loading".into(),
            completed: 0,
            total: 1,
            percent: 2.0,
            elapsed_ms: 0,
        },
    );

    let source = image::open(&request.path).map_err(|e| format!("Falha ao abrir imagem: {e}"))?;
    let (width, height) = source.dimensions();

    if width == 0 || height == 0 {
        return Err("A imagem não tem dimensões válidas".into());
    }

    let megapixels = (width as u64 * height as u64) / 1_000_000;
    if megapixels > MAX_INPUT_MEGAPIXELS {
        return Err(format!(
            "Imagem muito grande ({} x {}). O limite é ~{} MP por causa da memória necessária para o upscale.",
            width, height, MAX_INPUT_MEGAPIXELS
        ));
    }

    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

    let rgb = composite_on_white(&source);

    let output = match request.mode.as_str() {
        "classic" => {
            emit_progress(
                app,
                Progress {
                    stage: "upscaling".into(),
                    completed: 0,
                    total: 1,
                    percent: 10.0,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                },
            );
            classic::upscale(&rgb, scale, &cancel)?
        }
        _ => {
            emit_progress(
                app,
                Progress {
                    stage: "model".into(),
                    completed: 0,
                    total: 1,
                    percent: 4.0,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                },
            );

            esrgan::upscale(
                &env,
                &sessions,
                model_id,
                model_path,
                &rgb,
                scale,
                &cancel,
                started,
                |progress| emit_progress(app, progress),
            )?
        }
    };

    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

    emit_progress(
        app,
        Progress {
            stage: "saving".into(),
            completed: 0,
            total: 1,
            percent: 97.0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    );

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let output_path = cache_dir.join(format!("remaster-{timestamp}.png"));

    let (out_width, out_height) = output.dimensions();
    DynamicImage::ImageRgb8(output)
        .save_with_format(&output_path, image::ImageFormat::Png)
        .map_err(|e| format!("Falha ao salvar resultado temporário: {e}"))?;

    let elapsed_ms = started.elapsed().as_millis() as u64;

    emit_progress(
        app,
        Progress {
            stage: "done".into(),
            completed: 1,
            total: 1,
            percent: 100.0,
            elapsed_ms,
        },
    );

    let backend = if request.mode.as_str() == "classic" || !esrgan::cuda_ready() {
        "cpu"
    } else {
        "cuda"
    };

    Ok(UpscaleResult {
        output_path: output_path.to_string_lossy().into_owned(),
        width: out_width,
        height: out_height,
        elapsed_ms,
        mode: request.mode.clone(),
        scale,
        model: if request.mode.as_str() == "classic" {
            "classic".to_string()
        } else {
            model_id.to_string()
        },
        backend: backend.to_string(),
    })
}
