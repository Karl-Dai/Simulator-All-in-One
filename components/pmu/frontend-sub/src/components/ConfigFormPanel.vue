<script setup lang="ts">
import { reactive, computed, ref, onMounted, watch } from "vue";
import { invoke } from "../simlab-ui/configInvoke";
import { load } from "@tauri-apps/plugin-store";
import type { ConfigInput } from "../types";
import { running, listenPorts } from "../composables/useSubEvents";
import { t } from "../i18n";

import { documentValue, scheduleDocument, saveDocument, flushDocuments, chooseProject } from "../suiteWorkspace";
const suiteBuild = import.meta.env.VITE_SIMLAB_SUITE === "1";

const form = reactive<ConfigInput>({
  protocol: "V3", idcode: "0GX00GP1", stn: "测试子站",
  mgmt_port: 8000, data_port: 0, data_rate_fps: 50,
  phasors: [{ magnitude: 1000, phase_deg: 0 }],
  analogs: [300, 3000], digitals: [10],
});

if (suiteBuild) Object.assign(form, documentValue<ConfigInput>("workspace"));
watch(form, () => { if (suiteBuild) scheduleDocument("workspace", form); }, { deep: true });

const projectSection = ref("workspace");
async function projectAction(action: "import" | "export") {
  if (await chooseProject(action, projectSection.value) && action === "import") {
    if (projectSection.value === "workspace") Object.assign(form, documentValue<ConfigInput>("workspace"));
    if (projectSection.value === "presets") await refreshPresetNames();
  }
}
const busy = ref(false);
const error = ref("");
const invalidPhasorCount = ref(false);
async function runAction(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true; error.value = "";
  try { await action(); }
  catch (cause) { error.value = String(cause); }
  finally { busy.value = false; }
}

const phasorCount = computed({
  get: () => form.phasors.length,
  set: (n: number) => {
    invalidPhasorCount.value = !Number.isInteger(n) || n < 0 || n > 65535;
    if (invalidPhasorCount.value) return;
    const cur = form.phasors.length;
    if (n > cur) for (let i = cur; i < n; i++) form.phasors.push({ magnitude: 1000, phase_deg: 0 });
    else form.phasors.length = Math.max(0, n);
  },
});

async function start() {
  if (form.protocol === "V2" && form.mgmt_port === 8000) form.mgmt_port = 7000;
  if (form.protocol === "V3" && form.mgmt_port === 7000) form.mgmt_port = 8000;
  if (suiteBuild) { await saveDocument("workspace", form); await flushDocuments(); }
  await invoke("start_substation", { config: { ...form } });
  running.value = true;
}
async function stop() { await invoke("stop_substation"); running.value = false; }
async function apply() { if (suiteBuild) await saveDocument("workspace", form); await invoke("update_config", { config: { ...form } }); }

// --- Presets ---
const presetStorePath = Promise.resolve("presets.json");
const presetName = ref("");
const presetNames = ref<string[]>([]);
const selectedPreset = ref("");

async function refreshPresetNames() {
  if (suiteBuild) { presetNames.value = Object.keys(documentValue("presets")); return; }
  const store = await load(await presetStorePath, { defaults: {}, autoSave: false });
  presetNames.value = await store.keys();
}
async function savePreset() {
  if (!presetName.value.trim()) return;
  if (suiteBuild) {
    await saveDocument("presets", { ...documentValue("presets"), [presetName.value.trim()]: JSON.parse(JSON.stringify(form)) });
    await refreshPresetNames(); selectedPreset.value = presetName.value.trim(); return;
  }
  const store = await load(await presetStorePath, { defaults: {}, autoSave: false });
  await store.set(presetName.value.trim(), { ...form });
  await store.save();
  await refreshPresetNames();
  selectedPreset.value = presetName.value.trim();
}
async function loadPreset() {
  if (!selectedPreset.value) return;
  if (suiteBuild) {
    const cfg = documentValue<Record<string, ConfigInput>>("presets")[selectedPreset.value];
    if (cfg) { Object.assign(form, cfg); await saveDocument("workspace", form); }
    return;
  }
  const store = await load(await presetStorePath, { defaults: {}, autoSave: false });
  const cfg = await store.get<ConfigInput>(selectedPreset.value);
  if (cfg) Object.assign(form, cfg);
}
async function deletePreset() {
  if (!selectedPreset.value) return;
  if (suiteBuild) {
    const presets: Record<string, unknown> = { ...documentValue<Record<string, unknown>>("presets") }; delete presets[selectedPreset.value];
    await saveDocument("presets", presets); selectedPreset.value = ""; await refreshPresetNames(); return;
  }
  const store = await load(await presetStorePath, { defaults: {}, autoSave: false });
  await store.delete(selectedPreset.value);
  await store.save();
  selectedPreset.value = "";
  await refreshPresetNames();
}
onMounted(() => { void runAction(refreshPresetNames); });
</script>

