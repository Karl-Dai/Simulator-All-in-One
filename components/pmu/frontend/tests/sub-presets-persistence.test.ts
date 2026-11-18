import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
afterEach(() => { vi.unstubAllEnvs(); vi.unstubAllGlobals() })
it('loads a preset into an independent form so editing does not alter the library', async () => {
 vi.resetModules(); vi.stubEnv('VITE_SIMLAB_SUITE', '1')
 vi.stubGlobal('localStorage', { getItem: () => null, setItem: () => {}, removeItem: () => {} })
 const preset = { protocol: 'V3', idcode: '0GX00GP1', stn: 'Saved', mgmt_port: 8000, data_port: 0, data_rate_fps: 50, phasors: [{ magnitude: 123, phase_deg: 0 }], analogs: [], digitals: [] }
 const writes: Array<{key: string, value: unknown}> = []
 window.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
  if (command === 'simlab_load_document') return args.key === 'presets' ? { 'Station A': preset } : null
  if (command === 'simlab_save_document') { writes.push(args); return }
 } } as any
 const storage = await import('../../frontend-sub/src/suiteWorkspace')
 await storage.restoreDocuments('pmu.sub')
 const { default: ConfigFormPanel } = await import('../../frontend-sub/src/components/ConfigFormPanel.vue')
 const wrapper = mount(ConfigFormPanel)
 try {
  await flushPromises()
  const controls = wrapper.findAll('.preset-load').at(-1)!
  await controls.get('select').setValue('Station A')
  await controls.findAll('button')[0]!.trigger('click'); await flushPromises()
  await wrapper.findAll('.phasor-row input')[0]!.setValue(456); await flushPromises()
  await storage.flushDocuments()
  expect((storage.documentValue<Record<string, typeof preset>>('presets')['Station A'])!.phasors[0].magnitude).toBe(123)
  expect(writes.filter(write => write.key === 'presets')).toHaveLength(0)
  expect((storage.documentValue<typeof preset>('workspace').phasors)![0].magnitude).toBe(456)
 } finally { wrapper.unmount() }
})
