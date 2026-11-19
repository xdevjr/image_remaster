<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch, type CSSProperties } from "vue";

import type { ViewerMode } from "../types";

const props = defineProps<{
  originalUrl: string;
  resultUrl: string;
  originalWidth: number;
  originalHeight: number;
  resultWidth: number;
  resultHeight: number;
  mode: ViewerMode;
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

const panels = computed(() => [
  { url: props.originalUrl, label: "Original" },
  { url: props.resultUrl, label: "Remaster" },
]);

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function panelSize(index: number) {
  if (props.mode === "side") {
    return { w: Math.max(1, (size.value.w - GAP) / 2), h: Math.max(1, size.value.h) };
  }
  return { w: Math.max(1, size.value.w), h: Math.max(1, size.value.h) };
}

function imageSize(index: number) {
  if (props.mode === "slider") {
    return { w: Math.max(1, props.resultWidth), h: Math.max(1, props.resultHeight) };
  }
  return index === 0
    ? { w: Math.max(1, props.originalWidth), h: Math.max(1, props.originalHeight) }
    : { w: Math.max(1, props.resultWidth), h: Math.max(1, props.resultHeight) };
}

function fitScale(index: number, zoomValue = zoom.value) {
  const panel = panelSize(index);
  const image = imageSize(index);
  const fit = Math.min(panel.w / image.w, panel.h / image.h);
  return { fit: fit || 1, panel, image, scale: (fit || 1) * zoomValue };
}

function geometry(index: number, zoomValue = zoom.value) {
  const { panel, scale } = fitScale(index, zoomValue);
  const image = imageSize(index);
  const dw = image.w * scale;
  const dh = image.h * scale;
  return {
    panel,
    dw,
    dh,
    left: panel.w / 2 - centerX.value * dw,
    top: panel.h / 2 - centerY.value * dh,
  };
}

function imageStyle(index: number): CSSProperties {
  const g = geometry(index);
  const image = imageSize(index);
  const effective = g.dw / image.w;
  return {
    left: `${g.left}px`,
    top: `${g.top}px`,
    width: `${g.dw}px`,
    height: `${g.dh}px`,
    imageRendering: effective >= 4 ? "pixelated" : "auto",
  };
}

const zoomPercent = computed(() => {
  const g = geometry(1);
  return Math.round((g.dw / props.resultWidth) * 100);
});

function fit() {
  zoom.value = 1;
  centerX.value = 0.5;
  centerY.value = 0.5;
}

function oneToOne() {
  const { fit: base } = fitScale(1, 1);
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
  const normalizedX = (pointerX - g.left) / g.dw;
  const normalizedY = (pointerY - g.top) / g.dh;

  const factor = Math.exp(-event.deltaY * 0.0015);
  const nextZoom = clamp(zoom.value * factor, MIN_ZOOM, MAX_ZOOM);

  const { panel, scale } = fitScale(index, nextZoom);
  const nextDw = image.w * scale;
  const nextDh = image.h * scale;

  centerX.value = clamp(normalizedX - (pointerX - panel.w / 2) / nextDw, 0, 1);
  centerY.value = clamp(normalizedY - (pointerY - panel.h / 2) / nextDh, 0, 1);
  zoom.value = nextZoom;
}

let panning: { index: number; lastX: number; lastY: number } | null = null;
let sliderDragging = false;

function onPointerDown(event: PointerEvent, index: number) {
  if (event.button !== 0) return;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  panning = { index, lastX: event.clientX, lastY: event.clientY };
}

function startSliderDrag(event: PointerEvent) {
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
  () => [props.resultUrl, props.originalUrl],
  () => fit(),
);
</script>

<template>
  <div ref="container" class="viewer">
    <div v-if="mode === 'side'" class="panels">
      <div
        v-for="(panel, index) in panels"
        :key="index"
        class="panel"
        @wheel="onWheel($event, index)"
        @pointerdown="onPointerDown($event, index)"
        @dblclick="fit"
      >
        <img :src="panel.url" :style="imageStyle(index)" draggable="false" alt="" />
        <span class="panel-badge">{{ panel.label }}</span>
      </div>
    </div>

    <div
      v-else
      class="panel single"
      @wheel="onWheel($event, 1)"
      @pointerdown="onPointerDown($event, 1)"
      @dblclick="fit"
    >
      <div class="layer">
        <img :src="resultUrl" :style="imageStyle(1)" draggable="false" alt="" />
      </div>
      <div
        class="layer"
        :style="{ clipPath: `inset(0 ${(1 - sliderPos) * 100}% 0 0)` }"
      >
        <img :src="originalUrl" :style="imageStyle(1)" draggable="false" alt="" />
      </div>
      <div
        class="divider"
        :style="{ left: `${sliderPos * 100}%` }"
        @pointerdown.stop="startSliderDrag"
      >
        <div class="divider-line"></div>
        <div class="divider-handle">⇔</div>
      </div>
      <span class="panel-badge left">Original</span>
      <span class="panel-badge right">Remaster</span>
    </div>

    <div class="viewer-controls">
      <button class="mini-btn" @click="fit">Ajustar</button>
      <button class="mini-btn" @click="oneToOne">1:1</button>
      <span class="zoom-label">{{ zoomPercent }}%</span>
    </div>

    <div class="viewer-hint">Role para zoom · arraste para mover · duplo clique ajusta</div>
  </div>
</template>
