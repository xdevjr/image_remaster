mod commands;
pub mod processing;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use ort::environment::Environment;
use ort::session::Session;
use tauri::Manager;

pub struct AppState {
    pub cancel: Arc<AtomicBool>,
    pub env: Environment,
    /// Sessões ONNX carregadas, por id de modelo.
    pub sessions: Arc<Mutex<HashMap<String, Session>>>,
    pub cache_dir: PathBuf,
    /// Caminhos dos modelos disponíveis, por id.
    pub models: HashMap<String, PathBuf>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    processing::esrgan::prepare_backend();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let env = ort::init()
                .with_name("image-remaster")
                .with_telemetry(false)
                .build()?;

            let cache_dir = app
                .path()
                .app_cache_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("image-remaster"));
            let _ = std::fs::create_dir_all(&cache_dir);

            // Remove resultados antigos do cache.
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with("remaster-") {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }

            let models = processing::resolve_models(app.handle());
            processing::esrgan::initialize_backend();

            app.manage(AppState {
                cancel: Arc::new(AtomicBool::new(false)),
                env,
                sessions: Arc::new(Mutex::new(HashMap::new())),
                cache_dir,
                models,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_image,
            commands::upscale_image,
            commands::upscale_flow,
            commands::save_image,
            commands::cancel_job,
            commands::runtime_info,
            commands::startup_info,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
