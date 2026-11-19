pub mod classic;
pub mod esrgan;
pub mod worker;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use worker::run_job;

/// Modelo usado quando a UI não especifica nenhum.
pub const DEFAULT_MODEL: &str = "qualidade";

/// Modelo leve padrão (mantido para compatibilidade de caminhos/testes).
pub const MODEL_FILE: &str = "realesr-general-x4v3.onnx";

pub struct ModelDef {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub description: &'static str,
}

pub const MODELS: &[ModelDef] = &[
    ModelDef {
        id: "rapido",
        label: "Rápido",
        file: "realesr-general-x4v3.onnx",
        description: "Real-ESRGAN x4v3 · leve e veloz",
    },
    ModelDef {
        id: "qualidade",
        label: "Qualidade",
        file: "RealESRGAN_x4plus_fp32.onnx",
        description: "Real-ESRGAN x4plus · melhor equilíbrio para fotos",
    },
    ModelDef {
        id: "nitido",
        label: "Nítido",
        file: "4x-UltraSharp-fp32-opset17.onnx",
        description: "4x-UltraSharp · mais textura e nitidez",
    },
    ModelDef {
        id: "anime",
        label: "Anime",
        file: "RealESRGAN_x4plus_anime_6B_fp32.onnx",
        description: "Real-ESRGAN anime · ilustrações e anime",
    },
];

pub fn model_by_id(id: &str) -> Option<&'static ModelDef> {
    MODELS.iter().find(|model| model.id == id)
}

fn candidates_for(app: &tauri::AppHandle, file: &str) -> Vec<PathBuf> {
    use tauri::Manager;

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(dir) = std::env::var("IMAGE_REMASTER_MODELS_DIR") {
        candidates.push(PathBuf::from(dir).join(file));
    }

    // Compatibilidade: override direto do modelo "Qualidade".
    if Some(file) == model_by_id(DEFAULT_MODEL).map(|model| model.file) {
        if let Ok(path) = std::env::var("IMAGE_REMASTER_MODEL") {
            candidates.push(PathBuf::from(path));
        }
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("models").join(file));
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("models").join(file));
        candidates.push(cwd.join("..").join("models").join(file));
    }

    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        candidates.push(Path::new(manifest_dir).join("..").join("models").join(file));
    }

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".local/share/image-remaster/models")
                .join(file),
        );
    }

    candidates
}

/// Procura um arquivo de modelo em locais previsíveis (dev e produção).
pub fn resolve_model_path(app: &tauri::AppHandle, file: &str) -> Option<PathBuf> {
    candidates_for(app, file)
        .into_iter()
        .find(|path| path.exists())
}

/// Resolve todos os modelos instalados.
pub fn resolve_models(app: &tauri::AppHandle) -> HashMap<String, PathBuf> {
    MODELS
        .iter()
        .filter_map(|model| {
            resolve_model_path(app, model.file).map(|path| (model.id.to_string(), path))
        })
        .collect()
}
