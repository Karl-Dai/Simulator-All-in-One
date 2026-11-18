import { registerConfigurationFlusher } from './simlab-ui/configClose'
import { invoke } from './simlab-ui/configInvoke'
const suite = import.meta.env.VITE_SIMLAB_SUITE === '1'
type Document = Record<string, unknown>
const drafts = new Map<string, Document>()
const dirty = new Set<string>()
let pending: Promise<void> | undefined
let timer: ReturnType<typeof setTimeout> | undefined
let banner: HTMLDivElement | undefined
function status(error?: unknown) {
  if (!suite) return
  if (!banner) {
    banner = document.createElement('div')
    banner.style.cssText = 'position:fixed;bottom:8px;left:8px;z-index:2147483646;padding:8px 12px;border:1px solid currentColor;border-radius:6px;background:var(--surface,#202124);color:var(--text,#fff);font:12px system-ui;max-width:48vw;overflow-wrap:anywhere'
    document.body.append(banner)
  }
  banner.setAttribute('role', error ? 'alert' : 'status')
  banner.replaceChildren(document.createTextNode(error ? `配置未保存 / Configuration not saved: ${String(error)}` : dirty.size ? '正在保存配置 / Saving configuration…' : '配置已保存 / Configuration saved'))
  if (error) { const retry = document.createElement('button'); retry.textContent = '重试 / Retry'; retry.onclick = () => { void flushDocuments().catch(() => {}) }; banner.append(retry) }
}
export async function restoreDocuments(role: 'pmu.master' | 'pmu.sub') {
  if (!suite) return
  for (const key of ['workspace', 'settings', 'presets']) {
    let value = await invoke<Document | null>('simlab_load_document', { key, legacy: null })
    if (role === 'pmu.master' && key === 'workspace') {
      const raw = localStorage.getItem('pmusim.config.v1')
      if (value === null && raw) value = await invoke<Document | null>('simlab_load_document', { key, legacy: JSON.parse(raw) })
      if (value !== null && raw) localStorage.removeItem('pmusim.config.v1')
    }
    if (value !== null && (typeof value !== 'object' || Array.isArray(value))) throw new Error(`Invalid saved ${key}`)
    drafts.set(key, value ?? {})
  }
  registerConfigurationFlusher(flushDocuments)
  window.addEventListener('blur', () => { void flushDocuments().catch(() => {}) })
  document.addEventListener('visibilitychange', () => { void flushDocuments().catch(() => {}) })
  window.addEventListener('beforeunload', event => { if (dirty.size) { void flushDocuments().catch(() => {}); event.preventDefault(); event.returnValue = '' } })
}
export function documentValue<T extends object>(key: string): Partial<T> { return JSON.parse(JSON.stringify(drafts.get(key) ?? {})) as Partial<T> }
export async function saveDocument(key: string, value: object): Promise<void> {
  if (!suite) return
  drafts.set(key, JSON.parse(JSON.stringify(value)) as Document)
  dirty.add(key)
  status()
  return flushDocuments()
}
export function scheduleDocument(key: string, value: object) {
  if (!suite) return
  drafts.set(key, JSON.parse(JSON.stringify(value)) as Document)
  dirty.add(key); status()
  if (timer) clearTimeout(timer)
  timer = setTimeout(() => { timer = undefined; void flushDocuments().catch(() => {}) }, 300)
}
export async function flushDocuments(): Promise<void> {
  if (!suite) return
  if (timer) { clearTimeout(timer); timer = undefined }
  if (pending) { await pending; if (dirty.size) return flushDocuments(); return }
  pending = (async () => {
    while (dirty.size) {
      const key = dirty.values().next().value as string
      const value = drafts.get(key)!
      try { await invoke('simlab_save_document', { key, value }) }
      catch (error) { status(error); throw error }
      if (drafts.get(key) === value) dirty.delete(key)
    }
    status()
  })()
  try { await pending } finally { pending = undefined }
}

export async function chooseProject(action: 'import' | 'export', key = 'workspace'): Promise<boolean> {
  const dialog = await import('@tauri-apps/plugin-dialog')
  if (action === 'export') {
    const path = await dialog.save({ defaultPath: `pmu-${key}.saioproj`, filters: [{ name: 'SAIO Project', extensions: ['saioproj'] }] })
    if (!path) return false
    await flushDocuments()
    await invoke('simlab_export_document', { key, path, value: documentValue(key) })
  } else {
    const path = await dialog.open({ multiple: false, filters: [{ name: 'SAIO Project / Legacy JSON', extensions: ['saioproj', 'json'] }] })
    if (!path || Array.isArray(path)) return false
    await flushDocuments()
    const value = await invoke<Document>('simlab_import_document', { key, path })
    drafts.set(key, value)
    window.dispatchEvent(new CustomEvent('saio-document-imported', { detail: key }))
  }
  return true
}
