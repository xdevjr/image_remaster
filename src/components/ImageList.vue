<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";

import type { JobItem, ModelInfo } from "../types";
import { nodeSummary } from "../lib/flow";

const props = defineProps<{
  items: JobItem[];
  selectedId: string | null;
  busy: boolean;
  compareMode: boolean;
  compareIds: string[];
  models: ModelInfo[];
}>();

const emit = defineEmits<{
  (event: "select", id: string): void;
  (event: "remove", id: string): void;
  (event: "add"): void;
  (event: "clear"): void;
  (event: "run-item", id: string): void;
  (event: "save-item", id: string): void;
  (event: "toggle-compare", id: string): void;
  (event: "select-all-compare"): void;
  (event: "clear-compare"): void;
  (
    event: "item-menu",
    payload: { id: string; label: string; x: number; y: number },
  ): void;
}>();

function thumb(item: JobItem) {
  return convertFileSrc(item.info.path);
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function statusLabel(item: JobItem) {
  switch (item.status) {
    case "processing":
      return "processando";
    case "done":
      return "pronta";
    case "error":
      return "erro";
    case "cancelled":
      return "cancelada";
    default:
      return "pendente";
  }
}

function modelLabel(id: string) {
  return props.models.find((model) => model.id === id)?.label ?? id;
}

function flowLabel(item: JobItem) {
  return item.nodes
    .map((node) => `${node.name} · ${nodeSummary(node, modelLabel)}`)
    .join("  ");
}

function openItemMenu(event: MouseEvent, item: JobItem) {
  event.preventDefault();
  event.stopPropagation();
  emit("item-menu", {
    id: item.id,
    label: fileName(item.info.path),
    x: event.clientX,
    y: event.clientY,
  });
}
</script>

<template>
  <aside class="image-list">
    <header class="list-header">
      <span>{{ compareMode ? `Comparar (${compareIds.length})` : `Imagens (${items.length})` }}</span>
      <div class="list-header-actions">
        <template v-if="compareMode">
          <button
            class="icon-btn"
            title="Marcar todas"
            :disabled="busy"
            @click="emit('select-all-compare')"
          >
            ✓
          </button>
          <button
            class="icon-btn"
            title="Desmarcar todas"
            :disabled="busy"
            @click="emit('clear-compare')"
          >
            −
          </button>
        </template>
        <button class="icon-btn" title="Adicionar imagens" :disabled="busy" @click="emit('add')">
          +
        </button>
        <button class="icon-btn" title="Limpar lista" :disabled="busy" @click="emit('clear')">
          ×
        </button>
      </div>
    </header>

    <div class="list-scroll">
      <button
        v-for="item in items"
        :key="item.id"
        :class="[
          'list-item',
          { active: item.id === selectedId, compared: compareMode && compareIds.includes(item.id) },
        ]"
        :title="item.error ?? item.info.path"
        @click="emit('select', item.id)"
        @contextmenu="openItemMenu($event, item)"
      >
        <span
          v-if="compareMode"
          :class="['list-check', { checked: compareIds.includes(item.id) }]"
          title="Incluir na comparação"
          @click.stop="emit('toggle-compare', item.id)"
        >
          {{ compareIds.includes(item.id) ? "✓" : "" }}
        </span>
        <img class="list-thumb" :src="thumb(item)" :alt="fileName(item.info.path)" loading="lazy" />
        <span class="list-meta">
          <span class="list-name">{{ fileName(item.info.path) }}</span>
          <span class="list-sub">
            {{ item.info.width }}×{{ item.info.height }}
            <template v-if="item.result"> → {{ item.result.width }}×{{ item.result.height }}</template>
          </span>
          <span class="list-flow" :title="flowLabel(item)">{{ flowLabel(item) }}</span>
          <span :class="['list-status', item.status]">{{ statusLabel(item) }}</span>
        </span>
        <span class="list-actions">
          <span
            :class="['list-action', 'run', { disabled: busy || item.status === 'processing' }]"
            title="Processar somente esta imagem"
            @click.stop="emit('run-item', item.id)"
          >
            ▶
          </span>
          <span
            :class="['list-action', 'save', { disabled: busy || !item.result }]"
            title="Salvar o resultado desta imagem"
            @click.stop="item.result && emit('save-item', item.id)"
          >
            ↓
          </span>
          <span class="list-action remove" title="Remover" @click.stop="emit('remove', item.id)">
            ×
          </span>
        </span>
      </button>
    </div>
  </aside>
</template>
