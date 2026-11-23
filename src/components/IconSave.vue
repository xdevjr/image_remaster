<script setup lang="ts">
withDefaults(
  defineProps<{
    active?: boolean;
    size?: number;
  }>(),
  { active: false, size: 14 },
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
    <path d="M5 4h11l3 3v13H5z" />
    <path d="M8 4v5h8V4M8 20v-6h8v6" />
  </svg>
</template>
