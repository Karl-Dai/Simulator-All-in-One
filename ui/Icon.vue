<script setup lang="ts">
withDefaults(defineProps<{ name: string; size?: number }>(), { size: 16 })
const paths: Record<string, string> = {
  play: 'm8 5 11 7-11 7Z',
  stop: 'M6 6h12v12H6Z',
  trash: 'M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7',
  refresh: 'M20 7a9 9 0 0 0-15-2L2 8m0-5v5h5M4 17a9 9 0 0 0 15 2l3-3m0 5v-5h-5',
  search: 'M21 21l-6-6M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0',
  plus: 'M12 5v14M5 12h14',
  lock: 'M5 10h14v11H5ZM8 10V7a4 4 0 0 1 8 0v3',
  folder: 'M3 5h6l2 3h10v12H3Z',
  chart: 'M3 3v18h18M7 14l4-5 4 3 5-7',
  node: 'M4 4h16v16H4ZM4 10h16M10 4v16',
  pointer: 'm5 3 15 9-7 2-3 7Z',
  signal: 'M12 12h.01M8 8a6 6 0 0 0 0 8M16 8a6 6 0 0 1 0 8M5 5a10 10 0 0 0 0 14M19 5a10 10 0 0 1 0 14',
  clock: 'M12 8v5l3 2M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
  bell: 'M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4',
  file: 'M4 3h10l6 6v12H4ZM14 3v6h6M8 13h8M8 17h6',
}
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none"
    stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true" focusable="false" class="simlab-icon">
    <path :d="paths[name] || paths.node" />
  </svg>
</template>

<style scoped>
.simlab-icon { display: inline-block; flex: none; vertical-align: middle; }
</style>
