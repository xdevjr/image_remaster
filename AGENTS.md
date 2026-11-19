# AGENTS.md

Desktop app (Tauri v2 + Vue 3) that upscales low-res images locally with Real-ESRGAN (ONNX) and
compares original/result side by side. Supports batch processing with a clickable image list.
Runtime is fully offline; do not add network calls to the processing path (see "Privacidade" in
`README.md`).

## Toolchain

- Rust **1.100.0** and Bun **1.4.5** are pinned in `mise.toml`. Use `mise install`; if mise isn't
  activated, prepend `$HOME/.local/share/mise/shims` to `PATH`.
- Use **bun**, never npm/node. Lockfile is `bun.lock`.
- TypeScript is pinned to **5.x**. TypeScript 7 breaks `vue-tsc` (`ERR_PACKAGE_PATH_NOT_EXPORTED`) —
  don't upgrade it.
- Linux requires `webkit2gtk-4.1`/GTK3 (already present on this machine).

## Commands

```bash
mise install                 # toolchain
bun install                  # frontend deps
mise run model               # downloads the 4 ONNX models into ./models (~154 MB)
mise run dev                 # Vite + Tauri dev (don't start vite separately; port 5173 must be free)
mise run build               # AppImage only (see build flow below)

bun run build                # typecheck only: vue-tsc --noEmit + vite build
cd src-tauri && cargo check
cd src-tauri && cargo test   # 3 tests; ONNX test skips if the model is absent
cd src-tauri && cargo test upscale_tiny_image_produces_4x_output
cd src-tauri && cargo run --release --example upscale_file -- /tmp/in.jpg /tmp/out.png 1|2|4 ../models/RealESRGAN_x4plus_fp32.onnx
```

- **Production builds must go through `mise run build`** (`scripts/build_appimage.sh`):
  1. `prepare_bundle.sh` copies the CUDA providers to `src-tauri/libs/`;
  2. `tauri build --no-bundle` compiles;
  3. provider symlinks are removed from `target/release`;
  4. `tauri bundle` **fails at linuxdeploy on purpose** (CUDA deps) after creating the AppDir;
  5. `finalize_appimage.sh` puts the providers next to the executable and repacks with
     `linuxdeploy-plugin-appimage`.
  Do not "fix" the linuxdeploy failure by adding `bundle.linux.appimage.files`/resources with the
  provider `.so`: Tauri copies them **before** linuxdeploy and it fails the same way.
- App args for dev need the double separator:
  `bun run tauri dev -- -- --autorun a.jpg b.jpg`.
  `--autorun` processes all passed images as a batch; without it they are just listed/opened.
  Optional app flags: `--model rapido|qualidade|nitido|anime` and `--scale 1|2|4`
  (e.g. `--autorun --model nitido --scale 2 a.jpg b.jpg`).
- No CI/lint config exists; `cargo fmt`/`clippy` are available but not enforced.

## Architecture

- `src-tauri/src/lib.rs` — Tauri builder, `AppState` (cancel flag, ONNX `Environment`, sessions
  `Mutex<HashMap<model_id, Session>>`, cache dir, resolved model paths). Calls
  `esrgan::prepare_backend()` before Tauri starts.
- `src-tauri/src/commands.rs` — IPC: `load_image`, `upscale_image`, `save_image`, `cancel_job`,
  `runtime_info`, `startup_info` (returns `paths: Vec<String>`). Long jobs run in `spawn_blocking`;
  progress is the `upscale:progress` event. Structs use camelCase; JS passes camelCase args.
- `src-tauri/src/processing/mod.rs` — model catalog (`MODELS`: `rapido`/`qualidade`/`nitido`/
  `anime`, default `qualidade`) and path resolution.
- `src-tauri/src/processing/` — `worker.rs` (orchestration, cache, 6 MP limit, result has
  `model`/`backend`), `esrgan.rs` (tiling, sessions per model, GPU), `classic.rs` (Lanczos3 +
  unsharp; scale 1 skips resize). Scales: 1 = remaster at original size (4x model then downscale),
  2 = 4x then Lanczos downscale, 4 = native model output.
- `src/App.vue` — items list (`JobItem[]`), batch loop, selection, save-all. `ImageList.vue` is the
  sidebar (shown when >1 image). `ComparisonViewer.vue` shares normalized zoom/center; slider mode
  clips the original layer.

