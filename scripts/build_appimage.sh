#!/usr/bin/env bash
# Build completo do AppImage.
#
# O linuxdeploy falha ao tentar resolver as dependências CUDA dos providers do
# ONNX Runtime. O Tauri cria o AppDir completo antes dessa etapa, então seguimos
# com ele e reempacotamos no finalize_appimage.sh com os providers ao lado do
# executável (onde o ORT os procura).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BUNDLE="$ROOT/src-tauri/target/release/bundle/appimage"
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

bash scripts/prepare_bundle.sh

bun run tauri build --no-bundle

rm -f "$ROOT"/src-tauri/target/release/libonnxruntime_providers_*.so

if ! bun run tauri bundle >"$LOG" 2>&1; then
  if [[ ! -d "$BUNDLE/Image Remaster.AppDir" ]]; then
    cat "$LOG" >&2
    echo "o bundle não gerou o AppDir" >&2
    exit 1
  fi
  echo "(linuxdeploy falhou como esperado; usando o AppDir para reempacotar)"
fi

bash scripts/finalize_appimage.sh
