#!/usr/bin/env bash
# Libera (ou bloqueia) o modo GPU do app, sem baixar nada.
set -euo pipefail

DEST="${XDG_DATA_HOME:-$HOME/.local/share}/image-remaster"
ACTION="${1:-on}"

case "$ACTION" in
  on)
    mkdir -p "$DEST/cuda.partial" "$DEST/cudnn.partial"
    [[ -d "$DEST/cuda.partial" ]] && mv "$DEST/cuda.partial" "$DEST/cuda" 2>/dev/null || true
    [[ -d "$DEST/cudnn.partial" ]] && mv "$DEST/cudnn.partial" "$DEST/cudnn" 2>/dev/null || true
    echo "GPU habilitada (se as libs estiverem instaladas)."
    ;;
  off)
    for dir in cuda cudnn; do
      if [[ -d "$DEST/$dir" ]]; then
        mv "$DEST/$dir" "$DEST/$dir.partial"
      fi
    done
    echo "GPU desabilitada; o app usará CPU."
    ;;
  *)
    echo "uso: $0 [on|off]" >&2
    exit 1
    ;;
esac
