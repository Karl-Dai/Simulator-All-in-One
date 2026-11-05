import {
  DEFAULT_APPEARANCE,
  parseAppearance,
  resolveLanguage,
  type AppearanceReader,
  type AppearanceV1,
  type ResolvedLocale,
} from './contracts/appearance'

export interface AppearanceSyncOptions {
  /** The application must apply this override without saving its local preference. */
  onLocale(locale: ResolvedLocale): void
  /** A null value clears the previous synchronization error. */
  onError(error: Error | null): void
  /** Standalone readers must use their application's own settings source. */
  mode?: 'suite' | 'standalone'
}

export interface AppearanceSync {
  refresh(): Promise<void>
  dispose(): void
}

const activeSyncs = new WeakMap<Document, () => void>()

/** Synchronize only the injected preferences; never access a host bridge or storage. */
export function startAppearanceSync(
  reader: AppearanceReader,
  options: AppearanceSyncOptions,
): AppearanceSync {
  activeSyncs.get(document)?.()
  const root = document.documentElement
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  let current: AppearanceV1 = { ...DEFAULT_APPEARANCE }
  let lastLocale: ResolvedLocale | undefined
  let lastError: string | undefined
  let stopped = false
  let requestId = 0
  let pending: Promise<void> | undefined
  let timer: ReturnType<typeof setTimeout> | undefined

  function apply() {
    if (stopped) return
    const theme = current.theme === 'system' ? (media.matches ? 'dark' : 'light') : current.theme
    if (options.mode !== 'standalone') {
      root.dataset.simlab = 'component'
      root.dataset.simlabTheme = theme
    }
    root.dataset.theme = theme
    root.style.colorScheme = theme
    const locale = resolveLanguage(current.language, navigator.languages?.[0] || navigator.language)
    root.lang = locale
    if (locale !== lastLocale) {
      options.onLocale(locale)
      lastLocale = locale
    }
  }

  function report(error: Error | null) {
    if (stopped) return
    const signature = error ? `${error.name}: ${error.message}` : undefined
    if (signature === lastError) return
    lastError = signature
    options.onError(error)
  }

  function refresh(): Promise<void> {
    clearTimeout(timer)
    if (stopped || document.hidden) return Promise.resolve()
    if (pending) return pending
    const id = ++requestId
    pending = Promise.resolve().then(async () => {
      try {
        if (stopped || id !== requestId || document.hidden) return
        const value = parseAppearance(await reader.readAppearance())
        if (stopped || id !== requestId || document.hidden) return
        current = value
        apply()
        report(null)
      } catch (cause) {
        if (stopped || id !== requestId || document.hidden) return
        report(cause instanceof Error ? cause : new Error(String(cause)))
      } finally {
        if (!stopped && id === requestId) {
          pending = undefined
          if (!document.hidden) timer = setTimeout(() => { void refresh() }, 1500)
        }
      }
    })
    return pending
  }

  function visibility() {
    clearTimeout(timer)
    if (document.hidden) {
      // Native calls cannot be cancelled; invalidate their results and let a
      // restored window read immediately even if an earlier call is still slow.
      requestId++
      pending = undefined
    } else {
      void refresh()
    }
  }

  function dispose() {
    if (stopped) return
    stopped = true
    requestId++
    clearTimeout(timer)
    media.removeEventListener('change', apply)
    document.removeEventListener('visibilitychange', visibility)
    window.removeEventListener('languagechange', apply)
    window.removeEventListener('pagehide', dispose)
    if (activeSyncs.get(document) === dispose) activeSyncs.delete(document)
  }

  activeSyncs.set(document, dispose)
  apply()
  media.addEventListener('change', apply)
  document.addEventListener('visibilitychange', visibility)
  window.addEventListener('languagechange', apply)
  window.addEventListener('pagehide', dispose)
  void refresh()
  return { refresh, dispose }
}
