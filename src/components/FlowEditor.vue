<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";

import FlowNodeCard from "./FlowNodeCard.vue";
import type { FlowNode, FlowPreset, ModelInfo } from "../types";
import {
  collectSubtree,
  descendantCount,
  layoutFlowTree,
  newNodeId,
  nextNodeName,
} from "../lib/flow";

const props = defineProps<{
  nodes: FlowNode[];
  models: ModelInfo[];
  busy: boolean;
  imageName: string;
  imageUrl: string;
  imageWidth: number;
  imageHeight: number;
  resultTimes: Record<string, number>;
  resultUrls: Record<string, string>;
  presets: FlowPreset[];
}>();

const emit = defineEmits<{
  (event: "update:nodes", value: FlowNode[]): void;
  (event: "save-preset", name: string): void;
  (event: "apply-preset", id: string): void;
  (event: "delete-preset", id: string): void;
}>();

const open = ref(false);
const presetsOpen = ref(false);
const presetName = ref("");
const graphScroll = ref<HTMLDivElement | null>(null);

const initialWidth = Math.min(1180, Math.round(window.innerWidth * 0.94));
const initialHeight = Math.min(840, Math.round(window.innerHeight * 0.9));
const panelWidth = ref(Math.max(520, initialWidth));
const panelHeight = ref(Math.max(420, initialHeight));
const panelLeft = ref(Math.max(12, Math.round((window.innerWidth - panelWidth.value) / 2)));
const panelTop = ref(Math.max(12, Math.round((window.innerHeight - panelHeight.value) / 2)));

const isMoving = ref(false);
const MIN_WIDTH = 520;
const MIN_HEIGHT = 420;
const MODAL_MARGIN = 12;

let moving: { startX: number; startY: number; startLeft: number; startTop: number } | null = null;
let resizing: {
  dir: string;
  startX: number;
  startY: number;
  startW: number;
  startH: number;
  startLeft: number;
  startTop: number;
} | null = null;

const roots = computed(() => props.nodes.filter((node) => node.parentId === null));
const hiddenCount = computed(() => props.nodes.filter((node) => !node.visible).length);
const layout = computed(() => layoutFlowTree(props.nodes));

function defaultModel() {
  return props.models.find((model) => model.available)?.id ?? props.models[0]?.id ?? "qualidade";
}

function compactSummary(node: FlowNode) {
  if (node.targetWidth && node.targetHeight) {
    return `alvo ${node.targetWidth}×${node.targetHeight}`;
  }
  return `${node.scale}x`;
}

function commit(next: FlowNode[]) {
  emit("update:nodes", next);
}

function update(id: string, patch: Partial<FlowNode>) {
  commit(props.nodes.map((node) => (node.id === id ? { ...node, ...patch } : node)));
}

function addChild(parentId: string | null) {
  const parent = parentId ? props.nodes.find((node) => node.id === parentId) : null;
  const template = parent ?? props.nodes[0];
  const node: FlowNode = {
    id: newNodeId(),
    name: nextNodeName(props.nodes, parentId),
    parentId,
    mode: template?.mode ?? "ai",
    scale: 1,
    targetWidth: null,
    targetHeight: null,
    model: template?.model || defaultModel(),
    save: true,
    visible: true,
  };
  commit([...props.nodes, node]);
}

function removeNode(id: string) {
  if (props.nodes.length <= 1) return;
  const removed = collectSubtree(props.nodes, id);
  const next = props.nodes.filter((node) => !removed.has(node.id));
  if (next.length === 0) return;
  commit(next);
}

function toggleVisible(id: string) {
  const node = props.nodes.find((item) => item.id === id);
  if (node) update(id, { visible: !node.visible });
}

function toggleSave(id: string) {
  const node = props.nodes.find((item) => item.id === id);
  if (node) update(id, { save: !node.save });
}

function showAll() {
  commit(props.nodes.map((node) => ({ ...node, visible: true })));
}

