<script setup lang="ts">
import { useI18n } from 'shared-frontend'
import type { Locale } from 'shared-frontend'

const { t, locale, setLocale, languageManaged } = useI18n()

function pick(next: Locale) {
  if (locale.value !== next) setLocale(next)
}
</script>

<template>
  <div class="lang-toggle" role="group" aria-label="Language">
    <button
      type="button"
      class="lang-toggle-btn"
      :class="{ active: locale === 'zh-CN' }"
      :title="languageManaged ? t('suiteAppearance.managed') : '切换到中文'"
      :disabled="languageManaged"
      @click="pick('zh-CN')"
    >中</button>
    <button
      type="button"
      class="lang-toggle-btn"
      :class="{ active: locale === 'en-US' }"
      :title="languageManaged ? t('suiteAppearance.managed') : 'Switch to English'"
      :disabled="languageManaged"
      @click="pick('en-US')"
    >EN</button>
    <span v-if="languageManaged" class="managed-label">{{ t('suiteAppearance.managed') }}</span>
  </div>
</template>

<style scoped>
.lang-toggle {
  display: inline-flex;
  margin-right: 8px;
  border-radius: 4px;
  overflow: hidden;
  background: var(--c-control-bg);
  border: 1px solid var(--c-surface1);
}

.lang-toggle-btn {
  padding: 2px 10px;
  font-size: 12px;
  background: transparent;
  border: none;
  color: var(--c-subtext0);
  cursor: pointer;
  font-family: inherit;
  line-height: 1.6;
}

.lang-toggle-btn:hover:not(.active) {
  background: var(--c-hover-bg);
  color: var(--c-text);
}

.lang-toggle-btn.active {
  background: var(--c-selection-bg);
  color: var(--c-selection-text);
  font-weight: 600;
}
.managed-label { align-self: center; padding-inline: 5px; font-size: 11px; white-space: nowrap; }
button:disabled { cursor: default; }
</style>
