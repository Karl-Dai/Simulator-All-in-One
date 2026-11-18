import { computed, ref, shallowRef, type InjectionKey } from 'vue'
import { useI18n } from '@shared/i18n'
import { startAppearanceSync } from 'simlab-ui/appearance'
import type { AppearanceReader } from 'simlab-ui/contracts/appearance'

let activeSession: { dispose(): void } | null = null

export function createSuiteAppearance(enabled: boolean, reader: AppearanceReader) {
  if (!enabled) return null
  activeSession?.dispose()
  const { setEffectiveLocale } = useI18n()
  const error = shallowRef<Error | null>(null)
  const dismissed = ref(false)
  const busy = ref(false)
  const sync = startAppearanceSync(reader, {
    mode: 'suite',
    onLocale: locale => setEffectiveLocale(locale === 'en' ? 'en-US' : 'zh-CN'),
    onError: next => {
      error.value = next
      dismissed.value = false
      // Do not include native error payloads, paths or user configuration in logs.
      if (next) console.warn('Suite appearance settings could not be read')
    },
  })
  let disposed = false
  const controller = {
    visible: computed(() => error.value !== null && !dismissed.value),
    busy,
    dismiss: () => { dismissed.value = true },
    async retry() {
      if (disposed || busy.value) return
      dismissed.value = false
      busy.value = true
      try { await sync.refresh() } finally { busy.value = false }
    },
    dispose() {
      if (disposed) return
      disposed = true
      sync.dispose()
      if (activeSession === controller) {
        activeSession = null
        setEffectiveLocale(null)
      }
    },
  }
  activeSession = controller
  return controller
}

export type SuiteAppearance = ReturnType<typeof createSuiteAppearance>
export const suiteAppearanceKey: InjectionKey<SuiteAppearance> = Symbol('suite-appearance')