function presetRoots(preset: FlowPreset) {
  return preset.nodes.filter((node) => node.parentId === null).length;
}

function savePreset() {
  const name = presetName.value.trim();
  if (!name) return;
  emit("save-preset", name);
  presetName.value = "";
}

function edgePath(edge: { x1: number; y1: number; x2: number; y2: number }) {
  const middle = (edge.y1 + edge.y2) / 2;
  return `M ${edge.x1} ${edge.y1} C ${edge.x1} ${middle}, ${edge.x2} ${middle}, ${edge.x2} ${edge.y2}`;
}

async function centerGraph() {
  await nextTick();
  const element = graphScroll.value;
  if (!element) return;
  element.scrollLeft = Math.max(0, (element.scrollWidth - element.clientWidth) / 2);
  element.scrollTop = 0;
}

function openEditor() {
  clampPanel();
  open.value = true;
  void centerGraph();
}

function clampPanel() {
  panelWidth.value = Math.min(
    panelWidth.value,
    Math.max(MIN_WIDTH, window.innerWidth - MODAL_MARGIN * 2),
  );
  panelHeight.value = Math.min(
    panelHeight.value,
    Math.max(MIN_HEIGHT, window.innerHeight - MODAL_MARGIN * 2),
  );
  panelLeft.value = Math.min(
    Math.max(MODAL_MARGIN, panelLeft.value),
    Math.max(MODAL_MARGIN, window.innerWidth - panelWidth.value - MODAL_MARGIN),
  );
  panelTop.value = Math.min(
    Math.max(MODAL_MARGIN, panelTop.value),
    Math.max(MODAL_MARGIN, window.innerHeight - panelHeight.value - MODAL_MARGIN),
  );
}

function startMove(event: PointerEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, textarea, a")) return;
  event.preventDefault();
  moving = {
    startX: event.clientX,
    startY: event.clientY,
    startLeft: panelLeft.value,
    startTop: panelTop.value,
  };
  isMoving.value = true;
  window.addEventListener("pointermove", onMoveMove);
  window.addEventListener("pointerup", stopInteraction);
}

function onMoveMove(event: PointerEvent) {
  if (!moving) return;
  const maxLeft = Math.max(
    MODAL_MARGIN,
    window.innerWidth - panelWidth.value - MODAL_MARGIN,
  );
  const maxTop = Math.max(
    MODAL_MARGIN,
    window.innerHeight - panelHeight.value - MODAL_MARGIN,
  );
  panelLeft.value = Math.min(
    maxLeft,
    Math.max(MODAL_MARGIN, moving.startLeft + event.clientX - moving.startX),
  );
  panelTop.value = Math.min(
    maxTop,
    Math.max(MODAL_MARGIN, moving.startTop + event.clientY - moving.startY),
  );
}

function startResize(event: PointerEvent, dir: string) {
  event.preventDefault();
  event.stopPropagation();
  resizing = {
    dir,
    startX: event.clientX,
    startY: event.clientY,
    startW: panelWidth.value,
    startH: panelHeight.value,
    startLeft: panelLeft.value,
    startTop: panelTop.value,
  };
  window.addEventListener("pointermove", onResizeMove);
  window.addEventListener("pointerup", stopInteraction);
}

function onResizeMove(event: PointerEvent) {
  if (!resizing) return;
  const dx = event.clientX - resizing.startX;
  const dy = event.clientY - resizing.startY;
  const right = resizing.startLeft + resizing.startW;
  const bottom = resizing.startTop + resizing.startH;

  if (resizing.dir.includes("w")) {
    panelLeft.value = Math.max(
      MODAL_MARGIN,
      Math.min(right - MIN_WIDTH, resizing.startLeft + dx),
    );
    panelWidth.value = right - panelLeft.value;
  } else if (resizing.dir.includes("e")) {
    panelWidth.value = Math.max(
      MIN_WIDTH,
      Math.min(
        resizing.startW + dx,
        window.innerWidth - MODAL_MARGIN - panelLeft.value,
      ),
    );
  }

  if (resizing.dir.includes("n")) {
    panelTop.value = Math.max(
      MODAL_MARGIN,
      Math.min(bottom - MIN_HEIGHT, resizing.startTop + dy),
    );
    panelHeight.value = bottom - panelTop.value;
  } else if (resizing.dir.includes("s")) {
    panelHeight.value = Math.max(
      MIN_HEIGHT,
      Math.min(
        resizing.startH + dy,
        window.innerHeight - MODAL_MARGIN - panelTop.value,
      ),
    );
  }
}

