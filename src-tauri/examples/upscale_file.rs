//! Utilitário de linha de comando para testar o pipeline de upscale fora da UI.
//!
//! Uso:
//!   cargo run --example upscale_file -- <entrada> <saida> [escala] [modelo.onnx]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Instant;

use image_remaster_lib::processing::{esrgan, MODEL_FILE};

fn main() {
    esrgan::prepare_backend();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("uso: upscale_file <entrada> <saida> [2|4] [modelo.onnx]");
        std::process::exit(1);
    }

    let input = &args[1];
    let output = &args[2];
    let scale: u32 = args.get(3).and_then(|value| value.parse().ok()).unwrap_or(4);

    let model_path = match args.get(4) {
        Some(path) => PathBuf::from(path),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("models")
            .join(MODEL_FILE),
    };
    println!("modelo: {}", model_path.display());

    let source = image::open(input).expect("imagem de entrada").to_rgb8();
    println!("entrada: {}x{}", source.width(), source.height());

    let environment = ort::init()
        .with_name("image-remaster-cli")
        .build()
        .expect("ambiente ONNX");
    let sessions = Mutex::new(HashMap::new());
    let cancel = AtomicBool::new(false);
    let started = Instant::now();

    let upscaled = esrgan::upscale(
        &environment,
        &sessions,
        "cli",
        &model_path,
        &source,
        scale,
        &cancel,
        started,
        |progress| {
            if progress.completed % 4 == 0 || progress.percent >= 94.0 {
                eprintln!(
                    "[{:>3}/{:>3}] {:.0}%",
                    progress.completed, progress.total, progress.percent
                );
            }
        },
    )
    .expect("upscale");

    upscaled.save(output).expect("salvar saída");
    println!(
        "saída: {}x{} em {:.2?} -> {}",
        upscaled.width(),
        upscaled.height(),
        started.elapsed(),
        output
    );
}
