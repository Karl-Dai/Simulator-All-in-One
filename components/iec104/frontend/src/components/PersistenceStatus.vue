<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@app/simlab-ui/configInvoke'
const status = ref<{enabled: boolean; error: string | null}>({enabled:false,error:null})
let timer: ReturnType<typeof setInterval> | undefined
async function refresh() {
  try { status.value = await invoke('get_persistence_status') }
  catch (error) { status.value = {enabled:false,error:String(error)} }
}
onMounted(() => { void refresh(); timer = setInterval(() => { void refresh() }, 2000) })
onUnmounted(() => clearInterval(timer))
</script>
<template>
  <div class="suite-persistence-status" :class="{failed: status.error}" :role="status.error ? 'alert' : 'status'" aria-live="polite">
    {{ status.error ? `配置未保存：${status.error}` : status.enabled ? '配置自动保存' : '配置存储尚未就绪' }}
  </div>
</template>
<style scoped>
.suite-persistence-status {position:fixed;bottom:8px;left:8px;z-index:10000;max-width:min(520px,75vw);padding:6px 10px;border:1px solid var(--border,#666);border-radius:6px;background:var(--surface,#202124);color:var(--text,#eee);font:12px system-ui;overflow-wrap:anywhere}
.failed {color:var(--danger,#ffb4a9);border-color:currentColor}
</style>
