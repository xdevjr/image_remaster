use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use image::imageops::FilterType;
use image::RgbImage;
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;

use super::worker::Progress;

/// Tamanho do bloco de inferência (pixels de entrada).
const TILE: u32 = 256;
/// Sobreposição entre blocos, para evitar emendas visíveis.
const OVERLAP: u32 = 16;
/// Fator nativo do modelo Real-ESRGAN general x4v3.
const MODEL_SCALE: u32 = 4;

const CUDA_UNKNOWN: u8 = 0;
const CUDA_READY: u8 = 1;
const CUDA_DISABLED: u8 = 2;

/// 0 = ainda não verificado, 1 = CUDA disponível, 2 = usando CPU.
static CUDA_STATE: AtomicU8 = AtomicU8::new(CUDA_UNKNOWN);
/// Tile escolhido conforme a VRAM (0 = ainda não detectado).
static GPU_TILE: AtomicU32 = AtomicU32::new(0);
/// VRAM total em MiB (0 = desconhecida).
static GPU_TOTAL_MB: AtomicU32 = AtomicU32::new(0);
/// Último modelo/tamanho processado, para liberar arenas ao mudar de imagem.
static LAST_JOB: Mutex<Option<String>> = Mutex::new(None);

fn thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn default_lib_dir(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    Some(base.join("image-remaster").join(name))
}

fn cuda_dirs() -> Option<(PathBuf, PathBuf)> {
    let cuda = std::env::var_os("IMAGE_REMASTER_CUDA_DIR")
        .map(PathBuf::from)
        .or_else(|| default_lib_dir("cuda"))?;
    let cudnn = std::env::var_os("IMAGE_REMASTER_CUDNN_DIR")
        .map(PathBuf::from)
        .or_else(|| default_lib_dir("cudnn"))?;

    if cuda.is_dir() && cudnn.is_dir() {
        Some((cuda, cudnn))
    } else {
        None
    }
}

fn provider_file_available() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join("libonnxruntime_providers_cuda.so").exists())
        })
        .unwrap_or(false)
}

/// Prepara as libs da GPU e, se necessário, re-executa o processo com
/// `LD_LIBRARY_PATH` apontando para elas (o ORT carrega o provider sozinho).
pub fn prepare_backend() {
    if std::env::var_os("IMAGE_REMASTER_REEXEC").is_some() {
        return;
    }

    let Some((cuda, cudnn)) = cuda_dirs() else {
        return;
    };

    let Some(providers) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .filter(|dir| dir.join("libonnxruntime_providers_cuda.so").exists())
    else {
        return;
    };

    let mut paths = vec![
        providers.to_string_lossy().into_owned(),
        cuda.to_string_lossy().into_owned(),
        cudnn.to_string_lossy().into_owned(),
    ];

    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        paths.push(existing.to_string_lossy().into_owned());
    }

    std::env::set_var("LD_LIBRARY_PATH", paths.join(":"));
    std::env::set_var("IMAGE_REMASTER_REEXEC", "1");

    let Ok(exe) = std::env::current_exe() else {
        return;
    };

    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("[cuda] re-exec falhou: {error}");
}

/// Verifica se há libs CUDA/cuDNN e provider ao lado do executável.
fn ensure_cuda() -> bool {
    match CUDA_STATE.load(Ordering::Relaxed) {
        CUDA_READY => return true,
        CUDA_DISABLED => return false,
        _ => {}
    }

    let ready = cuda_dirs().is_some() && provider_file_available();
    CUDA_STATE.store(
        if ready { CUDA_READY } else { CUDA_DISABLED },
        Ordering::Relaxed,
    );
    ready
}

/// Escolhe um tile seguro conforme a VRAM total (256 MB é conservador em 8 GB).
fn gpu_total_mb() -> Option<u32> {
    let cached = GPU_TOTAL_MB.load(Ordering::Relaxed);
    if cached != 0 {
        return Some(cached);
    }

    let total = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()
        .and_then(|output| {
            let text = String::from_utf8(output.stdout).ok()?;
            text.lines().next()?.trim().parse::<u32>().ok()
        })?;

    GPU_TOTAL_MB.store(total, Ordering::Relaxed);
    Some(total)
}

fn detect_gpu_tile() -> u32 {
    let cached = GPU_TILE.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }

    let detected = gpu_total_mb()
        .map(|total| if total >= 12_000 { 384 } else { 256 })
        .unwrap_or(256);

    GPU_TILE.store(detected, Ordering::Relaxed);
    detected
}

