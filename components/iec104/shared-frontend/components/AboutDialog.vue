<script setup lang="ts">
import { vModal } from "@app/simlab-ui/modal"
import { computed, ref, onMounted } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { ABOUT_RELEASE_NOTES, APP_NAME, REPO_URL, RELEASES_URL, SIMLAB_URL } from '@app/releaseNotes'
import { useI18n } from '../i18n'
import { documentationUrlForLocale } from '../i18n/documentation'
import { useClipboardFlash } from '../composables/useClipboardFlash'
import { isSimLabSuite } from '../suite'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ (e: 'close'): void }>()

const { t, locale } = useI18n()
const { flash: copied, openOrCopy } = useClipboardFlash()
const version = ref('')
const releaseNotes = computed(() => isSimLabSuite
  ? [locale.value === 'zh-CN' ? '随 SAIO 统一更新。' : 'Updates are delivered with SAIO.']
  : ABOUT_RELEASE_NOTES[locale.value])
const documentationUrl = computed(() => documentationUrlForLocale(REPO_URL, locale.value))
onMounted(async () => {
  try { version.value = await getVersion() } catch { version.value = '' }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-pop">
    <div v-if="visible" v-modal="() => emit('close')" class="about-backdrop dialog-blur" @mousedown.self="emit('close')">
      <div class="about-box" role="dialog" aria-modal="true">
        <div class="about-header">
          <div class="about-title">{{ APP_NAME }}</div>
          <div class="about-version">{{ isSimLabSuite ? 'SAIO ' : '' }}v{{ version || '—' }}</div>
        </div>
        <div class="about-body">
          <div class="about-section-title">{{ t('about.whatsNew') }}</div>
          <ul class="about-notes">
            <li v-for="(note, i) in releaseNotes" :key="i">{{ note }}</li>
          </ul>
          <div class="about-links">
            <a
              href="#"
              data-testid="simlab-link"
              :title="SIMLAB_URL"
              @click.prevent="openOrCopy(SIMLAB_URL, t('about.onlineDemoLabel'))"
            >{{ t('about.onlineDemoLabel') }}</a>
            <span class="about-sep">·</span>
            <a
              href="#"
              data-testid="documentation-link"
              @click.prevent="openOrCopy(documentationUrl, t('about.homepageLabel'))"
            >{{ t('about.homepageLabel') }}</a>
            <template v-if="!isSimLabSuite">
              <span class="about-sep">·</span>
              <a href="#" data-testid="release-history-link" @click.prevent="openOrCopy(RELEASES_URL, t('about.releasesLabel'))">{{ t('about.releasesLabel') }}</a>
            </template>
            <span v-if="copied" class="about-toast">{{ copied }}</span>
          </div>
        </div>
        <div class="about-footer">
          <button class="btn btn-primary" @click="emit('close')">{{ t('common.close') }}</button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.about-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2100;
}
.about-box {
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  background: var(--c-base);
  border: 1px solid var(--c-surface1);
  border-radius: 8px;
  width: 420px;
  max-width: 90vw;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}
.about-header {
  flex-shrink: 0;
  padding: 18px 22px 10px;
  border-bottom: 1px solid var(--c-surface0);
}
.about-title { font-size: 16px; font-weight: 600; color: var(--c-text); }
.about-version { font-size: 12px; color: var(--c-subtext0); margin-top: 2px; font-variant-numeric: tabular-nums; }
.about-body { min-height: 0; overflow-y: auto; padding: 14px 22px 8px; color: var(--c-subtext1); font-size: 13px; }
.about-section-title { color: var(--c-text); font-weight: 600; margin-bottom: 6px; }
.about-notes { margin: 0 0 14px; padding-left: 18px; line-height: 1.65; }
.about-links { font-size: 12px; display: flex; align-items: center; flex-wrap: wrap; gap: 4px; }
.about-links a { color: var(--c-blue); text-decoration: none; cursor: pointer; }
.about-links a:hover { text-decoration: underline; }
.about-sep { color: var(--c-surface2); }
.about-toast { color: var(--c-green); margin-left: 10px; font-size: 11px; }
.about-footer { flex-shrink: 0; display: flex; justify-content: flex-end; padding: 8px 22px 16px; }
.btn { padding: 7px 20px; border: none; border-radius: 6px; cursor: pointer; font-size: 13px; }
.btn-primary { background: var(--c-blue); color: var(--c-base); }
.btn-primary:hover { background: var(--c-sapphire); }
</style>
