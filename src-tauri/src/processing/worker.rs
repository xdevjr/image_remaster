use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use image::{DynamicImage, GenericImageView, RgbImage};
use ort::environment::Environment;
use ort::session::Session;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::{FlowNode, FlowResult, UpscaleRequest, UpscaleResult};

use super::{classic, esrgan};

/// Limite de megapixels da imagem original. O resultado 4x consome bastante
/// memória, mas o tiling controla a VRAM; imagens grandes (ex.: 13 MP) são aceitas.
const MAX_INPUT_MEGAPIXELS: u64 = 32;

/// Chave interna para a imagem original dentro do grafo de saídas.
const SOURCE_KEY: &str = "\u{0}source";

static FLOW_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub completed: u32,
    pub total: u32,
    pub percent: f32,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_steps: Option<u32>,
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

/// Lê a imagem de entrada e valida o limite de megapixels.
fn load_source(path: &str) -> Result<(RgbImage, u32, u32), String> {
    let source = image::open(path).map_err(|e| format!("Falha ao abrir imagem: {e}"))?;
    let (width, height) = source.dimensions();

    if width == 0 || height == 0 {
        return Err("A imagem não tem dimensões válidas".into());
    }

    let megapixels = (width as u64 * height as u64) / 1_000_000;
    if megapixels > MAX_INPUT_MEGAPIXELS {
        return Err(format!(
            "Imagem muito grande ({} x {}). O limite é ~{} MP de entrada.",
            width, height, MAX_INPUT_MEGAPIXELS
        ));
    }

    Ok((composite_on_white(&source), width, height))
}

fn cache_path(cache_dir: &Path, tag: Option<usize>) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    match tag {
        Some(step) => {
            let seq = FLOW_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            cache_dir.join(format!("remaster-{timestamp}-{seq}-{step}.png"))
        }
        None => cache_dir.join(format!("remaster-{timestamp}.png")),
    }
}

fn save_png(cache_dir: &Path, output: &RgbImage, tag: Option<usize>) -> Result<PathBuf, String> {
    let output_path = cache_path(cache_dir, tag);
    DynamicImage::ImageRgb8(output.clone())
        .save_with_format(&output_path, image::ImageFormat::Png)
        .map_err(|e| format!("Falha ao salvar resultado temporário: {e}"))?;
    Ok(output_path)
}

fn normalize_scale(scale: u32) -> u32 {
    match scale {
        1 => 1,
        2 => 2,
        _ => 4,
    }
}

fn backend_label(mode: &str) -> String {
    if mode == "classic" || !esrgan::cuda_ready() {
        "cpu".to_string()
    } else {
        "cuda".to_string()
    }
}

fn model_label(mode: &str, model_id: &str) -> String {
    if mode == "classic" {
        "classic".to_string()
    } else {
        model_id.to_string()
    }
}

/// Executa uma única etapa (single-step, usado pelo comando legado `upscale_image`).
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
    let scale = normalize_scale(request.scale);

    emit_progress(
        app,
        Progress {
            stage: "loading".into(),
            completed: 0,
            total: 1,
            percent: 2.0,
            elapsed_ms: 0,
            step: None,
            total_steps: None,
        },
    );

    let (rgb, width, height) = load_source(&request.path)?;

    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelado".into());
    }

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
                    step: None,
                    total_steps: None,
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
                    step: None,
                    total_steps: None,
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
            step: None,
            total_steps: None,
        },
    );

    let output_path = save_png(cache_dir, &output, None)?;
    let (out_width, out_height) = output.dimensions();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    emit_progress(
        app,
        Progress {
            stage: "done".into(),
            completed: 1,
            total: 1,
            percent: 100.0,
            elapsed_ms,
            step: None,
            total_steps: None,
        },
    );

    Ok(UpscaleResult {
        output_path: output_path.to_string_lossy().into_owned(),
        width: out_width,
        height: out_height,
        input_width: width,
        input_height: height,
        elapsed_ms,
        mode: request.mode.clone(),
        scale,
        model: model_label(&request.mode, model_id),
        backend: backend_label(&request.mode),
    })
}

