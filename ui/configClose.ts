type Flusher = () => Promise<void>
type Unsubscribe = () => void
const flushers = new Set<Flusher>()
let flushing: Promise<void> | undefined
let errorBanner: HTMLDivElement | undefined

export function registerConfigurationFlusher(flush: Flusher): Unsubscribe {
  flushers.add(flush)
  return () => { flushers.delete(flush) }
}
function clearError() { errorBanner?.remove(); errorBanner = undefined }
function showError(error: unknown, retry: () => void) {
  clearError()
  errorBanner = document.createElement('div')
  errorBanner.dataset.saioCloseError = ''
  errorBanner.setAttribute('role', 'alert')
  errorBanner.style.cssText = 'position:fixed;bottom:48px;left:8px;right:8px;z-index:2147483647;padding:12px;border:1px solid currentColor;border-radius:6px;background:var(--surface,#202124);color:var(--text,#fff);font:13px system-ui;overflow-wrap:anywhere'
  errorBanner.append(document.createTextNode(`保存失败，窗口保持打开 / Save failed; window remains open: ${String(error)} `))
  const button = document.createElement('button')
  button.textContent = '重试并关闭 / Retry and close'
  button.onclick = retry
  errorBanner.append(button)
  document.body.append(errorBanner)
}

// The registry is shared by browser preferences, protocol forms and in-flight
// native configuration requests. Concurrent close requests share this work.
export function flushAllConfiguration(): Promise<void> {
  if (flushing) return flushing
  flushing = (async () => {
    for (const flush of flushers) await flush()
    clearError()
  })().catch(error => {
    showError(error, () => { void flushAllConfiguration().catch(() => {}) })
    throw error
  }).finally(() => { flushing = undefined })
  return flushing
}

const nativePending = new Set<Promise<unknown>>()
let nativeFlusherRegistered = false
export function isConfigurationCommand(command: string): boolean {
  if (command.startsWith('plugin:') || command.includes('configuration_close')) return false
  if (['simlab_save_document', 'simlab_save_preferences', 'simlab_import_document', 'simlab_export_document',
    'batch_add_data_points', 'batch_remove_data_points', 'batch_migrate_data_point_types', 'batch_update_control_options',
    'rename_connection'].includes(command)) return true
  return /^(replace_|create_|add_|remove_|delete_|update_|set_|import_|save_|new_project$|open_project$|load_(?:config|project|preset|settings|workspace))/.test(command)
}
export function trackConfigurationRequest<T>(command: string, request: Promise<T>): Promise<T> {
  if (!isConfigurationCommand(command)) return request
  if (!nativeFlusherRegistered) {
    nativeFlusherRegistered = true
    registerConfigurationFlusher(async () => {
      while (nativePending.size) await Promise.all([...nativePending])
    })
  }
  nativePending.add(request)
  void request.then(() => { nativePending.delete(request) }, () => { nativePending.delete(request) })
  return request
}

export async function installConfigurationClose(
  subscribe: (requested: () => void) => Promise<Unsubscribe> | Unsubscribe,
  confirm: () => Promise<void>,
  enable: () => Promise<void>,
): Promise<Unsubscribe> {
  let closing: Promise<void> | undefined
  function requested(): Promise<void> {
    if (closing) return closing
    const wasInert = document.body.inert
    const wasBusy = document.body.getAttribute('aria-busy')
    document.body.inert = true
    document.body.setAttribute('aria-busy', 'true')
    // Blur commits change-based inputs before Vue's queued watchers are flushed.
    const focused = document.activeElement
    if (focused instanceof HTMLElement) focused.blur()
    closing = (async () => {
      await Promise.resolve()
      await flushAllConfiguration()
      await confirm()
    })().catch(error => {
      document.body.inert = wasInert
      if (wasBusy === null) document.body.removeAttribute('aria-busy')
      else document.body.setAttribute('aria-busy', wasBusy)
      showError(error, () => { void requested() })
    }).finally(() => { closing = undefined })
    return closing
  }
  const stop = await subscribe(() => { void requested() })
  try { await enable() } catch (error) { stop(); throw error }
  return stop
}

export async function installWailsConfigurationClose(
  eventsOn: (event: string, handler: () => void) => Unsubscribe,
): Promise<Unsubscribe> {
  const host = window as unknown as { go?: { main?: { App?: {
    EnableConfigurationClose(): Promise<void>
    ConfirmConfigurationClose(): Promise<void>
  } } } }
  const app = host.go?.main?.App
  if (!app) throw new Error('Native configuration close service is unavailable')
  const mutations = new Set([
    'Launch', 'SetFavorite', 'SetTheme', 'SetLanguage', 'SavePreferences',
    'CreateConnection', 'UpdateConnection', 'DeleteConnection',
    'AddScanGroup', 'UpdateScanGroup', 'RemoveScanGroup', 'UpdateTransport',
    'AddDevice', 'UpdateDevice', 'RemoveDevice', 'AddRegister', 'UpdateRegister',
    'RemoveRegister', 'WriteRegister', 'ImportRegisters', 'SetPointMutation',
    'ClearPointMutation', 'SetDataSource', 'RemoveDataSource', 'LoadProject', 'SaveProject',
  ])
  const pending = new Set<Promise<unknown>>()
  const methods = app as unknown as Record<string, unknown>
  for (const name of mutations) {
    const original = methods[name]
    if (typeof original !== 'function') continue
    methods[name] = (...args: unknown[]) => {
      const result = Promise.resolve(original.apply(app, args))
      pending.add(result)
      void result.then(() => { pending.delete(result) }, () => { pending.delete(result) })
      return result
    }
  }
  registerConfigurationFlusher(async () => {
    while (pending.size) await Promise.all([...pending])
  })
  return installConfigurationClose(
    requested => eventsOn('saio-before-close', requested),
    () => app.ConfirmConfigurationClose(),
    () => app.EnableConfigurationClose(),
  )
}
