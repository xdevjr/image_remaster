<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import ComparisonViewer from "./components/ComparisonViewer.vue";
import ImageList from "./components/ImageList.vue";
import ProgressBar from "./components/ProgressBar.vue";
import Toolbar from "./components/Toolbar.vue";
import { useImageJob } from "./composables/useImageJob";
import type {
  BatchProgress,
  JobItem,
  RuntimeInfo,
  UpscaleMode,
  ViewerMode,
} from "./types";

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tif"];
const IMAGE_PATTERN = new RegExp(`\\.(${IMAGE_EXTENSIONS.join("|")})$`, "i");

const items = ref<JobItem[]>([]);
const selectedId = ref<string | null>(null);
const mode = ref<UpscaleMode>("ai");
const scale = ref(2);
const model = ref("qualidade");
const viewerMode = ref<ViewerMode>("side");
const busy = ref(false);
const batch = ref<BatchProgress | null>(null);
const error = ref<string | null>(null);
const notice = ref<string | null>(null);
const dragActive = ref(false);
const runtime = ref<RuntimeInfo | null>(null);

const { progress, loadImage, upscale, saveImage, cancel, runtimeInfo, startupInfo } =
  useImageJob();

const models = computed(() => runtime.value?.models ?? []);
const selectedModel = computed(
  () => models.value.find((option) => option.id === model.value) ?? null,
);
const gpuReady = computed(() => runtime.value?.gpu.active ?? false);
const modelLabel = computed(() => selectedModel.value?.label ?? "—");

const selectedItem = computed(
  () => items.value.find((item) => item.id === selectedId.value) ?? null,
);
const original = computed(() => selectedItem.value?.info ?? null);
const result = computed(() => selectedItem.value?.result ?? null);
const originalUrl = computed(() => (original.value ? convertFileSrc(original.value.path) : ""));
const resultUrl = computed(() => (result.value ? convertFileSrc(result.value.outputPath) : ""));

let unlistenDrop: UnlistenFn | null = null;
let noticeTimer: number | undefined;
let cancelRequested = false;

function newId() {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
}

function showNotice(message: string) {
  notice.value = message;
  if (noticeTimer) window.clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => {
    notice.value = null;
  }, 3500);
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatMs(ms: number) {
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${ms} ms`;
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

async function addPaths(paths: string[]) {
  if (paths.length === 0) return;
  error.value = null;

  const added: string[] = [];
  for (const path of paths) {
    if (items.value.some((item) => item.info.path === path)) continue;
    try {
      const info = await loadImage(path);
      const item: JobItem = {
        id: newId(),
        info,
        result: null,
        status: "pending",
        error: null,
      };
      items.value.push(item);
      added.push(item.id);
    } catch (cause) {
      error.value = String(cause);
    }
  }

  if (added.length > 0) {
    selectedId.value = added[0];
  }
}

function selectItem(id: string) {
  selectedId.value = id;
  progress.value = null;
}

function removeItem(id: string) {
  if (busy.value) return;
  const index = items.value.findIndex((item) => item.id === id);
  if (index === -1) return;
  items.value.splice(index, 1);
  if (selectedId.value === id) {
    selectedId.value = items.value[Math.min(index, items.value.length - 1)]?.id ?? null;
  }
}

function clearItems() {
  if (busy.value) return;
  items.value = [];
  selectedId.value = null;
  progress.value = null;
  error.value = null;
}

async function openImageDialog() {
  if (busy.value) return;
  const selected = await openDialog({
    title: "Escolha as imagens",
    multiple: true,
    directory: false,
    filters: [{ name: "Imagens", extensions: IMAGE_EXTENSIONS }],
  });

  if (typeof selected === "string") {
    await addPaths([selected]);
  } else if (Array.isArray(selected)) {
    await addPaths(selected);
  }
}

async function refreshRuntime() {
  try {
    runtime.value = await runtimeInfo();
  } catch {
    /* status é opcional */
  }
}

function checkModel() {
  if (mode.value !== "ai") return true;
  if (selectedModel.value && !selectedModel.value.available) {
    error.value = `Modelo "${selectedModel.value.label}" não encontrado. Rode \`mise run model\` no terminal e reinicie o app (ou use o motor Rápido).`;
    return false;
  }
  return true;
}

async function processItem(item: JobItem) {
  if (!checkModel()) {
    item.status = "error";
    item.error = error.value;
    return;
  }

  item.status = "processing";
  item.error = null;
  progress.value = null;

  try {
    const output = await upscale(
      item.info.path,
      mode.value,
      scale.value,
      mode.value === "ai" ? model.value : "",
    );
    item.result = output;
    item.status = "done";
  } catch (cause) {
    const message = String(cause);
    if (message.toLowerCase().includes("cancelado")) {
      item.status = "cancelled";
    } else {
      item.status = "error";
      item.error = message;
      error.value = message;
    }
  } finally {
    await refreshRuntime();
  }
}

