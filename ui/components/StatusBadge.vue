<script setup lang="ts">
withDefaults(defineProps<{
  label: string
  tone?: 'neutral' | 'success' | 'warning' | 'danger'
  busy?: boolean
}>(), { tone: 'neutral', busy: false })
</script>

<template>
  <span class="status-badge" :class="`status-badge--${tone}`" role="status" aria-live="polite" aria-atomic="true" :aria-busy="busy">
    <span class="status-badge__indicator" :class="{ 'status-badge__indicator--busy': busy }" aria-hidden="true" />
    <span>{{ label }}</span>
  </span>
</template>

<style scoped>
.status-badge {
  --status-color: var(--muted, #596579);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  color: var(--status-color);
  font: inherit;
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.status-badge--success { --status-color: var(--success, #26744b); }
.status-badge--warning { --status-color: var(--warning, #946000); }
.status-badge--danger { --status-color: var(--danger, #b93737); }
.status-badge__indicator {
  width: 7px;
  height: 7px;
  flex: none;
  border-radius: 50%;
  background: currentColor;
}
.status-badge__indicator--busy {
  width: 10px;
  height: 10px;
  box-sizing: border-box;
  background: transparent;
  border: 2px solid currentColor;
  border-right-color: transparent;
  animation: status-rotate 900ms linear infinite;
}
@keyframes status-rotate { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) {
  .status-badge__indicator--busy { animation: none; }
}
</style>
