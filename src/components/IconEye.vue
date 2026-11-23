<script setup lang="ts">
withDefaults(
  defineProps<{
    off?: boolean;
    size?: number;
  }>(),
  { off: false, size: 14 },
);
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    aria-hidden="true"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <template v-if="off">
      <path d="M3.4 4.2 20.6 19.8" />
      <path d="M2.6 12s3.6-6.2 9.4-6.2c1.9 0 3.5.6 4.9 1.4" />
      <path d="M21.4 12s-3.6 6.2-9.4 6.2c-1.9 0-3.5-.6-4.9-1.4" />
    </template>
    <template v-else>
      <path d="M2.6 12s3.6-6.2 9.4-6.2S21.4 12 21.4 12s-3.6 6.2-9.4 6.2S2.6 12 2.6 12Z" />
      <circle cx="12" cy="12" r="2.5" fill="currentColor" stroke="none" />
    </template>
  </svg>
</template>