<template>
  <section class="panel" :aria-busy="busy">
    <fieldset :disabled="busy" class="config-fields">
    <h3>{{ t("config.title") }}</h3>
    <div v-if="suiteBuild" class="preset-load" role="group" aria-label="Configuration project">
      <select v-model="projectSection" aria-label="Project contents">
        <option value="workspace">当前配置 / Configuration</option>
        <option value="presets">预设库 / Presets</option>
        <option value="settings">数据生成 / Generator</option>
      </select>
      <button :disabled="running" @click="runAction(() => projectAction('import'))">导入 / Import</button>
      <button @click="runAction(() => projectAction('export'))">导出 / Export</button>
    </div>
    <label>{{ t("config.protocol") }}
      <select v-model="form.protocol" :disabled="running">
        <option value="V2">V2 (Q/GDW 131-2006)</option>
        <option value="V3">V3 (GB/T 26865.2-2011)</option>
      </select>
    </label>
    <label>{{ t("config.stn") }} <input v-model="form.stn" /></label>
    <label>{{ t("config.idcode") }} <input v-model="form.idcode" maxlength="8" /></label>
    <label>{{ t("config.mgmtPort") }} <input type="number" v-model.number="form.mgmt_port" :disabled="running" /></label>
    <label>{{ t("config.dataPort") }} <input type="number" v-model.number="form.data_port" :disabled="running" /></label>
    <label>{{ t("config.rate") }} <input type="number" v-model.number="form.data_rate_fps" /></label>
    <label>{{ t("config.phasorCount") }} <input type="number" min="0" max="65535" step="1" v-model.number="phasorCount" :aria-invalid="invalidPhasorCount" aria-describedby="phasor-count-error" /></label>
    <p v-if="invalidPhasorCount" id="phasor-count-error" class="error" role="alert">{{ t("config.invalidPhasorCount") }}</p>
    <div v-for="(p, i) in form.phasors" :key="i" class="phasor-row">
      {{ t("config.phasorMag", { i }) }} <input type="number" v-model.number="p.magnitude" />
      {{ t("config.phasorPhase") }} <input type="number" v-model.number="p.phase_deg" />
    </div>
    <div class="actions">
      <button v-if="!running" :disabled="invalidPhasorCount" @click="runAction(start)">{{ t("config.start") }}</button>
      <button v-else @click="runAction(stop)">{{ t("config.stop") }}</button>
      <button :disabled="!running || invalidPhasorCount" @click="runAction(apply)">{{ t("config.apply") }}</button>
    </div>
    <p v-if="listenPorts">{{ t("config.listening", { mgmt: listenPorts.mgmt, data: listenPorts.data }) }}</p>

    <!-- Presets -->
    <hr />
    <h4>{{ t("preset.title") }}</h4>
    <div class="preset-save">
      <input v-model="presetName" :placeholder="t('preset.namePlaceholder')" :aria-label="t('preset.namePlaceholder')" />
      <button @click="runAction(savePreset)" :disabled="!presetName.trim()">{{ t("preset.save") }}</button>
    </div>
    <div class="preset-load">
      <select v-model="selectedPreset" :aria-label="t('preset.select')">
        <option value="" disabled>{{ t("preset.select") }}</option>
        <option v-for="name in presetNames" :key="name" :value="name">{{ name }}</option>
      </select>
      <button @click="runAction(loadPreset)" :disabled="running || !selectedPreset">{{ t("preset.load") }}</button>
      <button @click="runAction(deletePreset)" :disabled="!selectedPreset">{{ t("preset.delete") }}</button>
    </div>
    </fieldset>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
  </section>
</template>

<style scoped>
.panel { padding: 12px; min-width: 0; }
.config-fields { border: 0; padding: 0; margin: 0; min-width: 0; display: flex; flex-direction: column; gap: 6px; }
.error { color: var(--danger, #b91c1c); overflow-wrap: anywhere; }
input, select { min-width: 0; box-sizing: border-box; }
label > input, label > select { width: 55%; }
.preset-save > input, .preset-load > select { flex: 1; min-width: 0; }
.phasor-row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto minmax(0, 1fr); gap: 4px; align-items: center; }
.phasor-row input { width: 100%; }
label { display: flex; justify-content: space-between; gap: 8px; }
.phasor-row { font-size: 12px; }
.actions { display: flex; gap: 8px; margin-top: 8px; }
.preset-save { display: flex; gap: 6px; }
.preset-load { display: flex; gap: 6px; }
</style>
