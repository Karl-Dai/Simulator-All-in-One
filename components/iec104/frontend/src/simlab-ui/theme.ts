/** Shared by all eight suite webviews. No protocol or project data crosses IPC. */
type Theme = 'light' | 'dark' | 'system'
type NativeWindow = Window & {
  go?: { suitehost?: { Appearance?: { Theme(): Promise<string> } } }
  __TAURI_INTERNALS__?: { invoke<T>(command: string): Promise<T> }
}

export function startSuiteTheme() {
  const root = document.documentElement
  const native = window as NativeWindow
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  let theme: Theme = 'system'
  let stopped = false
  let pending = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let warned = false
  root.dataset.simlab = 'component'

  function apply() {
    const resolved = theme === 'system' ? (media.matches ? 'dark' : 'light') : theme
    // A separate attribute prevents a standalone theme composable from
    // accidentally overriding the suite palette when the OS changes.
    root.dataset.simlabTheme = resolved
    root.dataset.theme = resolved
    root.style.colorScheme = resolved
  }

  async function refresh() {
    clearTimeout(timer)
    if (stopped || pending || document.hidden) return
    pending = true
    try {
      const value = native.go?.suitehost?.Appearance
        ? await native.go.suitehost.Appearance.Theme()
        : await native.__TAURI_INTERNALS__?.invoke<string>('simlab_theme')
      if (value !== 'system' && value !== 'light' && value !== 'dark') {
        throw new Error('Invalid SimLab theme response')
      }
      if (!stopped) { theme = value; apply() }
      warned = false
    } catch (error) {
      if (!warned) console.warn('Cannot synchronize SimLab appearance; keeping current theme', error)
      warned = true
    } finally {
      pending = false
      if (!stopped && !document.hidden) timer = setTimeout(refresh, 1500)
    }
  }
  function visibility() {
    clearTimeout(timer)
    if (!document.hidden) void refresh()
  }
  function stop() {
    stopped = true
    clearTimeout(timer)
    media.removeEventListener('change', apply)
    document.removeEventListener('visibilitychange', visibility)
    window.removeEventListener('pagehide', stop)
  }
  apply()
  void refresh()
  media.addEventListener('change', apply)
  document.addEventListener('visibilitychange', visibility)
  window.addEventListener('pagehide', stop)
  return stop
}
