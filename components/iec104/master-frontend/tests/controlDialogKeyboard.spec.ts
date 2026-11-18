import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
import ControlDialog from '../src/components/ControlDialog.vue'
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async (command: string) => command === 'list_connections' ? [] : { steps: [], duration_ms: 1 }) }))
let wrapper: ReturnType<typeof mount> | undefined
afterEach(() => { wrapper?.unmount(); vi.clearAllMocks() })
describe('control dialog keyboard safety', () => {
  it('does not send when Enter activates Close or the advanced summary', async () => {
    wrapper = mount(ControlDialog, { props: { visible: true, connectionId: 'test', commonAddress: 1 }, global: { stubs: { Teleport: true } } })
    await wrapper.get('.modal-footer button').trigger('keydown', { key: 'Enter' })
    await wrapper.get('summary').trigger('keydown', { key: 'Enter' })
    await flushPromises()
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'send_control_command')).toHaveLength(0)
    await wrapper.get('.modal-footer button').trigger('click')
    expect(wrapper.emitted('close')).toHaveLength(1)
  })
})
