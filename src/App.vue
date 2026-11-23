<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import ComparisonViewer from "./components/ComparisonViewer.vue";
import ContextMenu from "./components/ContextMenu.vue";
import ImageList from "./components/ImageList.vue";
import ProgressBar from "./components/ProgressBar.vue";
import Toolbar from "./components/Toolbar.vue";
import { useFlowPresets } from "./composables/useFlowPresets";
import { useImageJob } from "./composables/useImageJob";
import {
  cloneNodesWithNewIds,
  copyNodes,
  createRootNode,
  orderNodes,
} from "./lib/flow";
import type {
  BatchProgress,
  ComparisonLayout,
  ContextMenuItem,
  FlowNode,
  FlowResult,
  JobItem,
  RuntimeInfo,
  ViewerMode,
  ViewerPanel,
} from "./types";

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tif"];
const IMAGE_PATTERN = new RegExp(`\\.(${IMAGE_EXTENSIONS.join("|")})$`, "i");

const items = ref<JobItem[]>([]);
const selectedId = ref<string | null>(null);

type ContextTarget = {
  kind: "panel" | "item" | "save-all";
  id: string;
  label: string;
  x: number;
  y: number;
};

type SaveAllMode = "final" | "all" | "visible" | "selected";

const defaultNodes = ref<FlowNode[]>([createRootNode("qualidade")]);
const viewerMode = ref<ViewerMode>("side");
const layoutMode = ref<ComparisonLayout>("auto");
const originalVisible = ref(true);
const compareMode = ref(false);
const compareIds = ref<string[]>([]);
const sidebarOpen = ref(false);
const busy = ref(false);
const batch = ref<BatchProgress | null>(null);
const error = ref<string | null>(null);
const notice = ref<string | null>(null);
const dragActive = ref(false);
const runtime = ref<RuntimeInfo | null>(null);

const { progress, loadImage, upscaleFlow, saveImage, cancel, runtimeInfo, startupInfo } =
  useImageJob();
const { presets, savePreset, removePreset } = useFlowPresets();

const contextMenu = ref<ContextTarget | null>(null);
const flowClipboard = ref<FlowNode[] | null>(null);

const models = computed(() => runtime.value?.models ?? []);
const gpuReady = computed(() => runtime.value?.gpu.active ?? false);
const canCompare = computed(() => items.value.length >= 2);
const hasAnyResult = computed(() => items.value.some((item) => !!item.result));

const selectedItem = computed(
  () => items.value.find((item) => item.id === selectedId.value) ?? null,
);
/** Árvore exibida/editada na toolbar: a da imagem selecionada. */
const nodes = computed<FlowNode[]>(() => selectedItem.value?.nodes ?? defaultNodes.value);
const roots = computed(() => nodes.value.filter((node) => node.parentId === null));
const hiddenNodeCount = computed(() => nodes.value.filter((node) => !node.visible).length);
const resultTimes = computed(() => {
  const times: Record<string, number> = {};
  for (const entry of selectedItem.value?.results ?? []) {
    times[entry.nodeId] = entry.elapsedMs;
  }
  return times;
});
const resultUrls = computed(() => {
  const urls: Record<string, string> = {};
  for (const entry of selectedItem.value?.results ?? []) {
    urls[entry.nodeId] = convertFileSrc(entry.outputPath);
  }
  return urls;
});
const missingStepModel = computed(() => {
  for (const node of nodes.value) {
    if (node.mode !== "ai") continue;
    const info = models.value.find((option) => option.id === node.model);
    if (info && !info.available) return info;
  }
  return null;
});

const original = computed(() => selectedItem.value?.info ?? null);
const result = computed(() => selectedItem.value?.result ?? null);
const originalUrl = computed(() => (original.value ? convertFileSrc(original.value.path) : ""));
const showSidebar = computed(() => sidebarOpen.value && items.value.length > 0);

function modelLabel(id: string) {
  return models.value.find((option) => option.id === id)?.label ?? id;
}

