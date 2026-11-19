<script setup lang="ts">
import { computed } from "vue";

import type { UpscaleProgress } from "../types";

const props = defineProps<{
  progress: UpscaleProgress | null;
  busy: boolean;
}>();

const label = computed(() => {
  const progress = props.progress;
  if (!progress) return "";

  switch (progress.stage) {
    case "loading":
      return "Carregando imagem…";
    case "model":
      return "Carregando modelo de IA…";
    case "upscaling":
      return progress.total > 1
        ? `Processando bloco ${progress.completed} de ${progress.total}…`
        : "Processando imagem…";
    case "saving":
      return "Salvando resultado…";
    case "retrying":
      return "Ajustando memória da GPU…";
    case "done":
      return "Concluído";
    default:
      return "Preparando…";
  }
});

const percent = computed(() => Math.round(props.progress?.percent ?? 0));

const elapsed = computed(() => {
  const ms = props.progress?.elapsedMs ?? 0;
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(1)} s`;
});
</script>

<template>
  <div v-if="busy" class="progress-wrap">
    <div class="progress-meta">
      <span class="progress-label">{{ label }}</span>
      <span class="progress-elapsed">{{ elapsed }}</span>
    </div>
    <div class="progress-track">
      <div class="progress-fill" :style="{ width: `${percent}%` }"></div>
    </div>
  </div>
</template>
