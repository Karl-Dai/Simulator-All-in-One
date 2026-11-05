type NativeRuntime = {
  Environment(): Promise<{ platform: string }>
  WindowSetDarkTheme(): void
  WindowSetLightTheme(): void
  WindowSetSystemDefaultTheme(): void
}

/** Follow the effective suite palette, falling back to the standalone choice. */
export async function startNativeWindowTheme(): Promise<() => void> {
  const runtime = (window as unknown as { runtime?: NativeRuntime }).runtime
  if (!runtime || (await runtime.Environment()).platform !== 'windows') return () => {}
  const root = document.documentElement
  let last: string | undefined
  function apply() {
    const theme = root.dataset.simlabTheme ?? root.dataset.theme ?? 'system'
    if (theme === last) return
    if (theme === 'dark') runtime!.WindowSetDarkTheme()
    else if (theme === 'light') runtime!.WindowSetLightTheme()
    else runtime!.WindowSetSystemDefaultTheme()
    last = theme
  }
  const observer = new MutationObserver(apply)
  observer.observe(root, { attributes: true, attributeFilter: ['data-theme', 'data-simlab-theme'] })
  apply()
  return () => observer.disconnect()
}