function resultLabel(entry: FlowResult) {
  const engine = entry.mode === "classic" ? "Rápido" : modelLabel(entry.model);
  if (entry.targetWidth && entry.targetHeight) {
    if (!entry.usedModel && entry.mode !== "classic") {
      return `alvo ${entry.targetWidth}×${entry.targetHeight} · redução`;
    }
    return `alvo ${entry.targetWidth}×${entry.targetHeight} · ${engine}`;
  }
  return `${entry.scale}x ${engine}`;
}

function resultTag(entry: FlowResult) {
  return entry.targetWidth && entry.targetHeight
    ? `${entry.width}x${entry.height}`
    : `${entry.scale}x`;
}

const viewerPanels = computed<ViewerPanel[]>(() => {
  if (compareMode.value && compareIds.value.length >= 2) {
    return compareIds.value
      .map((id) => items.value.find((item) => item.id === id))
      .filter((item): item is JobItem => !!item)
      .map((item) => {
        const last = item.result;
        return {
          id: item.id,
          hideable: true,
          url: convertFileSrc(last ? last.outputPath : item.info.path),
          label: last
            ? `${fileName(item.info.path)} · ${resultLabel(last)}`
            : fileName(item.info.path),
          sub: `${last ? last.width : item.info.width}×${last ? last.height : item.info.height}`,
          width: last ? last.width : item.info.width,
          height: last ? last.height : item.info.height,
        };
      });
  }

  const item = selectedItem.value;
  if (!item) return [];

  const resultByNode = new Map(item.results.map((entry) => [entry.nodeId, entry]));
  const panels: ViewerPanel[] = [];

  if (originalVisible.value) {
    panels.push({
      id: "original",
      hideable: item.results.length > 0,
      url: originalUrl.value,
      label: "Original",
      sub: `${item.info.width}×${item.info.height}`,
      width: item.info.width,
      height: item.info.height,
    });
  }

  for (const node of orderNodes(item.nodes)) {
    if (!node.visible) continue;
    const entry = resultByNode.get(node.id);
    if (!entry) continue;
    const parent = node.parentId
      ? item.nodes.find((candidate) => candidate.id === node.parentId)
      : null;
    panels.push({
      id: node.id,
      hideable: true,
      url: convertFileSrc(entry.outputPath),
      label: `Nó ${node.name} · ${resultLabel(entry)}`,
      sub: `entrada: ${parent ? `Nó ${parent.name}` : "Original"} ${entry.inputWidth}×${entry.inputHeight}`,
      width: entry.width,
      height: entry.height,
    });
  }

  return panels;
});

const totalElapsed = computed(() =>
  (selectedItem.value?.results ?? []).reduce((total, entry) => total + entry.elapsedMs, 0),
);

const contextItems = computed<ContextMenuItem[]>(() => {
  const menu = contextMenu.value;
  if (!menu) return [];

  if (menu.kind === "item") {
    return [
      { id: "copy-flow", label: "Copiar fluxo" },
      { id: "paste-flow", label: "Colar fluxo", disabled: !flowClipboard.value },
      { id: "remove-item", label: "Remover da lista", danger: true },
    ];
  }

  if (menu.kind === "save-all") {
    return [
      { id: "final", label: "Só o resultado final de cada imagem" },
      { id: "selected", label: "Apenas os nós marcados para salvar" },
      { id: "all", label: "Todos os nós (todos os ramos)" },
      { id: "visible", label: "Apenas os nós visíveis" },
    ];
  }

  const items: ContextMenuItem[] = [{ id: "save", label: "Salvar imagem…" }];
  if (
    compareMode.value ||
    menu.id !== "original" ||
    (selectedItem.value?.results.length ?? 0) > 0
  ) {
    items.push({ id: "hide", label: "Ocultar painel" });
  }
  if (!compareMode.value && (hiddenNodeCount.value > 0 || !originalVisible.value)) {
    items.push({ id: "show-all", label: "Mostrar todos os painéis" });
  }
  return items;
});

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

