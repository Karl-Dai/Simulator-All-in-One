import { afterEach, beforeEach, expect, it, vi } from 'vitest'
const host = window as unknown as { go?: unknown }
beforeEach(() => { vi.resetModules(); document.body.replaceChildren(); document.body.inert = false; document.body.removeAttribute('aria-busy') })
afterEach(() => { delete host.go; document.body.inert = false })
function deferred() { let resolve!: () => void; const promise = new Promise<void>(done => { resolve = done }); return { promise, resolve } }
async function settle() { for (let i = 0; i < 12; i++) await Promise.resolve() }
it('waits for an active Wails configuration request and a second flusher before one close acknowledgement', async () => {
 const store = await import('../../ui/configClose')
 const native = deferred(); const second = deferred(); let requested!: () => void
 const confirm = vi.fn().mockResolvedValue(undefined); const enable = vi.fn().mockResolvedValue(undefined)
 const app = { SetTheme: vi.fn().mockReturnValue(native.promise), EnableConfigurationClose: enable, ConfirmConfigurationClose: confirm }
 host.go = { main: { App: app } }
 await store.installWailsConfigurationClose((_event, callback) => { requested = callback; return () => {} })
 expect(enable).toHaveBeenCalledOnce()
 const save = app.SetTheme('dark')
 const flushSecond = vi.fn(() => second.promise); store.registerConfigurationFlusher(flushSecond)
 requested(); requested(); await settle()
 expect(document.body.inert).toBe(true); expect(confirm).not.toHaveBeenCalled(); expect(flushSecond).not.toHaveBeenCalled()
 native.resolve(); await save; await settle()
 expect(flushSecond).toHaveBeenCalledOnce(); expect(confirm).not.toHaveBeenCalled()
 second.resolve(); await settle()
 expect(confirm).toHaveBeenCalledOnce(); expect(app.SetTheme).toBeTypeOf('function')
})
it('keeps the window editable on save error and retries the complete close flow', async () => {
 const store = await import('../../ui/configClose'); let requested!: () => void
 const flush = vi.fn().mockRejectedValueOnce(new Error('database locked')).mockResolvedValue(undefined)
 store.registerConfigurationFlusher(flush)
 const confirm = vi.fn().mockResolvedValue(undefined)
 await store.installConfigurationClose(callback => { requested = callback; return () => {} }, confirm, async () => {})
 requested(); await settle()
 expect(confirm).not.toHaveBeenCalled(); expect(document.body.inert).toBe(false)
 expect(document.querySelector('[role="alert"]')?.textContent).toContain('database locked')
 document.querySelector<HTMLButtonElement>('[data-saio-close-error] button')!.click(); await settle()
 expect(flush).toHaveBeenCalledTimes(2); expect(confirm).toHaveBeenCalledOnce()
 expect(document.querySelector('[data-saio-close-error]')).toBeNull()
})
it('tracks Tauri configuration promises while reads and connections do not delay close', async () => {
 const store = await import('../../ui/configClose'); let requested!: () => void
 const writing = deferred(); const never = new Promise<void>(() => {})
 store.trackConfigurationRequest('simlab_save_document', writing.promise)
 store.trackConfigurationRequest('read_registers', never); store.trackConfigurationRequest('connect_substation', never)
 expect(store.isConfigurationCommand('batch_migrate_data_point_types')).toBe(true)
 expect(store.isConfigurationCommand('simlab_confirm_configuration_close')).toBe(false)
 expect(store.isConfigurationCommand('plugin:event|listen')).toBe(false)
 const confirm = vi.fn().mockResolvedValue(undefined)
 await store.installConfigurationClose(callback => { requested = callback; return () => {} }, confirm, async () => {})
 requested(); await settle(); expect(confirm).not.toHaveBeenCalled()
 writing.resolve(); await settle(); expect(confirm).toHaveBeenCalledOnce()
})
it('blurs the edited control and drains queued change watchers before flushing', async () => {
 const store = await import('../../ui/configClose'); let requested!: () => void; let committed = false
 const input = document.createElement('input'); document.body.append(input); input.focus()
 input.addEventListener('blur', () => { void Promise.resolve().then(() => { committed = true }) })
 store.registerConfigurationFlusher(async () => { expect(committed).toBe(true) })
 const confirm = vi.fn().mockResolvedValue(undefined)
 await store.installConfigurationClose(callback => { requested = callback; return () => {} }, confirm, async () => {})
 requested(); await settle(); expect(confirm).toHaveBeenCalledOnce()
})
it('serializes concurrent registry requests and supports unregistering a flusher', async () => {
 const store = await import('../../ui/configClose'); const work = deferred()
 const first = vi.fn(() => work.promise); const second = vi.fn().mockResolvedValue(undefined)
 store.registerConfigurationFlusher(first); const stop = store.registerConfigurationFlusher(second); stop()
 const a = store.flushAllConfiguration(); const b = store.flushAllConfiguration()
 expect(a).toBe(b); expect(first).toHaveBeenCalledOnce(); work.resolve(); await a
 expect(second).not.toHaveBeenCalled()
})
it('waits for launcher startup and its recent-entry commit before acknowledging close', async () => {
 const store = await import('../../ui/configClose'); const launch = deferred(); let requested!: () => void
 const confirm = vi.fn().mockResolvedValue(undefined)
 const app = { Launch: vi.fn().mockReturnValue(launch.promise), EnableConfigurationClose: vi.fn().mockResolvedValue(undefined), ConfirmConfigurationClose: confirm }
 host.go = { main: { App: app } }
 await store.installWailsConfigurationClose((_event, callback) => { requested = callback; return () => {} })
 const opening = app.Launch('pmu.master')
 requested(); await settle(); expect(confirm).not.toHaveBeenCalled()
 launch.resolve(); await opening; await settle(); expect(confirm).toHaveBeenCalledOnce()
})