fn build_session(
    env: &Environment,
    model_path: &Path,
    use_cuda: bool,
) -> Result<Session, String> {
    let mut builder = Session::builder(env)
        .map_err(|e| format!("Falha ao criar sessão ONNX: {e}"))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| format!("Falha ao configurar otimização: {e}"))?
        .with_intra_threads(thread_count())
        .map_err(|e| format!("Falha ao configurar threads: {e}"))?;

    if use_cuda {
        let max_workspace = std::env::var("IMAGE_REMASTER_CUDA_MAX_WORKSPACE")
            .map(|value| value != "0")
            .unwrap_or(false);
        let mut cuda = ort::ep::CUDA::default()
            .with_conv_algorithm_search(ort::ep::cuda::ConvAlgorithmSearch::Heuristic)
            .with_tf32(true)
            .with_conv_max_workspace(max_workspace)
            .with_arena_extend_strategy(ort::ep::ArenaExtendStrategy::SameAsRequested);

        if let Some(total_mb) = gpu_total_mb() {
            // Deixa margem para o desktop e evita que a arena cresça sem limite.
            let limit = (total_mb as usize) * 1024 * 1024 * 80 / 100;
            cuda = cuda.with_memory_limit(limit.max(3 * 1024 * 1024 * 1024));
        }

        builder = builder
            .with_execution_providers([cuda.build()])
            .map_err(|e| format!("Falha ao registrar CUDA: {e}"))?;
    }

    builder
        .commit_from_file(model_path)
        .map_err(|e| format!("Falha ao carregar o modelo: {e}"))
}

/// Indica se a GPU está disponível para inferência.
pub fn cuda_ready() -> bool {
    CUDA_STATE.load(Ordering::Relaxed) == CUDA_READY
}

/// Estado legível da GPU: `ready`, `unavailable` ou `unknown`.
pub fn cuda_status() -> &'static str {
    match CUDA_STATE.load(Ordering::Relaxed) {
        CUDA_READY => "ready",
        CUDA_DISABLED => "unavailable",
        _ => "unknown",
    }
}

/// Verifica antecipadamente se CUDA/cuDNN podem ser carregadas.
pub fn initialize_backend() {
    ensure_cuda();
}

fn build_and_cache(
    env: &Environment,
    sessions: &Mutex<HashMap<String, Session>>,
    model_id: &str,
    model_path: &Path,
) -> Result<(), String> {
    // Libera sessões de outros modelos: cada sessão mantém sua própria arena CUDA.
    clear_sessions(sessions);

    let use_cuda = ensure_cuda();
    let session = if use_cuda {
        match build_session(env, model_path, true) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("[cuda] indisponível em runtime, usando CPU: {error}");
                CUDA_STATE.store(CUDA_DISABLED, Ordering::Relaxed);
                build_session(env, model_path, false)?
            }
        }
    } else {
        build_session(env, model_path, false)?
    };

    // Nomes reais vindos do grafo (fallback para os nomes conhecidos).
    let input_names = session
        .inputs()
        .iter()
        .map(|outlet| outlet.name().to_string())
        .collect::<Vec<_>>();
    let output_names = session
        .outputs()
        .iter()
        .map(|outlet| outlet.name().to_string())
        .collect::<Vec<_>>();
    tracing_log(&input_names, &output_names);

    let mut guard = sessions
        .lock()
        .map_err(|_| "Estado do modelo indisponível".to_string())?;
    guard.insert(model_id.to_string(), session);
    Ok(())
}

fn session_for<'a>(
    env: &Environment,
    sessions: &'a Mutex<HashMap<String, Session>>,
    model_id: &str,
    model_path: &Path,
) -> Result<std::sync::MutexGuard<'a, HashMap<String, Session>>, String> {
    let loaded = {
        let guard = sessions
            .lock()
            .map_err(|_| "Estado do modelo indisponível".to_string())?;
        guard.contains_key(model_id)
    };

    if !loaded {
        build_and_cache(env, sessions, model_id, model_path)?;
    }

    sessions
        .lock()
        .map_err(|_| "Estado do modelo indisponível".to_string())
}

fn tracing_log(inputs: &[String], outputs: &[String]) {
    if std::env::var("IMAGE_REMASTER_DEBUG").is_ok() {
        eprintln!("modelo ONNX | entradas: {inputs:?} | saídas: {outputs:?}");
    }
}

fn clear_sessions(sessions: &Mutex<HashMap<String, Session>>) {
    if let Ok(mut guard) = sessions.lock() {
        guard.clear();
    }
}

