#!/usr/bin/env bash
# Finaliza o AppImage a partir do AppDir criado pelo Tauri.
#
# O linuxdeploy falha ao resolver as dependências CUDA dos providers do ONNX
# Runtime. O AppDir, porém, fica completo antes dessa etapa; aqui colocamos os
# providers ao lado do executável (onde o ORT os procura) e reempacotamos com
# o linuxdeploy-plugin-appimage.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE="$ROOT/src-tauri/target/release/bundle/appimage"
APPDIR="$BUNDLE/Image Remaster.AppDir"
TOOL="$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"

if [[ ! -d "$APPDIR" ]]; then
  echo "AppDir não encontrado em $APPDIR" >&2
  exit 1
fi

if [[ ! -x "$TOOL" ]]; then
  echo "linuxdeploy-plugin-appimage não encontrado em $TOOL; rode 'tauri build' uma vez." >&2
  exit 1
fi

for lib in libonnxruntime_providers_shared.so libonnxruntime_providers_cuda.so; do
  cp -L "$ROOT/src-tauri/libs/$lib" "$APPDIR/usr/bin/$lib"
done

# Providers comprimidos antigos não são mais usados.
rm -rf "$APPDIR/usr/lib/Image Remaster/providers"

ICON="$APPDIR/usr/share/icons/hicolor/256x256@2/apps/image-remaster.png"
if [[ ! -f "$ICON" ]]; then
  ICON="$(find "$APPDIR/usr/share/icons" -name 'image-remaster.png' | head -1)"
fi
cp "$ICON" "$APPDIR/image-remaster.png"

VERSION="$(python3 -c "import json; print(json.load(open('$ROOT/src-tauri/tauri.conf.json'))['version'])")"

rm -f "$BUNDLE"/*.AppImage
cd "$BUNDLE"
ARCH=x86_64 VERSION="$VERSION" \
  "$TOOL" --appimage-extract-and-run --appdir="$APPDIR" >/dev/null

OUT="$(find "$BUNDLE" -maxdepth 1 -name '*.AppImage' -print -quit)"
if [[ -z "${OUT:-}" || ! -f "$OUT" ]]; then
  echo "falha ao reempacotar o AppImage" >&2
  exit 1
fi

mv -f "$OUT" "$BUNDLE/Image Remaster_${VERSION}_amd64.AppImage"
rm -rf "$APPDIR"

echo "AppImage final: $BUNDLE/Image Remaster_${VERSION}_amd64.AppImage"
ls -lh "$BUNDLE/Image Remaster_${VERSION}_amd64.AppImage"