## GPU (CUDA) — non-obvious

- CUDA libs live outside the app: `~/.local/share/image-remaster/{cuda,cudnn}`, installed by
  `scripts/setup_cuda.sh` (CUDA 13 + cuDNN 9 wheels). `scripts/toggle_gpu.sh on|off` renames them.
- **ORT loads the CUDA provider from the executable's directory**, not from resources or a plain
  `LD_LIBRARY_PATH` search. The final AppImage therefore has `libonnxruntime_providers_{cuda,shared}.so`
  in `usr/bin` (added by `finalize_appimage.sh`); dev builds get them next to `target/debug`.
- `prepare_backend()` re-executes the process once with `LD_LIBRARY_PATH` including the exe dir and
  the CUDA/cuDNN dirs (`IMAGE_REMASTER_REEXEC=1` guards the loop), so ORT can resolve CUDA deps.
  Do **not** `dlopen` the provider before ORT: its initializer segfaults (`Provider_GetHost` host
  not initialized). The old `.xz`/extraction approach was removed for this reason.
- Without CUDA dirs the app stays on CPU; session failure under CUDA falls back to CPU and reports
  `backend: "cpu"`.
- Tiles: 256 px by default; 384 px only on GPUs with ≥12 GB VRAM (detected via `nvidia-smi`).
  512 px OOMs on 8 GB with x4plus. `IMAGE_REMASTER_TILE` overrides. CUDA is configured with
  heuristic conv search, TF32, no max workspace, `SameAsRequested` arena, and a BFC memory limit of
  ~80% of total VRAM (leaves headroom for the desktop).
- **VRAM between jobs:** each model session keeps its own CUDA arena, so `LAST_JOB` (model + input
  dimensions) is tracked and sessions are cleared when either changes; this is what prevents the
  second image from OOMing. If an allocation still fails, `upscale()` retries once with a fresh
  session at the same tile, then falls back to 192/128 px tiles. OOM is detected by strings like
  `Failed to allocate memory`, `Available memory of`, `requested bytes`, `bfc_arena`. Persisting
  OOM returns a user-friendly `memory_error` (GPU memory insufficient).
- If a bundle fails, delete `src-tauri/target/release/bundle/appimage/Image Remaster.AppDir` and
  `src-tauri/target/release/providers` — stale `.so` files there cause misleading errors.

## Non-obvious gotchas

- **Asset protocol + dotfiles:** results live in `app_cache_dir()` (`~/.cache/dev.juniorgs.imageremaster`).
  On Unix the asset scope forbids leading dots by default, so `tauri.conf.json` must keep
  `assetProtocol.scope.requireLiteralLeadingDot: false`. Without it, results silently fail to load
  (broken image icon) while the original still shows.
- Startup cleanup deletes every `remaster-*` file in the cache dir — don't point other features at
  that prefix.
- `ort` is **2.0.0-rc.15** with a particular API: `Session::builder(&env)`, `session.run` takes
  `&mut Session`, `Tensor::from_array((shape, vec))`, `inputs![name => tensor]`. ONNX Runtime
  telemetry is disabled with `.with_telemetry(false)` — keep it that way.
- Batch processing is sequential on purpose (one session/GPU); changing model/scale does not
  invalidate existing results.
- Frontend files use `<script setup lang="ts">`; UI strings and code comments are in pt-BR.
- Only `core:default` and `dialog:default` permissions are granted (`src-tauri/capabilities/`).
  Adding a plugin/API requires updating that file.
- Generated/ignored: `dist/`, `src-tauri/target/`, `src-tauri/gen/`, `src-tauri/libs/`,
  `models/*.onnx`, `node_modules/`. Icons are generated with `bun run tauri icon <1024px.png>`.
- For visual checks on this Wayland setup, `grim /tmp/x.png` captures the screen; drive the app
  with `--autorun` (needs a model). No `kdotool`/`wmctrl` on this desktop.

## Tests and verification

- `cargo test` covers the ONNX pipeline (skips if the model is missing), classic upscale, and
  PNG/JPEG/WebP export.
- Frontend has no unit tests; `bun run build` is the typecheck gate.
- For quick quality/speed checks, use the `upscale_file` example (release mode; optional model path;
  it re-execs for GPU automatically) instead of the GUI.
