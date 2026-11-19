#!/usr/bin/env bash
# Baixa os modelos Real-ESRGAN em formato ONNX usados pelo app.
set -euo pipefail

MODEL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/models"
mkdir -p "$MODEL_DIR"

HF="https://huggingface.co"

download() {
  local file="$1"
  local url="$2"
  local path="$MODEL_DIR/$file"

  if [[ -f "$path" ]]; then
    echo "ok (já existe): $file"
    return
  fi

  echo "baixando $file..."
  curl -L --fail --progress-bar -o "$path" "$url"
}

download "realesr-general-x4v3.onnx" \
  "$HF/Heliosoph/realesrgan-onnx/resolve/main/realesr-general-x4v3.onnx"

download "RealESRGAN_x4plus_fp32.onnx" \
  "$HF/universonic/RealESRGAN/resolve/main/RealESRGAN_x4plus_fp32.onnx"

download "4x-UltraSharp-fp32-opset17.onnx" \
  "$HF/Kim2091/UltraSharp/resolve/main/ONNX/4x-UltraSharp-fp32-opset17.onnx"

download "RealESRGAN_x4plus_anime_6B_fp32.onnx" \
  "$HF/universonic/RealESRGAN/resolve/main/RealESRGAN_x4plus_anime_6B_fp32.onnx"

echo
echo "Modelos disponíveis em $MODEL_DIR:"
ls -lh "$MODEL_DIR"/*.onnx
