import { afterEach, expect, it, vi } from 'vitest'
const { sdkInvoke } = vi.hoisted(() => ({ sdkInvoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: sdkInvoke }))
afterEach(() => { vi.resetModules(); sdkInvoke.mockReset(); document.body.inert = false })
it('delegates arguments/options to the SDK and drains a pending Tauri write without wrapping read-only internals', async () => {
 vi.resetModules()
 const native = { }
 Object.defineProperty(native, 'invoke', { value: () => Promise.resolve() })
 window.__TAURI_INTERNALS__ = native as any
 const descriptor = Object.getOwnPropertyDescriptor(native, 'invoke')
 let complete!: () => void
 const saving = new Promise<void>(resolve => { complete = resolve })
 const never = new Promise<void>(() => {})
 sdkInvoke.mockImplementation((command: string) => command === 'replace_connection' ? saving : never)
 const { invoke } = await import('../src/simlab-ui/configInvoke')
 const registry = await import('../src/simlab-ui/configClose')
 const args = { protocol: 'V3' }; const options = { headers: { 'X-Test': 'test' } }
 const write = invoke('replace_connection', args, options)
 void invoke('read_data'); void invoke('connect_substation')
 expect(sdkInvoke).toHaveBeenCalledWith('replace_connection', args, options)
 let requested!: () => void
 const confirm = vi.fn().mockResolvedValue(undefined)
 await registry.installConfigurationClose(callback => { requested = callback; return () => {} }, confirm, async () => {})
 requested(); await Promise.resolve(); await Promise.resolve()
 expect(confirm).not.toHaveBeenCalled()
 complete(); await write
 for (let i = 0; i < 12; i++) await Promise.resolve()
 expect(confirm).toHaveBeenCalledOnce()
 expect(Object.getOwnPropertyDescriptor(native, 'invoke')).toEqual(descriptor)
})