/// Ordena os nós para execução: pais antes dos filhos, preservando a ordem de criação
/// entre irmãos/ramos. Também valida ids duplicados, pais inexistentes e ciclos.
fn topological_order(nodes: &[FlowNode]) -> Result<Vec<usize>, String> {
    if nodes.iter().any(|node| node.id.is_empty()) {
        return Err("Todo nó do fluxo precisa de um identificador.".into());
    }

    let mut index_by_id: HashMap<&str, usize> = HashMap::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        if index_by_id.insert(node.id.as_str(), index).is_some() {
            return Err(format!("Nó duplicado no fluxo: {}", node.id));
        }
    }

    let mut depth = vec![0usize; nodes.len()];
    for start in 0..nodes.len() {
        let mut seen = vec![false; nodes.len()];
        let mut current = start;
        let mut current_depth = 0usize;
        while let Some(parent_id) = nodes[current].parent_id.as_deref() {
            if seen[current] {
                return Err("O fluxo contém um ciclo.".into());
            }
            seen[current] = true;
            let parent = *index_by_id
                .get(parent_id)
                .ok_or_else(|| format!("Nó pai '{}' não encontrado.", parent_id))?;
            current = parent;
            current_depth += 1;
        }
        depth[start] = current_depth;
    }

    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&index| (depth[index], index));
    Ok(order)
}

fn node_label(node: &FlowNode) -> &str {
    if node.name.is_empty() {
        node.id.as_str()
    } else {
        node.name.as_str()
    }
}

/// Ajusta a resolução alvo dentro da caixa pedida, preservando a proporção.
fn fit_within(input_w: u32, input_h: u32, target_w: u32, target_h: u32) -> (u32, u32) {
    let scale = f64::min(
        target_w as f64 / input_w as f64,
        target_h as f64 / input_h as f64,
    );
    (
        ((input_w as f64 * scale).round() as u32).max(1),
        ((input_h as f64 * scale).round() as u32).max(1),
    )
}

fn target_dimensions(
    input_w: u32,
    input_h: u32,
    target_w: Option<u32>,
    target_h: Option<u32>,
) -> Option<(u32, u32)> {
    match (target_w, target_h) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some(fit_within(input_w, input_h, w, h)),
        (Some(w), _) if w > 0 => Some((
            w,
            ((input_h as f64 * w as f64 / input_w as f64).round() as u32).max(1),
        )),
        (_, Some(h)) if h > 0 => Some((
            ((input_w as f64 * h as f64 / input_h as f64).round() as u32).max(1),
            h,
        )),
        _ => None,
    }
}