function stopInteraction() {
  moving = null;
  resizing = null;
  isMoving.value = false;
  window.removeEventListener("pointermove", onMoveMove);
  window.removeEventListener("pointermove", onResizeMove);
  window.removeEventListener("pointerup", stopInteraction);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && open.value) open.value = false;
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", clampPanel);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", clampPanel);
  stopInteraction();
});
</script>

<template>
  <div class="pipeline">
    <span class="group-label">Fluxo</span>
    <button
      class="pipeline-open"
      :disabled="busy"
      :title="`${nodes.length} nó(s) em ${roots.length} ramo(s) — clique para abrir o editor`"
      @click="openEditor"
    >
      <span v-for="root in roots.slice(0, 2)" :key="root.id" class="pipeline-chip">
        <b>{{ root.name }}</b>
        {{ compactSummary(root) }}
        <span v-if="descendantCount(nodes, root.id) > 0" class="pipeline-branch">
          +{{ descendantCount(nodes, root.id) }}
        </span>
      </span>
      <span v-if="roots.length > 2" class="pipeline-more">+{{ roots.length - 2 }} ramos</span>
      <span class="pipeline-count">{{ nodes.length }} nós</span>
      <span class="pipeline-edit">abrir</span>
    </button>

    <Teleport to="body">
      <div v-if="open" class="flow-modal">
        <div class="flow-modal-backdrop" @click="open = false"></div>
        <div
          class="flow-modal-frame"
          :class="{ moving: isMoving }"
          :style="{
            left: `${panelLeft}px`,
            top: `${panelTop}px`,
            width: `${panelWidth}px`,
            height: `${panelHeight}px`,
          }"
        >
          <section class="flow-modal-panel">
            <header class="flow-modal-head" @pointerdown="startMove">
              <div class="flow-head-text">
                <strong>{{ imageName ? `Fluxo de ${imageName}` : "Fluxo de remasterização" }}</strong>
                <span>
                  {{ nodes.length }} nó(s) · {{ roots.length }} ramo(s)
                </span>
              </div>
              <div class="flow-modal-actions">
                <button
                  class="btn btn-mini"
                  :class="{ 'btn-active': presetsOpen }"
                  @click="presetsOpen = !presetsOpen"
                >
                  Presets
                </button>
                <button class="icon-btn" title="Fechar" @click="open = false">×</button>
              </div>
            </header>

          <section v-if="presetsOpen" class="flow-presets flow-presets-panel">
            <div class="flow-presets-head">
              <span class="group-label">Presets</span>
              <input
                v-model="presetName"
                class="flow-preset-input"
                placeholder="Nome do preset"
                :disabled="busy"
                @keydown.enter.prevent="savePreset"
              />
              <button
                class="btn flow-preset-save"
                :disabled="busy || !presetName.trim()"
                @click="savePreset"
              >
                Salvar atual
              </button>
            </div>

            <div v-if="presets.length > 0" class="flow-preset-list">
              <div v-for="preset in presets" :key="preset.id" class="flow-preset">
                <div class="flow-preset-name">
                  <strong>{{ preset.name }}</strong>
                  <span>{{ preset.nodes.length }} nó(s) · {{ presetRoots(preset) }} ramo(s)</span>
                </div>
                <div class="flow-preset-actions">
                  <button class="btn btn-mini" :disabled="busy" @click="emit('apply-preset', preset.id)">
                    Aplicar
                  </button>
                  <button
                    class="icon-btn"
                    title="Excluir preset"
                    :disabled="busy"
                    @click="emit('delete-preset', preset.id)"
                  >
                    ×
                  </button>
                </div>
              </div>
            </div>
            <p v-else class="flow-presets-empty">
              Salve o fluxo atual com um nome para reutilizar em outras imagens.
            </p>
          </section>

          <div class="flow-modal-body">
            <div ref="graphScroll" class="flow-graph-scroll">
              <div
                class="flow-graph"
                :style="{ width: `${layout.width}px`, height: `${layout.height}px` }"
              >
                <svg
                  class="flow-edges"
                  :width="layout.width"
                  :height="layout.height"
                  aria-hidden="true"
                >
                  <defs>
                    <marker
                      id="flow-arrow"
                      viewBox="0 0 8 8"
                      refX="7"
                      refY="4"
                      markerWidth="7"
                      markerHeight="7"
                      orient="auto-start-reverse"
                    >
                      <path d="M 0 0 L 8 4 L 0 8 z" fill="rgba(56, 189, 248, 0.7)" />
                    </marker>
                  </defs>
                  <path
                    v-for="edge in layout.edges"
                    :key="`${edge.from ?? 'original'}-${edge.to}`"
                    :d="edgePath(edge)"
                    class="flow-edge"
                    marker-end="url(#flow-arrow)"
                  />
                </svg>

                <div
                  class="flow-original-card"
                  :style="{ left: `${layout.original.x}px`, top: `${layout.original.y}px` }"
                >
                  <img v-if="imageUrl" :src="imageUrl" alt="" />
                  <div class="flow-original-meta">
                    <strong>Imagem original</strong>
                    <span>{{ imageWidth > 0 ? `${imageWidth}×${imageHeight}` : "—" }}</span>
                  </div>
                  <button class="btn btn-mini" :disabled="busy" @click.stop="addChild(null)">
                    + Ramo
                  </button>
                </div>

                <FlowNodeCard
                  v-for="entry in layout.nodes"
                  :key="entry.node.id"
                  :style="{ left: `${entry.x}px`, top: `${entry.y}px` }"
                  :node="entry.node"
                  :nodes="nodes"
                  :models="models"
                  :busy="busy"
                  :image-width="imageWidth"
                  :image-height="imageHeight"
                  :result-time="resultTimes[entry.node.id]"
                  :result-url="resultUrls[entry.node.id]"
                  @update-node="update"
                  @add-child="addChild"
                  @remove-node="removeNode"
                  @toggle-visible="toggleVisible"
                  @toggle-save="toggleSave"
                />
              </div>
            </div>
          </div>

          <footer class="flow-modal-foot">
            <span>
              Cada nó é editável no próprio cartão. As setas ligam a entrada (imagem original ou saída
              do nó pai) à saída. O disquete marca o nó para
              <strong>Salvar todas → nós marcados</strong>; o olho mostra/oculta o painel; o × remove
              o nó e todos os seus descendentes.
            </span>
            <span v-if="hiddenCount > 0">
              <button class="btn btn-mini" :disabled="busy" @click="showAll">Mostrar todos</button>
            </span>
          </footer>
          </section>

          <div class="flow-resize n" @pointerdown="startResize($event, 'n')"></div>
          <div class="flow-resize s" @pointerdown="startResize($event, 's')"></div>
          <div class="flow-resize e" @pointerdown="startResize($event, 'e')"></div>
          <div class="flow-resize w" @pointerdown="startResize($event, 'w')"></div>
          <div class="flow-resize nw" @pointerdown="startResize($event, 'nw')"></div>
          <div class="flow-resize ne" @pointerdown="startResize($event, 'ne')"></div>
          <div class="flow-resize sw" @pointerdown="startResize($event, 'sw')"></div>
          <div
            class="flow-resize se"
            title="Redimensionar"
            @pointerdown="startResize($event, 'se')"
          >
            ◢
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>
