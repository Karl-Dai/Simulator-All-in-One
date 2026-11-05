<script setup lang="ts">
withDefaults(defineProps<{
  message: string
  details?: string
  detailsLabel?: string
  closeLabel: string
  retryLabel?: string
  busy?: boolean
}>(), { details: '', detailsLabel: '', retryLabel: '', busy: false })

const emit = defineEmits<{
  close: []
  retry: []
}>()
</script>

<template>
  <section class="error-notice" :aria-busy="busy">
    <p class="error-notice__message" role="status" aria-live="polite" aria-atomic="true">{{ message }}</p>
    <details v-if="details && detailsLabel" class="error-notice__details">
      <summary>{{ detailsLabel }}</summary>
      <pre>{{ details }}</pre>
    </details>
    <div class="error-notice__actions">
      <button v-if="retryLabel" type="button" :disabled="busy" @click="emit('retry')">{{ retryLabel }}</button>
      <button type="button" @click="emit('close')">{{ closeLabel }}</button>
    </div>
  </section>
</template>

<style scoped>
.error-notice {
  padding: 12px;
  border: 1px solid var(--danger, #b93737);
  border-radius: var(--radius-md, 6px);
  background: var(--surface, #ffffff);
  color: var(--text, #202631);
  font: inherit;
  font-size: 13px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.error-notice__message { margin: 0; }
.error-notice__details { margin-top: 8px; }
.error-notice__details summary { cursor: pointer; }
.error-notice__details pre {
  margin: 8px 0 0;
  max-height: 160px;
  overflow: auto;
  white-space: pre-wrap;
  font-size: 12px;
}
.error-notice__actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
.error-notice__actions button {
  padding: 4px 10px;
  min-height: 30px;
  border: 1px solid var(--c-surface1, #b9c1cc);
  border-radius: var(--radius-md, 6px);
  background: var(--surface, #ffffff);
  color: var(--text, #202631);
  font: inherit;
  cursor: pointer;
}
.error-notice__actions button:hover:not(:disabled) { background: var(--hover, #eef1f5); }
.error-notice__actions button:disabled { cursor: wait; opacity: .6; }
.error-notice :is(button, summary):focus-visible { outline: 2px solid var(--focus, #245ccc); outline-offset: 2px; }
</style>