/// Executa o fluxo em árvore de uma imagem. Cada nó consome a saída do nó pai
/// (ou a imagem original) e gera um resultado independente.
pub fn run_flow(
    app: &AppHandle,
    path: &str,
    nodes: &[FlowNode],
    cache_dir: &Path,
    env: Option<Environment>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    models: &HashMap<String, PathBuf>,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<FlowResult>, String> {
    run_flow_with(
        path,
        nodes,
        cache_dir,
        env,
        sessions,
        models,
        cancel,
        |progress| emit_progress(app, progress),
    )
}

/// Versão do fluxo que recebe o callback de progresso (testável sem GUI).
pub fn run_flow_with<F>(
    path: &str,
    nodes: &[FlowNode],
    cache_dir: &Path,
    env: Option<Environment>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    models: &HashMap<String, PathBuf>,
    cancel: Arc<AtomicBool>,
    mut report: F,
) -> Result<Vec<FlowResult>, String>
where
    F: FnMut(Progress),
{
    if nodes.is_empty() {
        return Err("Adicione pelo menos um nó ao fluxo.".into());
    }

    let order = topological_order(nodes)?;
    let total_nodes = order.len();
    let started = Instant::now();

    report(Progress {
        stage: "loading".into(),
        completed: 0,
        total: 1,
        percent: 2.0,
        elapsed_ms: 0,
        step: Some(1),
        total_steps: Some(total_nodes as u32),
    });

    let (source, _, _) = load_source(path)?;
    let mut images: HashMap<String, Arc<RgbImage>> = HashMap::new();
    images.insert(SOURCE_KEY.to_string(), Arc::new(source));

    // Quantas vezes cada saída será consumida (filhos). Quando chega a zero,
    // a imagem pode sair da memória — isso evita manter o grafo inteiro vivo.
    let mut uses: HashMap<String, usize> = HashMap::new();
    for node in nodes {
        let parent_key = node.parent_id.clone().unwrap_or_else(|| SOURCE_KEY.to_string());
        *uses.entry(parent_key).or_insert(0) += 1;
    }

    let mut results = Vec::with_capacity(total_nodes);
    let mut completed = 0usize;

    for index in order {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelado".into());
        }

        let node = &nodes[index];
        let label = node_label(node);
        let input_key = node.parent_id.clone().unwrap_or_else(|| SOURCE_KEY.to_string());
        let input = images.get(&input_key).cloned().ok_or_else(|| {
            format!(
                "A saída de entrada do nó '{}' não está disponível. Verifique se o nó pai foi processado.",
                label
            )
        })?;

        let (input_width, input_height) = input.dimensions();
        let scale = normalize_scale(node.scale);
        let target = target_dimensions(
            input_width,
            input_height,
            node.target_width,
            node.target_height,
        );
        let node_started = Instant::now();
        let step_number = completed + 1;
        let mut used_model = false;

        let progress_start = Progress {
            stage: "upscaling".into(),
            completed: 0,
            total: 1,
            percent: completed as f32 / total_nodes as f32 * 100.0,
            elapsed_ms: started.elapsed().as_millis() as u64,
            step: Some(step_number as u32),
            total_steps: Some(total_nodes as u32),
        };

        let output = if let Some((target_w, target_h)) = target {
            if node.mode == "classic" || (target_w <= input_width && target_h <= input_height) {
                // Alvo menor/igual à entrada ou motor Rápido: só redimensiona.
                report(progress_start);
                classic::resize(&input, target_w, target_h, &cancel)?
            } else {
                let environment = env
                    .as_ref()
                    .ok_or_else(|| format!("Backend de IA indisponível para o nó '{}'.", label))?;
                let model_id = if node.model.is_empty() {
                    super::DEFAULT_MODEL.to_string()
                } else {
                    node.model.clone()
                };
                let model_path = models.get(&model_id).ok_or_else(|| {
                    format!(
                        "Modelo '{}' não encontrado (nó '{}'). Rode `mise run model` para baixá-lo.",
                        model_id, label
                    )
                })?;

                report(Progress {
                    stage: "model".into(),
                    ..progress_start
                });

                let base = completed as f32;
                let native = esrgan::upscale(
                    environment,
                    &sessions,
                    &model_id,
                    model_path,
                    &input,
                    4,
                    &cancel,
                    started,
                    |mut progress| {
                        progress.percent =
                            (base + progress.percent / 100.0) / total_nodes as f32 * 100.0;
                        progress.step = Some(step_number as u32);
                        progress.total_steps = Some(total_nodes as u32);
                        report(progress);
                    },
                )?;
                used_model = true;
                classic::resize(&native, target_w, target_h, &cancel)?
            }
        } else {
            match node.mode.as_str() {
                "classic" => {
                    report(progress_start);
                    classic::upscale(&input, scale, &cancel)?
                }
                _ => {
                    let environment = env.as_ref().ok_or_else(|| {
                        format!("Backend de IA indisponível para o nó '{}'.", label)
                    })?;
                    let model_id = if node.model.is_empty() {
                        super::DEFAULT_MODEL.to_string()
                    } else {
                        node.model.clone()
                    };
                    let model_path = models.get(&model_id).ok_or_else(|| {
                        format!(
                            "Modelo '{}' não encontrado (nó '{}'). Rode `mise run model` para baixá-lo.",
                            model_id, label
                        )
                    })?;

                    report(Progress {
                        stage: "model".into(),
                        ..progress_start
                    });

                    let base = completed as f32;
                    let output = esrgan::upscale(
                        environment,
                        &sessions,
                        &model_id,
                        model_path,
                        &input,
                        scale,
                        &cancel,
                        started,
                        |mut progress| {
                            // Converte o progresso interno do nó em progresso global.
                            progress.percent =
                                (base + progress.percent / 100.0) / total_nodes as f32 * 100.0;
                            progress.step = Some(step_number as u32);
                            progress.total_steps = Some(total_nodes as u32);
                            report(progress);
                        },
                    )?;
                    used_model = true;
                    output
                }
            }
        };

        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelado".into());
        }

        let (out_width, out_height) = output.dimensions();
        let output_path = save_png(cache_dir, &output, Some(step_number))?;
        let elapsed_ms = node_started.elapsed().as_millis() as u64;

        let model_id = if node.mode == "classic" {
            String::new()
        } else if node.model.is_empty() {
            super::DEFAULT_MODEL.to_string()
        } else {
            node.model.clone()
        };

        results.push(FlowResult {
            node_id: node.id.clone(),
            parent_id: node.parent_id.clone(),
            output_path: output_path.to_string_lossy().into_owned(),
            width: out_width,
            height: out_height,
            input_width,
            input_height,
            target_width: target.map(|(width, _)| width),
            target_height: target.map(|(_, height)| height),
            used_model,
            elapsed_ms,
            mode: node.mode.clone(),
            scale: if target.is_some() { 0 } else { scale },
            model: model_label(&node.mode, &model_id),
            backend: if used_model && esrgan::cuda_ready() {
                "cuda".to_string()
            } else {
                "cpu".to_string()
            },
        });

        // Libera a entrada quando nenhum outro filho vai reutilizá-la.
        if let Some(count) = uses.get_mut(&input_key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                images.remove(&input_key);
            }
        }

        completed += 1;

        // Mantém a saída viva apenas se algum filho ainda vai consumi-la.
        if uses.get(&node.id).copied().unwrap_or(0) > 0 {
            images.insert(node.id.clone(), Arc::new(output));
        }
    }

    report(Progress {
        stage: "done".into(),
        completed: 1,
        total: 1,
        percent: 100.0,
        elapsed_ms: started.elapsed().as_millis() as u64,
        step: Some(total_nodes as u32),
        total_steps: Some(total_nodes as u32),
    });

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classic_node(id: &str, name: &str, parent: Option<&str>, scale: u32) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            name: name.to_string(),
            parent_id: parent.map(str::to_string),
            mode: "classic".into(),
            scale,
            target_width: None,
            target_height: None,
            model: String::new(),
        }
    }

    #[test]
    fn flow_runs_branches_from_shared_parent() {
        let dir = std::env::temp_dir().join(format!("image-remaster-flow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("diretório de teste");

        let source_path = dir.join("source.png");
        image::RgbImage::from_pixel(8, 6, image::Rgb([30, 60, 90]))
            .save(&source_path)
            .expect("imagem de teste");
        let cache_dir = dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache de teste");

        // Original ├─ 1 (2x) ├─ 1.1 (1x)
        //          └─ 2 (4x) └─ 2.1 (1x)
        let nodes = vec![
            classic_node("a", "1", None, 2),
            classic_node("b", "1.1", Some("a"), 1),
            classic_node("c", "2", None, 4),
            classic_node("d", "2.1", Some("c"), 1),
        ];

        let mut updates = Vec::new();
        let results = run_flow_with(
            &source_path.to_string_lossy(),
            &nodes,
            &cache_dir,
            None,
            Arc::new(Mutex::new(HashMap::new())),
            &HashMap::new(),
            Arc::new(AtomicBool::new(false)),
            |progress| updates.push(progress),
        )
        .expect("fluxo ramificado");

        assert_eq!(results.len(), 4);
        // Ordem topológica: raízes primeiro, depois os filhos.
        let by_id: HashMap<&str, &FlowResult> =
            results.iter().map(|result| (result.node_id.as_str(), result)).collect();

        assert_eq!((by_id["a"].width, by_id["a"].height), (16, 12));
        assert_eq!((by_id["b"].input_width, by_id["b"].input_height), (16, 12));
        assert_eq!((by_id["b"].width, by_id["b"].height), (16, 12));
        assert_eq!((by_id["c"].width, by_id["c"].height), (32, 24));
        assert_eq!((by_id["d"].input_width, by_id["d"].input_height), (32, 24));
        assert_eq!((by_id["d"].width, by_id["d"].height), (32, 24));

        // O ramo 2 não deve ter sido afetado pelo ramo 1 (partiu do original).
        assert_eq!((by_id["c"].input_width, by_id["c"].input_height), (8, 6));
        assert_eq!(by_id["a"].parent_id, None);
        assert_eq!(by_id["b"].parent_id.as_deref(), Some("a"));

        for result in &results {
            assert!(std::path::Path::new(&result.output_path).exists());
        }
        assert!(updates.last().is_some_and(|progress| progress.stage == "done"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flow_targets_resolution_without_changing_aspect() {
        let dir = std::env::temp_dir().join(format!(
            "image-remaster-flow-target-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("diretório de teste");

        let source_path = dir.join("source.png");
        image::RgbImage::from_pixel(8, 6, image::Rgb([30, 60, 90]))
            .save(&source_path)
            .expect("imagem de teste");
        let cache_dir = dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache de teste");

        let nodes = vec![
            FlowNode {
                id: "a".into(),
                name: "1".into(),
                parent_id: None,
                mode: "classic".into(),
                scale: 2,
                target_width: Some(20),
                target_height: Some(20),
                model: String::new(),
            },
            FlowNode {
                id: "b".into(),
                name: "2".into(),
                parent_id: None,
                mode: "ai".into(),
                scale: 2,
                target_width: Some(4),
                target_height: Some(3),
                model: "rapido".into(),
            },
        ];

        // Sem ambiente/modelos: o nó B tem alvo menor que a entrada e deve
        // apenas redimensionar, sem carregar o modelo.
        let results = run_flow_with(
            &source_path.to_string_lossy(),
            &nodes,
            &cache_dir,
            None,
            Arc::new(Mutex::new(HashMap::new())),
            &HashMap::new(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .expect("fluxo com resolução alvo");

        let by_id: HashMap<&str, &FlowResult> =
            results.iter().map(|result| (result.node_id.as_str(), result)).collect();

        // 8x6 (4:3) dentro de 20x20 -> 20x15, proporção preservada.
        assert_eq!((by_id["a"].width, by_id["a"].height), (20, 15));
        assert_eq!((by_id["a"].target_width, by_id["a"].target_height), (Some(20), Some(15)));
        assert_eq!(by_id["a"].scale, 0);

        // Alvo menor: redução direta, sem modelo.
        assert_eq!((by_id["b"].width, by_id["b"].height), (4, 3));
        assert_eq!(by_id["b"].backend, "cpu");
        assert!(!by_id["b"].used_model);
        assert!(!by_id["a"].used_model);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flow_runs_ai_branch_without_recomputing_siblings() {
        let model_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("models")
            .join(crate::processing::MODEL_FILE);

        if !model_path.exists() {
            eprintln!("modelo ausente em {} — teste ignorado", model_path.display());
            return;
        }

        let dir = std::env::temp_dir().join(format!("image-remaster-flow-ai-{}", std::process::id()));
        let cache_dir = dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache de teste");

        let source_path = dir.join("source.png");
        image::RgbImage::from_fn(24, 24, |x, y| image::Rgb([(x * 9) as u8, (y * 9) as u8, 120]))
            .save(&source_path)
            .expect("imagem de teste");

        let env = ort::init()
            .with_name("image-remaster-flow-test")
            .build()
            .expect("ambiente ONNX");
        let mut models = HashMap::new();
        models.insert("rapido".to_string(), model_path);

        let ai_node = |id: &str, name: &str, parent: Option<&str>, scale: u32| FlowNode {
            id: id.to_string(),
            name: name.to_string(),
            parent_id: parent.map(str::to_string),
            mode: "ai".into(),
            scale,
            target_width: None,
            target_height: None,
            model: "rapido".into(),
        };

        // Original ├─ 1 (2x, IA) ├─ 1.1 (1x, IA)
        //          └─ 2 (1x, IA)
        let nodes = vec![
            ai_node("a", "1", None, 2),
            ai_node("b", "1.1", Some("a"), 1),
            ai_node("c", "2", None, 1),
        ];

        let results = run_flow_with(
            &source_path.to_string_lossy(),
            &nodes,
            &cache_dir,
            Some(env),
            Arc::new(Mutex::new(HashMap::new())),
            &models,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .expect("fluxo de IA ramificado");

        let by_id: HashMap<&str, &FlowResult> =
            results.iter().map(|result| (result.node_id.as_str(), result)).collect();

        assert_eq!(results.len(), 3);
        assert_eq!((by_id["a"].width, by_id["a"].height), (48, 48));
        assert_eq!((by_id["b"].input_width, by_id["b"].input_height), (48, 48));
        assert_eq!((by_id["b"].width, by_id["b"].height), (48, 48));
        assert_eq!((by_id["c"].input_width, by_id["c"].input_height), (24, 24));
        assert_eq!((by_id["c"].width, by_id["c"].height), (24, 24));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