/** Atualiza a árvore da imagem selecionada e a usa como modelo para novas imagens. */
function applyNodes(next: FlowNode[]) {
  const copy = copyNodes(next);
  defaultNodes.value = copyNodes(copy);
  const item = selectedItem.value;
  if (item) item.nodes = copy;
}

async function addPaths(paths: string[]) {
  if (paths.length === 0) return;
  error.value = null;

  const hadItems = items.value.length > 0;
  const added: string[] = [];
  for (const path of paths) {
    if (items.value.some((item) => item.info.path === path)) continue;
    try {
      const info = await loadImage(path);
      const item: JobItem = {
        id: newId(),
        info,
        nodes: cloneNodesWithNewIds(defaultNodes.value),
        result: null,
        results: [],
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
    // A lista abre sozinha para continuar o trabalho com a nova imagem.
    sidebarOpen.value = true;
    selectedId.value = added[0];
    if (hadItems) {
      showNotice(`${added.length} imagem(ns) adicionada(s) à lista.`);
    }
    if (compareMode.value && compareIds.value.length < 2) {
      compareIds.value = items.value.slice(0, 4).map((item) => item.id);
    }
  } else if (paths.length > 0) {
    showNotice("Essa(s) imagem(ns) já está(ão) na lista.");
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
  compareIds.value = compareIds.value.filter((value) => value !== id);
  if (selectedId.value === id) {
    selectedId.value = items.value[Math.min(index, items.value.length - 1)]?.id ?? null;
  }
  if (items.value.length < 2) {
    compareMode.value = false;
  }
}

function clearItems() {
  if (busy.value) return;
  items.value = [];
  selectedId.value = null;
  compareIds.value = [];
  compareMode.value = false;
  progress.value = null;
  error.value = null;
}

function toggleCompareMode() {
  if (busy.value) return;
  compareMode.value = !compareMode.value;
  if (compareMode.value) {
    sidebarOpen.value = true;
    compareIds.value = items.value.slice(0, 4).map((item) => item.id);
    if (compareIds.value.length < 2) {
      showNotice("Carregue pelo menos duas imagens para comparar lado a lado.");
    }
  }
}

function toggleCompare(id: string) {
  if (compareIds.value.includes(id)) {
    compareIds.value = compareIds.value.filter((value) => value !== id);
  } else {
    compareIds.value = [...compareIds.value, id];
  }
}

function selectAllCompare() {
  compareIds.value = items.value.map((item) => item.id);
}

function clearCompare() {
  compareIds.value = [];
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

function checkModel(item: JobItem) {
  if (models.value.length === 0) return true;

  for (const node of item.nodes) {
    if (node.mode !== "ai") continue;
    const info = models.value.find((option) => option.id === node.model);
    if (info && !info.available) {
      error.value = `Modelo "${info.label}" não encontrado (nó ${node.name} de ${fileName(item.info.path)}). Rode \`mise run model\` no terminal e reinicie o app (ou use o motor Rápido).`;
      return false;
    }
  }
  return true;
}

async function processItem(item: JobItem) {
  if (!checkModel(item)) {
    item.status = "error";
    item.error = error.value;
    return;
  }

  item.status = "processing";
  item.error = null;
  progress.value = null;

  try {
    const outputs = await upscaleFlow(item.info.path, copyNodes(item.nodes));
    item.results = outputs;
    item.result = outputs[outputs.length - 1] ?? null;
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

async function runItem(id: string) {
  if (busy.value) return;
  const item = items.value.find((candidate) => candidate.id === id);
  if (!item || item.status === "processing") return;

  selectedId.value = id;
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
  for (const item of items.value) {
    if (!checkModel(item)) return;
  }

  let targets = items.value.filter((item) => !item.result);
  if (targets.length === 0) {
    targets = [...items.value]; // reprocessa tudo ao mudar o fluxo
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

async function exportImage(sourcePath: string, defaultPath: string, title = "Salvar imagem") {
  const destination = await saveDialog({
    title,
    defaultPath,
    filters: [
      { name: "PNG", extensions: ["png"] },
      { name: "JPEG", extensions: ["jpg", "jpeg"] },
      { name: "WebP", extensions: ["webp"] },
    ],
  });

  if (!destination) return;

  try {
    await saveImage(sourcePath, destination);
    showNotice("Imagem salva com sucesso!");
  } catch (cause) {
    error.value = String(cause);
  }
}

async function saveResult() {
  if (busy.value) return;
  const item = selectedItem.value;
  if (!item?.result) return;
  const stem = fileName(item.info.path).replace(/\.[^.]+$/, "");
  await exportImage(
    item.result.outputPath,
    `${stem}-remaster-${resultTag(item.result)}.png`,
    "Salvar imagem remasterizada",
  );
}

async function saveItem(id: string) {
  if (busy.value) return;
  const item = items.value.find((candidate) => candidate.id === id);
  if (!item?.result) return;
  const stem = fileName(item.info.path).replace(/\.[^.]+$/, "");
  await exportImage(
    item.result.outputPath,
    `${stem}-remaster-${resultTag(item.result)}.png`,
    "Salvar imagem remasterizada",
  );
}

async function savePanelImage(panelId: string) {
  if (busy.value) return;

  if (compareMode.value) {
    const target = items.value.find((item) => item.id === panelId);
    if (!target) return;
    const stem = fileName(target.info.path).replace(/\.[^.]+$/, "");
    const last = target.result;
    await exportImage(
      last?.outputPath ?? target.info.path,
      `${stem}-${last ? `remaster-${resultTag(last)}` : "original"}.png`,
      "Salvar imagem",
    );
    return;
  }

  const item = selectedItem.value;
  if (!item) return;
  const stem = fileName(item.info.path).replace(/\.[^.]+$/, "");

  if (panelId === "original") {
    await exportImage(item.info.path, `${stem}-original.png`, "Salvar imagem original");
    return;
  }

  const entry = item.results.find((candidate) => candidate.nodeId === panelId);
  if (!entry) return;
  const node = item.nodes.find((candidate) => candidate.id === panelId);
  await exportImage(
    entry.outputPath,
    `${stem}-no-${node?.name ?? panelId}-${resultTag(entry)}.png`,
    "Salvar resultado do nó",
  );
}

function showAllPanels() {
  originalVisible.value = true;
  const item = selectedItem.value;
  if (!item) return;
  for (const node of item.nodes) node.visible = true;
}

function openPanelMenu(payload: { id: string; label: string; x: number; y: number }) {
  contextMenu.value = { kind: "panel", ...payload };
}

function openItemMenu(payload: { id: string; label: string; x: number; y: number }) {
  contextMenu.value = { kind: "item", ...payload };
}

function openSaveAllMenu(event: MouseEvent) {
  contextMenu.value = {
    kind: "save-all",
    id: "save-all",
    label: "Salvar todas as imagens",
    x: event.clientX,
    y: event.clientY,
  };
}

function copyFlow(item?: JobItem | null) {
  const target = item ?? selectedItem.value;
  if (!target) return;
  flowClipboard.value = copyNodes(target.nodes);
  showNotice(`Fluxo de ${fileName(target.info.path)} copiado (${target.nodes.length} nó(s)).`);
}

function pasteFlow(item?: JobItem | null) {
  const target = item ?? selectedItem.value;
  if (!target || !flowClipboard.value) return;

  target.nodes = cloneNodesWithNewIds(flowClipboard.value);
  target.results = [];
  target.result = null;
  target.status = "pending";
  target.error = null;

  selectedId.value = target.id;
  sidebarOpen.value = true;
  showNotice(`Fluxo colado em ${fileName(target.info.path)}. Clique em Remasterizar.`);
}

async function handleContextAction(action: string) {
  const menu = contextMenu.value;
  if (!menu) return;
  contextMenu.value = null;

  if (menu.kind === "save-all") {
    await saveAll(action as SaveAllMode);
    return;
  }

  if (menu.kind === "item") {
    const item = items.value.find((candidate) => candidate.id === menu.id);
    if (!item) return;
    if (action === "copy-flow") {
      copyFlow(item);
    } else if (action === "paste-flow") {
      pasteFlow(item);
    } else if (action === "remove-item") {
      removeItem(item.id);
    }
    return;
  }

  if (action === "save") {
    await savePanelImage(menu.id);
  } else if (action === "hide") {
    hidePanel(menu.id);
  } else if (action === "show-all") {
    showAllPanels();
  }
}

function handleSavePreset(name: string) {
  const item = selectedItem.value;
  if (!item) return;
  savePreset(name, item.nodes);
  showNotice(`Preset "${name}" salvo.`);
}

function handleApplyPreset(id: string) {
  const preset = presets.value.find((candidate) => candidate.id === id);
  if (!preset) return;

  const item = selectedItem.value;
  if (!item) {
    defaultNodes.value = cloneNodesWithNewIds(preset.nodes);
    showNotice(`Preset "${preset.name}" definido como fluxo padrão.`);
    return;
  }

  item.nodes = cloneNodesWithNewIds(preset.nodes);
  item.results = [];
  item.result = null;
  item.status = "pending";
  item.error = null;
  showNotice(`Preset "${preset.name}" aplicado em ${fileName(item.info.path)}.`);
}

function handleDeletePreset(id: string) {
  const preset = presets.value.find((candidate) => candidate.id === id);
  removePreset(id);
  if (preset) showNotice(`Preset "${preset.name}" removido.`);
}

function hidePanel(id: string) {
  if (compareMode.value) {
    compareIds.value = compareIds.value.filter((value) => value !== id);
    if (compareIds.value.length < 2) {
      compareMode.value = false;
      showNotice("Comparação encerrada: restou menos de duas imagens visíveis.");
    }
    return;
  }

  if (id === "original") {
    const item = selectedItem.value;
    if (!item || item.results.length === 0) return;
    originalVisible.value = false;
    showNotice("Original oculto. Use Visão → Original ou Mostrar todos para restaurar.");
    return;
  }
  const item = selectedItem.value;
  const node = item?.nodes.find((candidate) => candidate.id === id);
  if (node) {
    node.visible = false;
    showNotice(`Nó ${node.name} oculto do comparador. Use Fluxo → Mostrar todos para restaurar.`);
  }
}

async function saveAll(mode: SaveAllMode = "final") {
  if (busy.value) return;

  const directory = await openDialog({
    title: "Escolha a pasta para salvar os resultados",
    directory: true,
    multiple: false,
  });
  if (typeof directory !== "string") return;

  const used = new Set<string>();
  let saved = 0;
  let failed = 0;

  const uniqueName = (base: string) => {
    let name = `${base}.png`;
    let counter = 2;
    while (used.has(name)) {
      name = `${base}-${counter}.png`;
      counter += 1;
    }
    used.add(name);
    return name;
  };

  for (const item of items.value) {
    const stem = fileName(item.info.path).replace(/\.[^.]+$/, "");
    const nodeById = new Map(item.nodes.map((node) => [node.id, node]));

    let entries: Array<{ nodeName: string; tag: string; path: string }> = [];

    if (mode === "final") {
      if (item.result) {
        entries.push({
          nodeName: "remaster",
          tag: resultTag(item.result),
          path: item.result.outputPath,
        });
      }
    } else {
      entries = item.results
        .filter((entry) => {
          const node = nodeById.get(entry.nodeId);
          if (!node) return false;
          if (mode === "all") return true;
          if (mode === "visible") return node.visible;
          if (mode === "selected") return node.save;
          return true;
        })
        .map((entry) => ({
          nodeName: `no-${nodeById.get(entry.nodeId)?.name ?? entry.nodeId}`,
          tag: resultTag(entry),
          path: entry.outputPath,
        }));
    }

    for (const entry of entries) {
      const name = uniqueName(`${stem}-${entry.nodeName}-${entry.tag}`);
      try {
        await saveImage(entry.path, `${directory}/${name}`);
        saved += 1;
      } catch (cause) {
        error.value = String(cause);
        failed += 1;
      }
    }
  }

  if (saved > 0) {
    showNotice(
      `${saved} arquivo(s) salvo(s) em ${directory}${failed > 0 ? ` (${failed} falha(s))` : ""}`,
    );
  } else if (failed === 0) {
    showNotice("Nenhum resultado disponível para essa opção de salvamento.");
  }
}

function onContextMenu(event: MouseEvent) {
  // Remove o menu de contexto padrão do WebView (Voltar, Recarregar, etc.).
  event.preventDefault();
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

  if (event.code === "Space" && result.value && !compareMode.value) {
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
  const defaultModel = preferred?.id ?? fallback?.id ?? runtime.value?.defaultModel ?? "qualidade";
  defaultNodes.value[0].model = defaultModel;

  try {
    const startup = await startupInfo();
    if (startup.model) {
      defaultNodes.value[0].model = startup.model;
    }
    if (startup.scale === 1 || startup.scale === 2 || startup.scale === 4) {
      defaultNodes.value[0].scale = startup.scale;
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
  window.addEventListener("contextmenu", onContextMenu);
});

onUnmounted(() => {
  unlistenDrop?.();
  unlistenDrop = null;
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("contextmenu", onContextMenu);
  if (noticeTimer) window.clearTimeout(noticeTimer);
});
</script>

<template>
  <div class="app">
    <Toolbar
      :has-result="!!result"
      :busy="busy"
      :viewer-mode="viewerMode"
      :models="models"
      :nodes="nodes"
      :result-times="resultTimes"
      :result-urls="resultUrls"
      :presets="presets"
      :compare-mode="compareMode"
      :can-compare="canCompare"
      :can-slider="viewerPanels.length === 2"
      :can-layout="viewerPanels.length >= 2"
      :layout-mode="layoutMode"
      :image-name="selectedItem ? fileName(selectedItem.info.path) : ''"
      :image-url="originalUrl"
      :image-width="original?.width ?? 0"
      :image-height="original?.height ?? 0"
      :sidebar-open="sidebarOpen"
      :can-toggle-sidebar="items.length > 0"
      :can-run-all="items.length > 0"
      :has-any-result="hasAnyResult"
      :original-visible="originalVisible"
      @open="openImageDialog"
      @run-all="runAll"
      @save-all-menu="openSaveAllMenu"
      @cancel="cancelJob"
      @update:viewer-mode="viewerMode = $event"
      @update:layout-mode="layoutMode = $event"
      @update:nodes="applyNodes"
      @save-preset="handleSavePreset"
      @apply-preset="handleApplyPreset"
      @delete-preset="handleDeletePreset"
      @toggle-compare="toggleCompareMode"
      @toggle-sidebar="sidebarOpen = !sidebarOpen"
      @toggle-original="originalVisible = !originalVisible"
    />

    <ProgressBar :progress="progress" :busy="busy" />

    <div v-if="missingStepModel" class="banner banner-warn">
      <span>
        Modelo <strong>{{ missingStepModel.label }}</strong> não encontrado. Rode
        <code>mise run model</code> no terminal e reinicie o app — ou use o motor
        <strong>Rápido</strong> no nó.
      </span>
    </div>

    <div v-if="error" class="banner banner-error">
      <span>{{ error }}</span>
      <button class="banner-close" title="Fechar" @click="error = null">✕</button>
    </div>

    <div v-if="notice" class="banner banner-ok">{{ notice }}</div>

    <main class="workspace">
      <ImageList
        v-if="showSidebar"
        :items="items"
        :selected-id="selectedId"
        :busy="busy"
        :compare-mode="compareMode"
        :compare-ids="compareIds"
        :models="models"
        @select="selectItem"
        @remove="removeItem"
        @add="openImageDialog"
        @clear="clearItems"
        @run-item="runItem"
        @save-item="saveItem"
        @toggle-compare="toggleCompare"
        @select-all-compare="selectAllCompare"
        @clear-compare="clearCompare"
        @item-menu="openItemMenu"
      />

      <section class="main-panel">
        <div v-if="!selectedItem" class="empty-state">
          <div class="empty-icon">
            <svg viewBox="0 0 24 24" width="34" height="34" aria-hidden="true">
              <path d="M12 4l6 10h-4v6H10v-6H6z" fill="currentColor" />
            </svg>
          </div>
          <h1>Arraste imagens aqui</h1>
          <p>PNG, JPEG, WebP, BMP, GIF ou TIFF — até ~32 MP por imagem.</p>
          <button class="btn btn-primary" @click="openImageDialog">Escolher imagens</button>
          <p class="empty-hint">
            Várias imagens viram uma lista; use <strong>Comparar</strong> para ver várias lado a
            lado. Monte ramificações no menu <strong>Fluxo</strong>. Atalhos: Ctrl+O abrir · Ctrl+S
            salvar · Espaço alterna lado a lado / slider
          </p>
        </div>

        <template v-else>
          <ComparisonViewer
            v-if="
              viewerPanels.length >= 2 ||
              (viewerPanels.length >= 1 && (selectedItem?.results.length ?? 0) > 0)
            "
            :panels="viewerPanels"
            :layout="layoutMode"
            :mode="viewerMode === 'slider' && viewerPanels.length === 2 ? 'slider' : 'side'"
            @hide="hidePanel"
            @panel-menu="openPanelMenu"
          />

          <div
            v-else-if="viewerPanels.length === 0 && (selectedItem?.results.length ?? 0) > 0"
            class="single-preview panels-empty"
          >
            <p>Nenhum painel visível no momento.</p>
            <button class="btn" @click="showAllPanels">Mostrar todos os painéis</button>
          </div>

          <div v-else class="single-preview">
            <img :src="originalUrl" alt="Prévia da imagem original" />
            <div class="single-preview-caption">
              <strong>{{ original?.width }} × {{ original?.height }}</strong>
              <span>{{ original?.format.toUpperCase() }} · {{ formatBytes(original?.fileSize ?? 0) }}</span>
            </div>
            <p class="single-preview-hint">
              Clique em <strong>Remasterizar</strong> para gerar e comparar o resultado
              <template v-if="compareMode"> — ou marque imagens na lista para comparar</template>
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
      <span v-if="result && selectedItem" class="status-item">
        {{ selectedItem.results.length }} nó(s) processado(s) · {{ formatMs(totalElapsed) }} ·
        {{ result.backend === "cuda" ? "GPU" : "CPU" }}
      </span>
      <span v-if="compareMode" class="status-item status-busy">
        comparando {{ compareIds.length }} imagem(ns)
      </span>
      <span v-if="busy" class="status-item status-busy">
        {{ batch ? `imagem ${batch.current}/${batch.total}` : "processando…" }}
      </span>
      <span class="status-spacer"></span>
      <span class="status-item status-dim">
        {{ gpuReady ? "CUDA pronta" : "CPU" }} ·
        {{ roots.length }} ramo(s) · {{ nodes.length }} nó(s)<template v-if="hiddenNodeCount">
          · {{ hiddenNodeCount }} oculto(s)</template
        >
      </span>
    </footer>

    <div v-if="dragActive" class="drop-overlay">
      <div class="drop-overlay-inner">Solte as imagens para adicionar</div>
    </div>

    <ContextMenu
      v-if="contextMenu"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :title="contextMenu.label"
      :items="contextItems"
      @select="handleContextAction"
      @close="contextMenu = null"
    />
  </div>
</template>
