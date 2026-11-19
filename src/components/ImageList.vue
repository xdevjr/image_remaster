<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";

import type { BatchProgress, JobItem } from "../types";

const props = defineProps<{
  items: JobItem[];
  selectedId: string | null;
  busy: boolean;
  batch: BatchProgress | null;
}>();

const emit = defineEmits<{
  (event: "select", id: string): void;
  (event: "remove", id: string): void;
  (event: "add"): void;
  (event: "clear"): void;
  (event: "runAll"): void;
  (event: "saveAll"): void;
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

const hasResults = () => props.items.some((item) => item.result);
</script>

<template>
  <aside class="image-list">
    <header class="list-header">
      <span>Imagens ({{ items.length }})</span>
      <div class="list-header-actions">
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
        :class="['list-item', { active: item.id === selectedId }]"
        :title="item.error ?? item.info.path"
        @click="emit('select', item.id)"
      >
        <img class="list-thumb" :src="thumb(item)" :alt="fileName(item.info.path)" loading="lazy" />
        <span class="list-meta">
          <span class="list-name">{{ fileName(item.info.path) }}</span>
          <span class="list-sub">
            {{ item.info.width }}×{{ item.info.height }}
            <template v-if="item.result"> → {{ item.result.width }}×{{ item.result.height }}</template>
          </span>
          <span :class="['list-status', item.status]">{{ statusLabel(item) }}</span>
        </span>
        <span class="list-remove" title="Remover" @click.stop="emit('remove', item.id)">×</span>
      </button>
    </div>

    <footer class="list-footer">
      <div v-if="batch" class="list-batch">Remasterizando {{ batch.current }}/{{ batch.total }}…</div>
      <button
        class="btn btn-primary list-btn"
        :disabled="busy || items.length === 0"
        @click="emit('runAll')"
      >
        Remasterizar todas
      </button>
      <button class="btn list-btn" :disabled="busy || !hasResults()" @click="emit('saveAll')">
        Salvar todas
      </button>
    </footer>
  </aside>
</template>
