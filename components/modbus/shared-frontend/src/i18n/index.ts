import { ref, readonly } from 'vue'
import { detectInitialLocale, storeLocale } from './detect'
import zhCN, { type Messages } from './locales/zh-CN'
import enUS from './locales/en-US'
import type { Locale } from './types'

export type { Locale } from './types'

const dictionaries: Record<Locale, Messages> = {
  'zh-CN': zhCN,
  'en-US': enUS,
}

const locale = ref<Locale>(detectInitialLocale())
const languageManaged = ref(false)

function lookup(dict: Messages, key: string): string | null {
  const parts = key.split('.')
  let cur: unknown = dict
  for (const p of parts) {
    if (cur && typeof cur === 'object' && p in (cur as Record<string, unknown>)) {
      cur = (cur as Record<string, unknown>)[p]
    } else {
      return null
    }
  }
  return typeof cur === 'string' ? cur : null
}

function interpolate(tpl: string, params?: Record<string, string | number>): string {
  if (!params) return tpl
  return tpl.replace(/\{(\w+)\}/g, (_, k) =>
    k in params ? String(params[k]) : `{${k}}`,
  )
}

function translate(key: string, params?: Record<string, string | number>): string {
  const tpl =
    lookup(dictionaries[locale.value], key) ??
    lookup(dictionaries['en-US'], key) ??
    key
  return interpolate(tpl, params)
}

function setLocale(next: Locale): void {
  if (languageManaged.value) return
  locale.value = next
  storeLocale(next)
}

function setEffectiveLocale(next: Locale | null): void {
  if (next !== null && next !== 'zh-CN' && next !== 'en-US') return
  languageManaged.value = next !== null
  locale.value = next ?? detectInitialLocale()
}

export function useI18n() {
  return {
    t: translate,
    locale,
    setLocale,
    setEffectiveLocale,
    languageManaged: readonly(languageManaged),
  }
}
