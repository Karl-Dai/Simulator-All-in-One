<script setup lang="ts">
import { reactive, watch, onUnmounted } from "vue";
import { invoke } from "simlab-ui/configInvoke";
import { running } from "../composables/useSubEvents";
import { t } from "../i18n";
import { useToast, toastError } from "../composables/useToast";
const { push } = useToast();

import { documentValue, scheduleDocument, flushDocuments } from "../suiteWorkspace";
const suiteBuild = import.meta.env.VITE_SIMLAB_SUITE === "1";
const gen = reactive({ freq_offset_hz: 0.0, rocof_hz_s: 0.0 });

if (suiteBuild) Object.assign(gen, documentValue<typeof gen>("settings"));

let timer: number | undefined;
watch(gen, () => {
  if (suiteBuild) scheduleDocument("settings", gen);
  clearTimeout(timer);
  timer = window.setTimeout(async () => {
    if (!running.value) return;
    try { if (suiteBuild) await flushDocuments(); await invoke("update_gen", { freqOffsetHz: gen.freq_offset_hz, rocofHzS: gen.rocof_hz_s }); }
    catch (cause) { push(toastError(cause), "error"); }
  }, 120);
});

watch(running, async value => {
  if (!suiteBuild || !value) return;
  try { await flushDocuments(); await invoke("update_gen", { freqOffsetHz: gen.freq_offset_hz, rocofHzS: gen.rocof_hz_s }); }
  catch (cause) { push(toastError(cause), "error"); }
});

function imported(event: Event) {
  if ((event as CustomEvent).detail === 'settings') Object.assign(gen, documentValue<typeof gen>("settings"));
}
window.addEventListener('saio-document-imported', imported);
onUnmounted(() => { clearTimeout(timer); window.removeEventListener('saio-document-imported', imported); });
async function trigger() {
  if (!running.value) return;
  try { await invoke("fire_trigger"); }
  catch (cause) { push(toastError(cause), "error"); }
}
</script>

<template>
  <section class="panel">
    <h3>{{ t("datagen.title") }}</h3>
    <label>{{ t("datagen.freqOffset") }}
      <input type="range" min="-2" max="2" step="0.01" v-model.number="gen.freq_offset_hz" />
      <span>{{ gen.freq_offset_hz.toFixed(2) }}</span>
    </label>
    <label>{{ t("datagen.rocof") }}
      <input type="range" min="-5" max="5" step="0.1" v-model.number="gen.rocof_hz_s" />
      <span>{{ gen.rocof_hz_s.toFixed(1) }}</span>
    </label>
    <button :disabled="!running" @click="trigger">{{ t("datagen.trigger") }}</button>
  </section>
</template>

<style scoped>
.panel { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
label { display: flex; align-items: center; gap: 8px; }
</style>
