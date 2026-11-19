#!/usr/bin/env bash
# Copia os providers do ONNX Runtime (CUDA) para src-tauri/libs.
# Eles são colocados ao lado do executável no AppImage pelo finalize_appimage.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/src-tauri/libs"

mkdir -p "$DEST"
rm -f "$DEST"/*.so "$DEST"/*.xz

SOURCE="$(find "$HOME/.cache/ort.pyke.io" -type f -name 'libonnxruntime_providers_cuda.so' -printf '%h\n' 2>/dev/null | head -1)"
if [[ -z "${SOURCE:-}" ]]; then
  echo "provider CUDA não encontrado; rode 'cd src-tauri && cargo build --release' primeiro." >&2
  exit 1
fi

for lib in libonnxruntime_providers_shared.so libonnxruntime_providers_cuda.so; do
  cp -L "$SOURCE/$lib" "$DEST/$lib"
done

ls -lh "$DEST"
