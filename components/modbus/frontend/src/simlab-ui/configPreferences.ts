import { registerConfigurationFlusher, trackConfigurationRequest } from './configClose'
type Values = Record<string, string>
type WireValue = { kind: 'text'; value: string } | { kind: 'json'; value: unknown }
type WireValues = Record<string, WireValue>
type NativePreferences = { load(legacy: WireValues): Promise<WireValues>; save(values: WireValues): Promise<void> }

export function encodePreferences(values: Values): WireValues {
  return Object.fromEntries(Object.entries(values).map(([key, raw]) => {
    if (/^\s*[\[{]/.test(raw)) {
      try { return [key, { kind: 'json', value: JSON.parse(raw) }] } catch { /* Ordinary text. */ }
    }
    return [key, { kind: 'text', value: raw }]
  }))
}
export function decodePreferences(values: WireValues): Values {
  return Object.fromEntries(Object.entries(values).map(([key, item]) => {
    if (item.kind === 'json') return [key, JSON.stringify(item.value)]
    if (item.kind === 'text' && typeof item.value === 'string') return [key, item.value]
    throw new Error(`Invalid preference value: ${key}`)
  }))
}

function nativePreferences(): NativePreferences {
  const host = window as unknown as {
    go?: { main?: { App?: { LoadPreferences(v: WireValues): Promise<WireValues>; SavePreferences(v: WireValues): Promise<void> } } }
    __TAURI_INTERNALS__?: { invoke<T>(command: string, args: unknown): Promise<T> }
  }
  const app = host.go?.main?.App
  if (app) return { load: legacy => app.LoadPreferences(legacy), save: values => app.SavePreferences(values) }
  const tauri = host.__TAURI_INTERNALS__
  if (tauri) return {
    load: legacy => tauri.invoke<WireValues>('simlab_load_preferences', { legacy }),
    save: values => trackConfigurationRequest('simlab_save_preferences', tauri.invoke<void>('simlab_save_preferences', { values })),
  }
  throw new Error('SAIO configuration service is unavailable')
}

// Install before importing modules that read preferences during initialization.
// The original WebView storage remains intact for migration and recovery.
export async function installSuitePreferences(role: string): Promise<{ flush(): Promise<void> }> {
  const legacy: Values = Object.create(null)
  const original = window.localStorage
  for (let i = 0; i < original.length; i++) {
    const key = original.key(i)
    if (key !== null) legacy[key] = original.getItem(key) ?? ''
  }
  const banner = document.createElement('div')
  banner.dataset.saioSaveStatus = 'loading'
  banner.setAttribute('role', 'status')
  banner.setAttribute('aria-live', 'polite')
  banner.style.cssText = 'position:fixed;bottom:8px;right:8px;z-index:2147483647;max-width:min(460px,90vw);padding:8px 12px;border:1px solid currentColor;border-radius:6px;background:var(--surface,#202124);color:var(--text,#fff);font:12px system-ui;overflow-wrap:anywhere'
  document.body.append(banner)
  let retry: (() => void) | undefined
  function status(kind: string, message: string, action?: () => void) {
    banner.dataset.saioSaveStatus = kind
    banner.replaceChildren(document.createTextNode(message))
    banner.setAttribute('role', kind === 'error' ? 'alert' : 'status')
    retry = action
    if (action) {
      const button = document.createElement('button')
      button.textContent = '重试 / Retry'
      button.style.marginLeft = '8px'
      button.onclick = () => retry?.()
      banner.append(button)
    }
  }
  let native: NativePreferences
  let values: Values
  for (;;) {
    status('loading', '正在恢复配置 / Restoring configuration…')
    try {
      native = nativePreferences()
      values = Object.assign(Object.create(null), decodePreferences(await native.load(encodePreferences(legacy))))
      break
    } catch (error) {
      await new Promise<void>(resolve => status('error', `${role}: 配置恢复失败；如读取仍失败，请修复配置后重启程序 / Restore failed. If reading still fails, fix configuration and restart the app: ${String(error)}`, resolve))
    }
  }
  let revision = 0
  let savedRevision = 0
  let timer: ReturnType<typeof setTimeout> | undefined
  let pending: Promise<void> | undefined
  function changed() {
    revision++
    status('saving', '正在保存配置 / Saving configuration…')
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => { timer = undefined; void flush().catch(() => {}) }, 300)
  }
  async function flush(): Promise<void> {
    if (timer) { clearTimeout(timer); timer = undefined }
    if (pending) { await pending; if (savedRevision !== revision) return flush(); return }
    if (savedRevision === revision) return
    pending = (async () => {
      while (savedRevision !== revision) {
        const savingRevision = revision
        const snapshot = { ...values }
        status('saving', '正在保存配置 / Saving configuration…')
        try { await native.save(encodePreferences(snapshot)) }
        catch (error) {
          status('error', `配置未保存 / Configuration not saved: ${String(error)}`, () => { void flush().catch(() => {}) })
          throw error
        }
        savedRevision = savingRevision
      }
      status('saved', '配置已保存 / Configuration saved')
    })()
    try { await pending } finally { pending = undefined }
  }
  const storage: Storage = {
    get length() { return Object.keys(values).length },
    key(index) { return Object.keys(values)[index] ?? null },
    getItem(key) { return Object.prototype.hasOwnProperty.call(values, String(key)) ? values[String(key)] : null },
    setItem(key, value) { key = String(key); value = String(value); if (values[key] !== value) { values[key] = value; changed() } },
    removeItem(key) { key = String(key); if (Object.prototype.hasOwnProperty.call(values, key)) { delete values[key]; changed() } },
    clear() { if (Object.keys(values).length) { values = Object.create(null); changed() } },
  }
  Object.defineProperty(window, 'localStorage', { configurable: true, value: storage })
  const flushVisible = () => { void flush().catch(() => {}) }
  window.addEventListener('pagehide', flushVisible)
  window.addEventListener('blur', flushVisible)
  document.addEventListener('visibilitychange', flushVisible)
  document.addEventListener('focusout', flushVisible)
  window.addEventListener('beforeunload', event => {
    if (savedRevision !== revision) { flushVisible(); event.preventDefault(); event.returnValue = '' }
  })
  registerConfigurationFlusher(flush)
  status('saved', '配置已恢复 / Configuration restored')
  return { flush }
}