async function runSelected() {
  if (busy.value) return;
  const item = selectedItem.value ?? items.value[0];
  if (!item) return;

  busy.value = true;
  item.status = "processing";
  try {
    await processItem(item);
  } finally {
    busy.value = false;
    progress.value = null;
  }
}

async function runAll() {
  if (busy.value || items.value.length === 0) return;
  if (!checkModel()) return;

  let targets = items.value.filter((item) => !item.result);
  if (targets.length === 0) {
    targets = [...items.value]; // reprocessa tudo ao mudar modelo/escala
  }

  cancelRequested = false;
  busy.value = true;
  batch.value = { current: 0, total: targets.length };
  selectedId.value = targets[0]?.id ?? selectedId.value;

  try {
    for (let index = 0; index < targets.length; index += 1) {
      if (cancelRequested) break;
      const item = targets[index];
      batch.value = { current: index + 1, total: targets.length };
      selectedId.value = item.id;
      await processItem(item);
      if (item.status === "cancelled") break;
    }
  } finally {
    batch.value = null;
    busy.value = false;
    progress.value = null;
  }
}

async function cancelJob() {
  cancelRequested = true;
  try {
    await cancel();
  } catch (cause) {
    error.value = String(cause);
  }
}

async function saveResult() {
  if (busy.value) return;
  const item = selectedItem.value;
  if (!item?.result) return;

  const stem = fileName(item.info.path).replace(/\.[^.]+$/, "");
  const destination = await saveDialog({
    title: "Salvar imagem remasterizada",
    defaultPath: `${stem}-remaster-${item.result.scale}x.png`,
    filters: [
      { name: "PNG", extensions: ["png"] },
      { name: "JPEG", extensions: ["jpg", "jpeg"] },
      { name: "WebP", extensions: ["webp"] },
    ],
  });

  if (!destination) return;

  try {
    await saveImage(item.result.outputPath, destination);
    showNotice("Imagem salva com sucesso!");
  } catch (cause) {
    error.value = String(cause);
  }
}

async function saveAll() {
  if (busy.value) return;
  const done = items.value.filter((item) => item.result);
  if (done.length === 0) return;

  const directory = await openDialog({
    title: "Escolha a pasta para salvar os resultados",
    directory: true,
    multiple: false,
  });
  if (typeof directory !== "string") return;

  const used = new Set<string>();
  let saved = 0;

  for (const item of done) {
    if (!item.result) continue;
    const stem = fileName(item.info.path).replace(/\.[^.]+$/, "");
    let name = `${stem}-remaster-${item.result.scale}x.png`;
    let counter = 2;
    while (used.has(name)) {
      name = `${stem}-remaster-${item.result.scale}x-${counter}.png`;
      counter += 1;
    }
    used.add(name);

    try {
      await saveImage(item.result.outputPath, `${directory}/${name}`);
      saved += 1;
    } catch (cause) {
      error.value = String(cause);
    }
  }

  if (saved > 0) {
    showNotice(`${saved} imagem(ns) salva(s) em ${directory}`);
  }
}

function onKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;

  if (event.ctrlKey || event.metaKey) {
    const key = event.key.toLowerCase();
    if (key === "o") {
      event.preventDefault();
      void openImageDialog();
    } else if (key === "s") {
      event.preventDefault();
      void saveResult();
    }
    return;
  }

  if (event.code === "Space" && result.value) {
    event.preventDefault();
    viewerMode.value = viewerMode.value === "side" ? "slider" : "side";
  }
}

onMounted(async () => {
  await refreshRuntime();

  const preferred = runtime.value?.models.find(
    (option) => option.id === runtime.value?.defaultModel && option.available,
  );
  const fallback = runtime.value?.models.find((option) => option.available);
  model.value = preferred?.id ?? fallback?.id ?? runtime.value?.defaultModel ?? model.value;

  try {
    const startup = await startupInfo();
    if (startup.model) {
      model.value = startup.model;
    }
    if (startup.scale === 1 || startup.scale === 2 || startup.scale === 4) {
      scale.value = startup.scale;
    }
    if (startup.paths.length > 0) {
      await addPaths(startup.paths);
      if (startup.autoRun) {
        await runAll();
      }
    }
  } catch (cause) {
    error.value = String(cause);
  }

  const webview = getCurrentWebview();
  unlistenDrop = await webview.onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter" || payload.type === "over") {
      dragActive.value = true;
    } else if (payload.type === "drop") {
      dragActive.value = false;
      const files = payload.paths.filter((path) => IMAGE_PATTERN.test(path));
      if (files.length > 0) {
        void addPaths(files);
      } else if (payload.paths.length > 0) {
        error.value = "Arquivo não suportado. Use PNG, JPEG, WebP, BMP, GIF ou TIFF.";
      }
    } else {
      dragActive.value = false;
    }
  });

  window.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
  unlistenDrop?.();
  unlistenDrop = null;
  window.removeEventListener("keydown", onKeydown);
  if (noticeTimer) window.clearTimeout(noticeTimer);
});
</script>

