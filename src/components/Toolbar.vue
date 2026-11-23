<script setup lang="ts">
import FlowEditor from "./FlowEditor.vue";
import IconEye from "./IconEye.vue";
import type {
  ComparisonLayout,
  FlowNode,
  FlowPreset,
  ModelInfo,
  ViewerMode,
} from "../types";

const props = defineProps<{
  hasResult: boolean;
  busy: boolean;
  viewerMode: ViewerMode;
  models: ModelInfo[];
  nodes: FlowNode[];
  resultTimes: Record<string, number>;
  resultUrls: Record<string, string>;
  presets: FlowPreset[];
  compareMode: boolean;
  canCompare: boolean;
  canSlider: boolean;
  canLayout: boolean;
  layoutMode: ComparisonLayout;
  imageName: string;
  imageUrl: string;
  imageWidth: number;
  imageHeight: number;
  sidebarOpen: boolean;
  canToggleSidebar: boolean;
  canRunAll: boolean;
  hasAnyResult: boolean;
  originalVisible: boolean;
}>();

const emit = defineEmits<{
  (event: "open"): void;
  (event: "run-all"): void;
  (event: "save-all-menu", mouseEvent: MouseEvent): void;
  (event: "cancel"): void;
  (event: "update:viewer-mode", value: ViewerMode): void;
  (event: "update:layout-mode", value: ComparisonLayout): void;
  (event: "update:nodes", value: FlowNode[]): void;
  (event: "save-preset", name: string): void;
  (event: "apply-preset", id: string): void;
  (event: "delete-preset", id: string): void;
  (event: "toggle-compare"): void;
  (event: "toggle-sidebar"): void;
  (event: "toggle-original"): void;
}>();
</script>

<template>
  <header class="toolbar">
    <div class="brand">
      <div class="brand-mark">
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path d="M12 3l6 10h-4v8H10v-8H6z" fill="currentColor" />
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
      <button
        :class="['btn', { 'btn-active': sidebarOpen }]"
        :disabled="!canToggleSidebar || busy"
        :title="sidebarOpen ? 'Ocultar lista de imagens' : 'Mostrar lista de imagens'"
        @click="emit('toggle-sidebar')"
      >
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
          <path
            d="M4 6h16M4 12h16M4 18h16"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
          />
        </svg>
        Lista
      </button>
    </div>

    <div class="toolbar-separator"></div>

    <FlowEditor
      v-if="!compareMode"
      :nodes="nodes"
      :models="models"
      :busy="busy"
      :image-name="imageName"
      :image-url="imageUrl"
      :image-width="imageWidth"
      :image-height="imageHeight"
      :result-times="resultTimes"
      :result-urls="resultUrls"
      :presets="presets"
      @update:nodes="emit('update:nodes', $event)"
      @save-preset="emit('save-preset', $event)"
      @apply-preset="emit('apply-preset', $event)"
      @delete-preset="emit('delete-preset', $event)"
    />

    <div v-if="canCompare" class="toolbar-group">
      <button
        :class="['btn', { 'btn-active': compareMode }]"
        :disabled="busy"
        :title="compareMode ? 'Voltar para a remasterização' : 'Comparar imagens sem remasterizar'"
        @click="emit('toggle-compare')"
      >
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
          <path
            d="M3 5h7v14H3zM14 5h7v14h-7z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linejoin="round"
          />
        </svg>
        Comparar
      </button>
    </div>

    <div v-if="hasResult && !compareMode" class="toolbar-group">
      <span class="group-label">Visão</span>
      <div class="segmented">
        <button
          :class="['segmented-item', { active: viewerMode === 'side' || !canSlider }]"
          title="Lado a lado"
          @click="emit('update:viewer-mode', 'side')"
        >
          Lado
        </button>
        <button
          :class="['segmented-item', { active: viewerMode === 'slider' && canSlider }]"
          :disabled="!canSlider"
          :title="canSlider ? undefined : 'Slider disponível com exatamente dois painéis'"
          @click="emit('update:viewer-mode', 'slider')"
        >
          Slider
        </button>
      </div>
      <button
        :class="['btn', 'btn-mini', { 'btn-active': originalVisible }]"
        :title="originalVisible ? 'Ocultar o original' : 'Mostrar o original'"
        aria-label="Mostrar ou ocultar o original"
        @click="emit('toggle-original')"
      >
        <IconEye :off="!originalVisible" :size="14" />
      </button>
    </div>

    <div v-if="canLayout && viewerMode !== 'slider'" class="toolbar-group">
      <span class="group-label">Layout</span>
      <div class="segmented">
        <button
          :class="['segmented-item', { active: layoutMode === 'auto' }]"
          title="Automático: quebra conforme a quantidade de painéis"
          @click="emit('update:layout-mode', 'auto')"
        >
          Auto
        </button>
        <button
          :class="['segmented-item', 'layout-icon', { active: layoutMode === 'horizontal' }]"
          title="Lado a lado na horizontal (uma linha)"
          @click="emit('update:layout-mode', 'horizontal')"
        >
          ↔
        </button>
        <button
          :class="['segmented-item', 'layout-icon', { active: layoutMode === 'vertical' }]"
          title="Lado a lado na vertical (uma coluna)"
          @click="emit('update:layout-mode', 'vertical')"
        >
          ↕
        </button>
        <button
          :class="['segmented-item', 'layout-icon', { active: layoutMode === 'grid' }]"
          title="Grade de 2 colunas (ex.: 2 em cima, 2 embaixo)"
          @click="emit('update:layout-mode', 'grid')"
        >
          ⊞
        </button>
      </div>
    </div>

    <div class="toolbar-actions">
      <button v-if="busy" class="btn btn-danger" @click="emit('cancel')">
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
          <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
        </svg>
        Cancelar
      </button>

      <button
        class="btn btn-primary"
        :disabled="!canRunAll || busy"
        title="Remasterizar todas as imagens da lista"
        @click="emit('run-all')"
      >
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
          <path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z" fill="currentColor" />
        </svg>
        Remasterizar tudo
      </button>

      <button
        class="btn"
        :disabled="busy || !hasAnyResult"
        title="Escolher o que salvar de todas as imagens"
        @click="emit('save-all-menu', $event)"
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
        Salvar todas
        <span class="btn-caret">▾</span>
      </button>
    </div>
  </header>
</template>
