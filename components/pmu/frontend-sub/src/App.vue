<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import ConfigFormPanel from "./components/ConfigFormPanel.vue";
import DataGenPanel from "./components/DataGenPanel.vue";
import StatusLogPanel from "./components/StatusLogPanel.vue";
import SentDataPanel from "./components/SentDataPanel.vue";
import { useToast } from "./composables/useToast";
import { useI18n } from "./i18n";
const suiteBuild = import.meta.env.VITE_SIMLAB_SUITE === "1";
const appVersion = ref("");
onMounted(async () => {
  if (suiteBuild) appVersion.value = await getVersion();
});
const { toasts, dismiss } = useToast();
const { t, locale, setLocale } = useI18n();
</script>

<template>
  <div class="app">
    <header>
      <h1>{{ t("app.title") }}<small v-if="appVersion" class="app-version"> · SAIO v{{ appVersion }}</small></h1>
      <div class="lang-toggle" role="group" aria-label="Language">
        <button :class="{ on: locale === 'zh' }" @click="setLocale('zh')">中</button>
        <button :class="{ on: locale === 'en' }" @click="setLocale('en')">EN</button>
      </div>
    </header>
    <main class="layout">
      <div class="left">
        <ConfigFormPanel />
        <DataGenPanel />
        <StatusLogPanel />
      </div>
      <div class="right">
        <SentDataPanel />
      </div>
    </main>
    <div class="toasts">
      <div v-for="t in toasts" :key="t.id" class="toast" :class="t.kind" @click="dismiss(t.id)">{{ t.message }}</div>
    </div>
  </div>
</template>

<style>
body { margin: 0; font-family: system-ui, sans-serif; }
.app { display: flex; flex-direction: column; height: 100vh; }
header { padding: 8px 16px; border-bottom: 1px solid #ddd; display: flex; align-items: center; justify-content: space-between; }
.app-version { font-size: 11px; font-weight: normal; }
header h1 { font-size: 16px; margin: 0; }
.lang-toggle { display: flex; border: 1px solid #bbb; border-radius: 3px; overflow: hidden; }
.lang-toggle button { background: #f0f0f0; border: none; color: #333; font-size: 11px; font-weight: 600; padding: 2px 8px; cursor: pointer; }
.lang-toggle button + button { border-left: 1px solid #bbb; }
.lang-toggle button:hover { background: #e4e4e4; }
.lang-toggle button.on { background: #2563a8; color: #fff; }
.layout { display: grid; grid-template-columns: 380px minmax(0, 1fr); min-height: 0; flex: 1; overflow: hidden; }
.left { min-width: 0; overflow: auto; border-right: 1px solid #eee; }
.right { min-width: 0; overflow: auto; }
.toasts { position: fixed; top: 12px; right: 12px; z-index: 9999; display: flex; flex-direction: column; gap: 6px; }
.toast { padding: 8px 14px; border-radius: 6px; box-shadow: 0 2px 8px rgba(0,0,0,.2); cursor: pointer; font-size: 13px; background: #444; color: #fff; max-width: 320px; word-break: break-word; }
.toast.error { background: #d33; color: #fff; }
.toast.success { background: #2a2; color: #fff; }
@media (max-width: 700px) {
  .layout { display: block; overflow: auto; }
  .left, .right { overflow: visible; }
  .left { border-right: 0; }
  .toasts { max-width: calc(100vw - 24px); }
}
</style>
