#!/usr/bin/env bash
# Instala as bibliotecas CUDA 13 + cuDNN 9 (wheels oficiais da NVIDIA no PyPI)
# em ~/.local/share/image-remaster/{cuda,cudnn} para acelerar a inferência na GPU.
#
# Download único (~1,2 GB); depois o app roda offline. Só é necessário em máquinas
# com GPU NVIDIA; sem essas libs o app cai automaticamente para CPU.
set -euo pipefail

DEST="${XDG_DATA_HOME:-$HOME/.local/share}/image-remaster"
CUDA_DIR="$DEST/cuda"
CUDNN_DIR="$DEST/cudnn"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "destino: $CUDA_DIR e $CUDNN_DIR"
mkdir -p "$CUDA_DIR" "$CUDNN_DIR" "$TMP/wheels"

# Versões testadas com o ort (onnxruntime 1.31 / CUDA 13).
python3 - "$TMP/wheels" <<'PY'
import json
import sys
import urllib.request

target = sys.argv[1]
packages = {
    "nvidia-cuda-runtime": "13.5.37",
    "nvidia-cublas": "13.10.1.6",
    "nvidia-cufft": "12.5.0.14",
    "nvidia-cuda-nvrtc": "13.5.37",
    "nvidia-curand": "10.4.5.25",
    "nvidia-cudnn-cu13": "9.28.1.4",
}

for name, version in packages.items():
    with urllib.request.urlopen(f"https://pypi.org/pypi/{name}/{version}/json") as response:
        data = json.load(response)
    wheel = next(
        (f for f in data["urls"] if "x86_64" in f["filename"] and "manylinux" in f["filename"]),
        None,
    )
    if wheel is None:
        raise SystemExit(f"wheel x86_64 não encontrado para {name}=={version}")
    print(f"baixando {wheel['filename']} ({wheel['size'] / 1e6:.0f} MB)")
    urllib.request.urlretrieve(wheel["url"], f"{target}/{wheel['filename']}")
PY

echo "extraindo..."
mkdir -p "$TMP/extract"
for wheel in "$TMP"/wheels/*.whl; do
  name="$(basename "$wheel" .whl)"
  mkdir -p "$TMP/extract/$name"
  bsdtar -xf "$wheel" -C "$TMP/extract/$name"
done

copy_lib() {
  local pattern="$1"
  local dest="$2"
  local found
  found="$(find "$TMP/extract" -path '*/lib/*' -name "$pattern" -print -quit)"
  if [[ -z "$found" ]]; then
    echo "AVISO: $pattern não encontrado" >&2
    return
  fi
  cp -L "$found" "$dest/"
}

for lib in \
  "libcudart.so.13" \
  "libcublas.so.13" \
  "libcublasLt.so.13" \
  "libcufft.so.12" \
  "libcurand.so.10" \
  "libnvrtc.so.13" \
  "libnvrtc-builtins.so.13.5"; do
  copy_lib "$lib" "$CUDA_DIR"
done

for lib in \
  "libcudnn.so.9" \
  "libcudnn_graph.so.9" \
  "libcudnn_ops.so.9" \
  "libcudnn_heuristic.so.9" \
  "libcudnn_adv.so.9" \
  "libcudnn_cnn.so.9" \
  "libcudnn_engines_precompiled.so.9" \
  "libcudnn_engines_runtime_compiled.so.9"; do
  copy_lib "$lib" "$CUDNN_DIR"
done

echo
echo "instalado:"
du -sh "$CUDA_DIR" "$CUDNN_DIR"
echo "Reinicie o app; ele detecta a GPU automaticamente (status em runtime_info)."
