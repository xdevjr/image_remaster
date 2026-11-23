<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch, type CSSProperties } from "vue";

import IconEye from "./IconEye.vue";
import type { ComparisonLayout, ViewerPanel } from "../types";

const props = defineProps<{
  panels: ViewerPanel[];
  mode: "side" | "slider";
  layout: ComparisonLayout;
}>();

const emit = defineEmits<{
  (event: "hide", id: string): void;
  (
    event: "panel-menu",
    payload: { id: string; label: string; x: number; y: number },
  ): void;
}>();

const GAP = 2;
const MIN_ZOOM = 1;
const MAX_ZOOM = 32;

const container = ref<HTMLDivElement | null>(null);
const size = ref({ w: 0, h: 0 });
const zoom = ref(1);
const centerX = ref(0.5);
const centerY = ref(0.5);
const sliderPos = ref(0.5);

const layout = computed(() => {
  const count = props.mode === "slider" ? 1 : Math.max(1, props.panels.length);
  let cols = 1;
  if (props.layout === "horizontal") {
    cols = count;
  } else if (props.layout === "vertical") {
    cols = 1;
  } else if (props.layout === "grid") {
    cols = Math.min(2, count);
  } else if (count <= 3) {
    cols = count;
  } else if (count === 4) {
    cols = 2;
  } else {
    cols = 3;
  }
  return { cols, rows: Math.ceil(count / cols), count };
});

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function panelRect(index: number) {
  if (props.mode === "slider") {
    return { w: Math.max(1, size.value.w), h: Math.max(1, size.value.h), left: 0, top: 0 };
  }
  const { cols, rows, count } = layout.value;
  const w = Math.max(1, (size.value.w - GAP * (cols - 1)) / cols);
  const h = Math.max(1, (size.value.h - GAP * (rows - 1)) / rows);
  const row = Math.floor(index / cols);
  const inRow = Math.min(cols, count - row * cols);
  const offset = ((cols - inRow) * (w + GAP)) / 2;
  return {
    w,
    h,
    left: offset + (index % cols) * (w + GAP),
    top: row * (h + GAP),
  };
}

function basePanel() {
  return props.panels[props.panels.length - 1];
}

function imageSize(index: number) {
  if (props.mode === "slider") {
    const base = basePanel();
    return { w: Math.max(1, base?.width ?? 1), h: Math.max(1, base?.height ?? 1) };
  }
  const panel = props.panels[index];
  return { w: Math.max(1, panel?.width ?? 1), h: Math.max(1, panel?.height ?? 1) };
}

function fitScale(index: number, zoomValue = zoom.value) {
  const rect = panelRect(index);
  const image = imageSize(index);
  const fit = Math.min(rect.w / image.w, rect.h / image.h);
  return { rect, image, fit: fit || 1, scale: (fit || 1) * zoomValue };
}

function geometry(index: number, zoomValue = zoom.value) {
  const { rect, image, scale } = fitScale(index, zoomValue);
  const dw = image.w * scale;
  const dh = image.h * scale;
  return {
    rect,
    dw,
    dh,
    left: rect.left + rect.w / 2 - centerX.value * dw,
    top: rect.top + rect.h / 2 - centerY.value * dh,
  };
}

function panelStyle(index: number): CSSProperties {
  const rect = panelRect(index);
  return {
    left: `${rect.left}px`,
    top: `${rect.top}px`,
    width: `${rect.w}px`,
    height: `${rect.h}px`,
  };
}

function imageStyle(index: number): CSSProperties {
  const g = geometry(index);
  const image = imageSize(index);
  const effective = g.dw / image.w;
  return {
    left: `${g.left - g.rect.left}px`,
    top: `${g.top - g.rect.top}px`,
    width: `${g.dw}px`,
    height: `${g.dh}px`,
    imageRendering: effective >= 4 ? "pixelated" : "auto",
  };
}

const zoomPercent = computed(() => {
  const g = geometry(0);
  return Math.round((g.dw / imageSize(0).w) * 100);
});

function clearSelection() {
  window.getSelection()?.removeAllRanges();
}

function fit() {
  clearSelection();
  zoom.value = 1;
  centerX.value = 0.5;
  centerY.value = 0.5;
}

function oneToOne() {
  const { fit: base } = fitScale(0, 1);
  zoom.value = clamp(1 / base, MIN_ZOOM, MAX_ZOOM);
  centerX.value = 0.5;
  centerY.value = 0.5;
}

function onWheel(event: WheelEvent, index: number) {
  event.preventDefault();
  const element = event.currentTarget as HTMLElement;
  const rect = element.getBoundingClientRect();
  const pointerX = event.clientX - rect.left;
  const pointerY = event.clientY - rect.top;

  const g = geometry(index);
  const image = imageSize(index);
  const normalizedX = (pointerX + g.rect.left - g.left) / g.dw;
  const normalizedY = (pointerY + g.rect.top - g.top) / g.dh;

  const factor = Math.exp(-event.deltaY * 0.0015);
  const nextZoom = clamp(zoom.value * factor, MIN_ZOOM, MAX_ZOOM);

  const { rect: nextRect, scale } = fitScale(index, nextZoom);
  const nextDw = image.w * scale;
  const nextDh = image.h * scale;

  centerX.value = clamp(normalizedX - (pointerX - nextRect.w / 2) / nextDw, 0, 1);
  centerY.value = clamp(normalizedY - (pointerY - nextRect.h / 2) / nextDh, 0, 1);
  zoom.value = nextZoom;
}

