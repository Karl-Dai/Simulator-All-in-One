import { afterEach, expect, it, vi } from 'vitest'
const state = vi.hoisted(() => ({ mounted: false, loaded: false, order: [] as string[], finish: undefined as (() => void) | undefined }))
vi.mock('@tauri-apps/api/event', () => ({ listen: async (name: string) => {
  if (name !== 'saio-before-close') throw new Error(`Unexpected event: ${name}`)
  state.order.push('listening')
  return () => {}
} }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: async (command: string) => {
  if (command !== 'simlab_enable_configuration_close') throw new Error(`Unexpected command: ${command}`)
  state.order.push('close-enabled')
} }))
vi.mock('simlab-ui/configPreferences', () => ({ installSuitePreferences: async () => {
  state.order.push('preferences')
  state.loaded = true
  await new Promise<void>(resolve => { state.finish = resolve })
  return { flush: async () => {} }
} }))
vi.mock('./bootstrap', () => { state.order.push('application'); state.mounted = true; return {} })
afterEach(() => { vi.unstubAllEnvs(); vi.resetModules(); state.loaded = false; state.mounted = false; state.order.length = 0; state.finish = undefined })
it('enables native close protection before restoring preferences and importing the application', async () => {
  vi.stubEnv('VITE_SIMLAB_SUITE', '1')
  await import('./main')
  await vi.waitFor(() => expect(state.loaded).toBe(true))
  expect(state.order).toEqual(['listening', 'close-enabled', 'preferences'])
  expect(state.mounted).toBe(false)
  state.finish?.()
  await vi.waitFor(() => expect(state.mounted).toBe(true))
  expect(state.order).toEqual(['listening', 'close-enabled', 'preferences', 'application'])
})
