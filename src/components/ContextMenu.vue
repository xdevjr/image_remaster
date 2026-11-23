<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";

import type { ContextMenuItem } from "../types";

const props = defineProps<{
  x: number;
  y: number;
  title?: string;
  items: ContextMenuItem[];
}>();

const emit = defineEmits<{
  (event: "select", id: string): void;
  (event: "close"): void;
}>();

const style = computed(() => {
  const width = 240;
  const height = 44 + props.items.length * 34 + 8;
  const x = Math.max(8, Math.min(props.x, window.innerWidth - width - 8));
  const y = Math.max(8, Math.min(props.y, window.innerHeight - height - 8));
  return { left: `${x}px`, top: `${y}px`, width: `${width}px` };
});

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div
      class="context-backdrop"
      @click="emit('close')"
      @contextmenu.prevent="emit('close')"
    ></div>
    <div class="context-menu" :style="style" @contextmenu.prevent>
      <div v-if="title" class="context-title">{{ title }}</div>
      <button
        v-for="item in items"
        :key="item.id"
        :class="['context-item', { danger: item.danger }]"
        :disabled="item.disabled"
        @click="emit('select', item.id)"
      >
        {{ item.label }}
      </button>
    </div>
  </Teleport>
</template>