let panning: { index: number; lastX: number; lastY: number } | null = null;
let sliderDragging = false;

function onPointerDown(event: PointerEvent, index: number) {
  if (event.button !== 0) return;
  event.preventDefault();
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  panning = { index, lastX: event.clientX, lastY: event.clientY };
}

function startSliderDrag(event: PointerEvent) {
  event.preventDefault();
  sliderDragging = true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  updateSlider(event);
}

function updateSlider(event: PointerEvent) {
  const element = container.value;
  if (!element) return;
  const rect = element.getBoundingClientRect();
  sliderPos.value = clamp((event.clientX - rect.left) / rect.width, 0, 1);
}

function onPointerMove(event: PointerEvent) {
  if (panning) {
    const g = geometry(panning.index);
    const dx = event.clientX - panning.lastX;
    const dy = event.clientY - panning.lastY;
    centerX.value = clamp(centerX.value - dx / g.dw, 0, 1);
    centerY.value = clamp(centerY.value - dy / g.dh, 0, 1);
    panning.lastX = event.clientX;
    panning.lastY = event.clientY;
    return;
  }
  if (sliderDragging) {
    updateSlider(event);
  }
}

function endDrag() {
  panning = null;
  sliderDragging = false;
}

function onPanelContext(event: MouseEvent, panel: ViewerPanel, index: number) {
  event.preventDefault();
  event.stopPropagation();
  emit("panel-menu", {
    id: panel.id ?? String(index),
    label: panel.label,
    x: event.clientX,
    y: event.clientY,
  });
}

function onSliderContext(event: MouseEvent) {
  event.preventDefault();
  const panel = props.panels[1] ?? props.panels[0];
  emit("panel-menu", {
    id: panel?.id ?? "slider",
    label: panel?.label ?? "Painel",
    x: event.clientX,
    y: event.clientY,
  });
}

let observer: ResizeObserver | null = null;

onMounted(() => {
  observer = new ResizeObserver((entries) => {
    const entry = entries[0];
    if (!entry) return;
    size.value = { w: entry.contentRect.width, h: entry.contentRect.height };
  });
  if (container.value) observer.observe(container.value);

  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", endDrag);
  window.addEventListener("pointercancel", endDrag);
});

onUnmounted(() => {
  observer?.disconnect();
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", endDrag);
  window.removeEventListener("pointercancel", endDrag);
});

watch(
  () => props.panels.map((panel) => panel.url).join("|"),
  () => fit(),
);
</script>

<template>
  <div ref="container" class="viewer">
    <div v-if="mode === 'side'" class="panels">
      <div
        v-for="(panel, index) in panels"
        :key="`${index}-${panel.url}`"
        class="panel"
        :style="panelStyle(index)"
        @wheel="onWheel($event, index)"
        @pointerdown="onPointerDown($event, index)"
        @dblclick.prevent="fit"
        @contextmenu="onPanelContext($event, panel, index)"
      >
        <img :src="panel.url" :style="imageStyle(index)" draggable="false" alt="" />
        <span class="panel-badge">{{ panel.label }}</span>
        <span v-if="panel.sub" class="panel-sub">{{ panel.sub }}</span>
        <button
          v-if="panel.hideable !== false"
          class="panel-hide"
          title="Ocultar este painel"
          @pointerdown.stop
          @click.stop="emit('hide', panel.id ?? String(index))"
        >
          <IconEye off :size="14" />
        </button>
      </div>
    </div>

    <div
      v-else
      class="panel single"
      @wheel="onWheel($event, 1)"
      @pointerdown="onPointerDown($event, 1)"
      @dblclick="fit"
      @contextmenu="onSliderContext"
    >
      <div class="layer">
        <img :src="panels[1]?.url" :style="imageStyle(1)" draggable="false" alt="" />
      </div>
      <div
        class="layer"
        :style="{ clipPath: `inset(0 ${(1 - sliderPos) * 100}% 0 0)` }"
      >
        <img :src="panels[0]?.url" :style="imageStyle(1)" draggable="false" alt="" />
      </div>
      <div
        class="divider"
        :style="{ left: `${sliderPos * 100}%` }"
        @pointerdown.stop="startSliderDrag"
      >
        <div class="divider-line"></div>
        <div class="divider-handle">⇔</div>
      </div>
      <span class="panel-badge left">{{ panels[0]?.label ?? "Original" }}</span>
      <span class="panel-badge right">{{ panels[1]?.label ?? "Resultado" }}</span>
    </div>

    <div class="viewer-controls">
      <button class="mini-btn" @click="fit">Ajustar</button>
      <button class="mini-btn" @click="oneToOne">1:1</button>
      <span class="zoom-label">{{ zoomPercent }}%</span>
    </div>

    <div class="viewer-hint">
      {{ panels.length > 2 ? `${panels.length} painéis · ` : "" }}Role para zoom · arraste para mover
      · duplo clique ajusta
    </div>
  </div>
</template>
