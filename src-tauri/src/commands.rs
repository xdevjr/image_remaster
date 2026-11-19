use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use image::GenericImageView;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::processing;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub file_size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleRequest {
    pub path: String,
    pub mode: String,
    pub scale: u32,
    #[serde(default)]
    pub model: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleResult {
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub elapsed_ms: u64,
    pub mode: String,
    pub scale: u32,
    pub model: String,
    pub backend: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    pub available: bool,
    pub size_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub active: bool,
    pub status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub models: Vec<ModelInfo>,
    pub gpu: GpuInfo,
    pub default_model: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupInfo {
    pub paths: Vec<String>,
    pub auto_run: bool,
    pub model: Option<String>,
    pub scale: Option<u32>,
}

/// Lê as imagens (e flags) passadas na linha de comando.
#[tauri::command]
pub fn startup_info() -> StartupInfo {
    let mut paths = Vec::new();
    let mut auto_run = false;
    let mut model = None;
    let mut scale = None;
    let mut args = std::env::args().skip(1).peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--autorun" => auto_run = true,
            "--model" => model = args.next(),
            "--scale" => scale = args.next().and_then(|value| value.parse::<u32>().ok()),
            _ if arg.starts_with('-') => {}
            _ if std::path::Path::new(&arg).exists() => paths.push(arg),
            _ => {}
        }
    }

    StartupInfo {
        paths,
        auto_run,
        model,
        scale,
    }
}

#[tauri::command]
pub fn load_image(path: String) -> Result<ImageInfo, String> {
    let file_path = PathBuf::from(&path);

    let metadata = std::fs::metadata(&file_path)
        .map_err(|e| format!("Não foi possível acessar o arquivo: {e}"))?;

    let image = image::open(&file_path).map_err(|e| format!("Falha ao abrir imagem: {e}"))?;
    let (width, height) = image.dimensions();

    let format = image::ImageFormat::from_path(&file_path)
        .map(|format| format!("{format:?}").to_lowercase())
        .unwrap_or_else(|_| "desconhecido".to_string());

    Ok(ImageInfo {
        path,
        width,
        height,
        format,
        file_size: metadata.len(),
    })
}

#[tauri::command]
pub async fn upscale_image(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: UpscaleRequest,
) -> Result<UpscaleResult, String> {
    let cancel = state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);

    let model_id = if request.model.is_empty() {
        processing::DEFAULT_MODEL.to_string()
    } else {
        request.model.clone()
    };

    let model_path = state.models.get(&model_id).cloned().ok_or_else(|| {
        format!(
            "Modelo '{}' não encontrado. Rode `mise run model` para baixá-lo.",
            model_id
        )
    })?;

    let cache_dir = state.cache_dir.clone();
    let env = state.env.clone();
    let sessions = state.sessions.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        processing::run_job(
            &app_handle,
            &request,
            &cache_dir,
            env,
            sessions,
            &model_id,
            &model_path,
            cancel,
        )
    })
    .await
    .map_err(|e| format!("A tarefa de upscale falhou: {e}"))?
}

#[tauri::command]
pub fn save_image(source_path: String, dest_path: String) -> Result<String, String> {
    let image = image::open(&source_path).map_err(|e| format!("Falha ao abrir resultado: {e}"))?;
    let destination = PathBuf::from(&dest_path);

    let extension = destination
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "png" => image
            .save_with_format(&destination, image::ImageFormat::Png)
            .map_err(|e| format!("Falha ao salvar PNG: {e}"))?,
        "jpg" | "jpeg" => {
            let rgb = image.to_rgb8();
            let file = std::fs::File::create(&destination)
                .map_err(|e| format!("Falha ao criar arquivo: {e}"))?;
            let mut writer = BufWriter::new(file);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 92);
            encoder
                .encode_image(&rgb)
                .map_err(|e| format!("Falha ao salvar JPEG: {e}"))?;
        }
        "webp" => image
            .save_with_format(&destination, image::ImageFormat::WebP)
            .map_err(|e| format!("Falha ao salvar WebP: {e}"))?,
        _ => {
            return Err(
                "Formato não suportado. Escolha .png, .jpg ou .webp na hora de salvar.".to_string(),
            )
        }
    }

    Ok(dest_path)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn runtime_info(state: State<'_, AppState>) -> RuntimeInfo {
    let models = processing::MODELS
        .iter()
        .map(|model| {
            let path = state.models.get(model.id);
            ModelInfo {
                id: model.id.to_string(),
                label: model.label.to_string(),
                description: model.description.to_string(),
                available: path.is_some(),
                size_bytes: path
                    .and_then(|path| std::fs::metadata(path).ok())
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
            }
        })
        .collect();

    let status = processing::esrgan::cuda_status().to_string();

    RuntimeInfo {
        models,
        gpu: GpuInfo {
            active: status == "ready",
            status,
        },
        default_model: processing::DEFAULT_MODEL.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_image_supports_png_jpeg_and_webp() {
        let dir = std::env::temp_dir().join(format!("image-remaster-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("diretório de teste");

        let source_path = dir.join("source.png");
        image::RgbImage::from_pixel(8, 8, image::Rgb([10, 20, 30]))
            .save(&source_path)
            .expect("imagem de teste");

        let source = source_path.to_string_lossy().into_owned();

        for (name, extension) in [("png", "png"), ("jpeg", "jpg"), ("webp", "webp")] {
            let destination = dir.join(format!("out-{name}.{extension}"));
            save_image(source.clone(), destination.to_string_lossy().into_owned())
                .unwrap_or_else(|error| panic!("falha ao salvar {name}: {error}"));
            assert!(destination.exists(), "{name} não foi criado");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