<template>
  <div class="app">
    <Toolbar
      :has-image="!!original"
      :has-result="!!result"
      :busy="busy"
      :mode="mode"
      :scale="scale"
      :viewer-mode="viewerMode"
      :models="models"
      :model="model"
      @open="openImageDialog"
      @run="runSelected"
      @save="saveResult"
      @cancel="cancelJob"
      @update:mode="mode = $event"
      @update:scale="scale = $event"
      @update:viewer-mode="viewerMode = $event"
      @update:model="model = $event"
    />

    <ProgressBar :progress="progress" :busy="busy" />

    <div
      v-if="mode === 'ai' && selectedModel && !selectedModel.available"
      class="banner banner-warn"
    >
      <span>
        Modelo <strong>{{ selectedModel.label }}</strong> não encontrado. Rode
        <code>mise run model</code> no terminal e reinicie o app — ou use o motor
        <strong>Rápido</strong>.
      </span>
    </div>

    <div v-if="error" class="banner banner-error">
      <span>{{ error }}</span>
      <button class="banner-close" title="Fechar" @click="error = null">✕</button>
    </div>

    <div v-if="notice" class="banner banner-ok">{{ notice }}</div>

    <main class="workspace">
      <ImageList
        v-if="items.length > 1"
        :items="items"
        :selected-id="selectedId"
        :busy="busy"
        :batch="batch"
        @select="selectItem"
        @remove="removeItem"
        @add="openImageDialog"
        @clear="clearItems"
        @run-all="runAll"
        @save-all="saveAll"
      />

      <section class="main-panel">
        <div v-if="!selectedItem" class="empty-state">
          <div class="empty-icon">
            <svg viewBox="0 0 24 24" width="34" height="34" aria-hidden="true">
              <path d="M12 4l6 10h-4v6H10v-6H6z" fill="currentColor" />
            </svg>
          </div>
          <h1>Arraste imagens aqui</h1>
          <p>PNG, JPEG, WebP, BMP, GIF ou TIFF — até ~6 MP por imagem.</p>
          <button class="btn btn-primary" @click="openImageDialog">Escolher imagens</button>
          <p class="empty-hint">
            Várias imagens viram uma lista; clique em uma para comparar. Atalhos: Ctrl+O abrir ·
            Ctrl+S salvar · Espaço alterna lado a lado / slider
          </p>
        </div>

        <template v-else>
          <ComparisonViewer
            v-if="result"
            :original-url="originalUrl"
            :result-url="resultUrl"
            :original-width="original?.width ?? 0"
            :original-height="original?.height ?? 0"
            :result-width="result.width"
            :result-height="result.height"
            :mode="viewerMode"
          />

          <div v-else class="single-preview">
            <img :src="originalUrl" alt="Prévia da imagem original" />
            <div class="single-preview-caption">
              <strong>{{ original?.width }} × {{ original?.height }}</strong>
              <span>{{ original?.format.toUpperCase() }} · {{ formatBytes(original?.fileSize ?? 0) }}</span>
            </div>
            <p class="single-preview-hint">
              Clique em <strong>Remasterizar</strong> para gerar e comparar o resultado
            </p>
          </div>
        </template>
      </section>
    </main>

    <footer class="status-bar">
      <span v-if="selectedItem" class="status-item">{{ fileName(selectedItem.info.path) }}</span>
      <span v-if="selectedItem" class="status-item">
        {{ selectedItem.info.width }}×{{ selectedItem.info.height }} →
        {{ result ? `${result.width}×${result.height}` : "—" }}
      </span>
      <span v-if="result" class="status-item">
        {{ result.mode === "ai" ? modelLabel : "Rápido" }} {{ result.scale }}x ·
        {{ formatMs(result.elapsedMs) }} · {{ result.backend === "cuda" ? "GPU" : "CPU" }}
      </span>
      <span v-if="busy" class="status-item status-busy">
        {{ batch ? `imagem ${batch.current}/${batch.total}` : "processando…" }}
      </span>
      <span class="status-spacer"></span>
      <span class="status-item status-dim">
        {{ gpuReady ? "CUDA pronta" : "CPU" }} ·
        {{ selectedModel?.available ? "modelo ok" : "modelo ausente" }}
      </span>
    </footer>

    <div v-if="dragActive" class="drop-overlay">
      <div class="drop-overlay-inner">Solte as imagens para adicionar</div>
    </div>
  </div>
</template>
