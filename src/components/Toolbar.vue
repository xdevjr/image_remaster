<script setup lang="ts">
import type { ModelInfo, UpscaleMode, ViewerMode } from "../types";

const props = defineProps<{
  hasImage: boolean;
  hasResult: boolean;
  busy: boolean;
  mode: UpscaleMode;
  scale: number;
  viewerMode: ViewerMode;
  models: ModelInfo[];
  model: string;
}>();

const emit = defineEmits<{
  (event: "open"): void;
  (event: "run"): void;
  (event: "save"): void;
  (event: "cancel"): void;
  (event: "update:mode", value: UpscaleMode): void;
  (event: "update:scale", value: number): void;
  (event: "update:viewerMode", value: ViewerMode): void;
  (event: "update:model", value: string): void;
}>();
</script>

<template>
  <header class="toolbar">
    <div class="brand">
      <div class="brand-mark">
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path
            d="M12 3l6 10h-4v8H10v-8H6z"
            fill="currentColor"
          />
        </svg>
      </div>
      <div class="brand-text">
        <strong>Image Remaster</strong>
        <span>upscale com IA</span>
      </div>
    </div>

    <div class="toolbar-group">
      <button class="btn" :disabled="busy" @click="emit('open')">
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <path
            d="M4 5h6l2 2h8v12H4z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.7"
            stroke-linejoin="round"
          />
        </svg>
        Abrir
      </button>
    </div>

    <div class="toolbar-separator"></div>

    <div class="toolbar-group">
      <span class="group-label">Motor</span>
      <div class="segmented">
        <button
          :class="['segmented-item', { active: mode === 'ai' }]"
          :disabled="busy"
          @click="emit('update:mode', 'ai')"
        >
          IA
        </button>
        <button
          :class="['segmented-item', { active: mode === 'classic' }]"
          :disabled="busy"
          @click="emit('update:mode', 'classic')"
        >
          Rápido
        </button>
      </div>
    </div>

    <div class="toolbar-group">
      <span class="group-label">Escala</span>
      <div class="segmented">
        <button
          :class="['segmented-item', { active: scale === 1 }]"
          :disabled="busy"
          title="Remasteriza sem aumentar a resolução"
          @click="emit('update:scale', 1)"
        >
          1x
        </button>
        <button
          :class="['segmented-item', { active: scale === 2 }]"
          :disabled="busy"
          @click="emit('update:scale', 2)"
        >
          2x
        </button>
        <button
          :class="['segmented-item', { active: scale === 4 }]"
          :disabled="busy"
          @click="emit('update:scale', 4)"
        >
          4x
        </button>
      </div>
    </div>

    <div v-if="mode === 'ai'" class="toolbar-group">
      <span class="group-label">Modelo</span>
      <div class="segmented">
        <button
          v-for="option in models"
          :key="option.id"
          :class="['segmented-item', { active: model === option.id }]"
          :disabled="busy || !option.available"
          :title="option.description"
          @click="emit('update:model', option.id)"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div v-if="hasImage && hasResult" class="toolbar-group">
      <span class="group-label">Visão</span>
      <div class="segmented">
        <button
          :class="['segmented-item', { active: viewerMode === 'side' }]"
          @click="emit('update:viewerMode', 'side')"
        >
          Lado a lado
        </button>
        <button
          :class="['segmented-item', { active: viewerMode === 'slider' }]"
          @click="emit('update:viewerMode', 'slider')"
        >
          Slider
        </button>
      </div>
    </div>

    <div class="toolbar-spacer"></div>

    <button
      v-if="busy"
      class="btn btn-danger"
      @click="emit('cancel')"
    >
      <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
        <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
      </svg>
      Cancelar
    </button>

    <button
      class="btn btn-primary"
      :disabled="!hasImage || busy"
      @click="emit('run')"
    >
      <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
        <path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z" fill="currentColor" />
      </svg>
      Remasterizar
    </button>

    <button
      v-if="hasResult"
      class="btn"
      :disabled="busy"
      @click="emit('save')"
    >
      <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
        <path
          d="M5 4h11l3 3v13H5z"
          fill="none"
          stroke="currentColor"
          stroke-width="1.7"
          stroke-linejoin="round"
        />
        <path d="M8 4v5h8V4M8 20v-6h8v6" fill="none" stroke="currentColor" stroke-width="1.5" />
      </svg>
      Salvar
    </button>
  </header>
</template>