fn is_out_of_memory(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("failed to allocate memory")
        || lower.contains("available memory of")
        || lower.contains("requested bytes")
        || lower.contains("bfc_arena")
        || lower.contains("out of memory")
        || lower.contains("bad_alloc")
}

fn memory_error(detail: &str) -> String {
    if detail.is_empty() {
        "Memória de GPU insuficiente para esta imagem. Tente escala 1x/2x, o modelo Rápido ou feche aplicativos que usam a GPU.".to_string()
    } else {
        format!(
            "Memória de GPU insuficiente para esta imagem. Tente escala 1x/2x, o modelo Rápido, feche aplicativos que usam a GPU ou reinicie o app. Detalhe: {detail}"
        )
    }
}

/// Executa o Real-ESRGAN em blocos, com retry automático em caso de falta de VRAM.
pub fn upscale<F>(
    env: &Environment,
    sessions: &Mutex<HashMap<String, Session>>,
    model_id: &str,
    model_path: &Path,
    src: &RgbImage,
    target_scale: u32,
    cancel: &AtomicBool,
    started: Instant,
    mut report: F,
) -> Result<RgbImage, String>
where
    F: FnMut(Progress),
{
    initialize_backend();

    // Mantém o modelo carregado entre nós do mesmo fluxo. A troca de modelo
    // libera as arenas CUDA; mudanças de tamanho reaproveitam a sessão e, se
    // faltar VRAM, o retry abaixo libera e recria.
    let changed = {
        let mut guard = LAST_JOB
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let changed = guard.as_deref() != Some(model_id);
        *guard = Some(model_id.to_string());
        changed
    };
    if changed {
        clear_sessions(sessions);
    }

    // Blocos maiores aproveitam melhor a GPU; na CPU, mantém 256 px.
    let default_tile = if cuda_ready() && model_id != "rapido" {
        detect_gpu_tile()
    } else {
        TILE
    };
    let initial = std::env::var("IMAGE_REMASTER_TILE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default_tile)
        .clamp(128, 1024);

    let mut tiles = vec![initial];
    if cuda_ready() {
        for fallback in [192u32, 128u32] {
            if fallback < initial {
                tiles.push(fallback);
            }
        }
    }

    let mut last_error = String::new();

    for tile in tiles {
        // Duas tentativas por tile: a segunda com sessão/arena novas.
        for attempt in 0..2 {
            match upscale_with_tile(
                env,
                sessions,
                model_id,
                model_path,
                src,
                target_scale,
                cancel,
                started,
                tile,
                &mut report,
            ) {
                Ok(image) => return Ok(image),
                Err(error) => {
                    if is_out_of_memory(&error) && cuda_ready() {
                        last_error = error;
                        clear_sessions(sessions);
                        if attempt == 0 {
                            eprintln!(
                                "[cuda] VRAM insuficiente com tile {tile}; liberando sessão e tentando de novo"
                            );
                            report(Progress {
                                stage: "retrying".into(),
                                completed: 0,
                                total: 1,
                                percent: 6.0,
                                elapsed_ms: started.elapsed().as_millis() as u64,
                                step: None,
                                total_steps: None,
                            });
                            continue;
                        }
                        break;
                    }
                    return Err(error);
                }
            }
        }

        if last_error.is_empty() {
            continue;
        }
        eprintln!("[cuda] tentando tile menor após OOM (tile atual: {tile})");
    }

    Err(memory_error(&last_error))
}

fn upscale_with_tile<F>(
    env: &Environment,
    sessions: &Mutex<HashMap<String, Session>>,
    model_id: &str,
    model_path: &Path,
    src: &RgbImage,
    target_scale: u32,
    cancel: &AtomicBool,
    started: Instant,
    tile: u32,
    report: &mut F,
) -> Result<RgbImage, String>
where
    F: FnMut(Progress),
{
    let mut guard = session_for(env, sessions, model_id, model_path)?;
    let session = guard
        .get_mut(model_id)
        .ok_or_else(|| "Sessão ONNX não inicializada".to_string())?;

    let input_name = session
        .inputs()
        .first()
        .map(|input| input.name().to_string())
        .unwrap_or_else(|| "input".to_string());
    let output_name = session
        .outputs()
        .first()
        .map(|output| output.name().to_string())
        .unwrap_or_else(|| "output".to_string());

    let (width, height) = src.dimensions();
    let out_width = width * MODEL_SCALE;
    let out_height = height * MODEL_SCALE;

    let cols = width.div_ceil(tile);
    let rows = height.div_ceil(tile);
    let total = cols.saturating_mul(rows).max(1);
    let mut completed = 0u32;

    let mut out = RgbImage::new(out_width, out_height);

    for ty in 0..rows {
        for tx in 0..cols {
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelado".into());
            }

            let x0 = tx * tile;
            let y0 = ty * tile;
            let x1 = (x0 + tile).min(width);
            let y1 = (y0 + tile).min(height);

            // Expande o bloco com sobreposição.
            let px0 = x0.saturating_sub(OVERLAP);
            let py0 = y0.saturating_sub(OVERLAP);
            let px1 = (x1 + OVERLAP).min(width);
            let py1 = (y1 + OVERLAP).min(height);

            let tile_w = px1 - px0;
            let tile_h = py1 - py0;
            let plane = (tile_w * tile_h) as usize;

            let mut data = vec![0f32; plane * 3];
            for y in 0..tile_h {
                for x in 0..tile_w {
                    let pixel = src.get_pixel(px0 + x, py0 + y);
                    let index = (y * tile_w + x) as usize;
                    data[index] = pixel[0] as f32 / 255.0;
                    data[plane + index] = pixel[1] as f32 / 255.0;
                    data[2 * plane + index] = pixel[2] as f32 / 255.0;
                }
            }

            let tensor = Tensor::from_array((
                [1usize, 3, tile_h as usize, tile_w as usize],
                data,
            ))
            .map_err(|e| format!("Falha ao preparar tensor: {e}"))?;

            let outputs = session
                .run(ort::inputs![input_name.as_str() => tensor])
                .map_err(|e| format!("Falha na inferência: {e}"))?;

            let (shape, output_data) = outputs[output_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Falha ao ler saída do modelo: {e}"))?;

            let out_tile_w = shape[3] as u32;
            let out_tile_h = shape[2] as u32;
            let out_plane = (out_tile_w * out_tile_h) as usize;

            // Região útil do bloco (sem a sobreposição).
            let crop_x = (x0 - px0) * MODEL_SCALE;
            let crop_y = (y0 - py0) * MODEL_SCALE;
            let crop_w = (x1 - x0) * MODEL_SCALE;
            let crop_h = (y1 - y0) * MODEL_SCALE;

            for y in 0..crop_h {
                for x in 0..crop_w {
                    let source_index = ((crop_y + y) * out_tile_w + (crop_x + x)) as usize;
                    let r = (output_data[source_index].clamp(0.0, 1.0) * 255.0).round() as u8;
                    let g = (output_data[out_plane + source_index].clamp(0.0, 1.0) * 255.0).round() as u8;
                    let b = (output_data[out_plane * 2 + source_index].clamp(0.0, 1.0) * 255.0)
                        .round() as u8;
                    out.put_pixel(x0 * MODEL_SCALE + x, y0 * MODEL_SCALE + y, image::Rgb([r, g, b]));
                }
            }

            completed += 1;
            let percent = 8.0 + (completed as f32 / total as f32) * 86.0;
            report(Progress {
                stage: "upscaling".into(),
                completed,
                total,
                percent,
                elapsed_ms: started.elapsed().as_millis() as u64,
                step: None,
                total_steps: None,
            });
        }
    }

    if target_scale != MODEL_SCALE {
        let resized = image::imageops::resize(
            &out,
            width * target_scale,
            height * target_scale,
            FilterType::Lanczos3,
        );
        Ok(resized)
    } else {
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn upscale_tiny_image_produces_4x_output() {
        let model_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("models")
            .join(crate::processing::MODEL_FILE);

        if !model_path.exists() {
            eprintln!("modelo ausente em {} — teste ignorado", model_path.display());
            return;
        }

        let env = ort::init()
            .with_name("image-remaster-test")
            .build()
            .expect("ambiente ONNX");
        let slots = Mutex::new(HashMap::new());
        let cancel = AtomicBool::new(false);

        let source = RgbImage::from_fn(32, 32, |x, y| {
            image::Rgb([(x * 7) as u8, (y * 7) as u8, 128])
        });

        let mut events = 0usize;
        let output = upscale(
            &env,
            &slots,
            "rapido",
            &model_path,
            &source,
            4,
            &cancel,
            Instant::now(),
            |_| events += 1,
        )
        .expect("upscale deve funcionar");

        assert_eq!(output.dimensions(), (128, 128));
        assert!(events >= 1, "deve emitir progresso");
    }
}
