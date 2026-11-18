import { afterEach, describe, expect, it, vi } from 'vitest'
import { shallowMount, flushPromises } from '@vue/test-utils'
import App from '../src/App.vue'
import UpdateDialog from '@shared/components/UpdateDialog.vue'
import AboutDialog from '@shared/components/AboutDialog.vue'

const invokeMock = vi.fn((..._args: unknown[]) => Promise.resolve(null))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }))

describe.skipIf(import.meta.env.VITE_SIMLAB_SUITE !== '1')('suite update ownership', () => {
  afterEach(() => { vi.useRealTimers(); invokeMock.mockClear() })

  it('does not offer standalone release downloads in About', () => {
    const wrapper = shallowMount(AboutDialog, { props: { visible: true } })
    expect(wrapper.find('[data-testid=release-history-link]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('never invokes component updates on startup or manual requests', async () => {
    vi.useFakeTimers()
    const wrapper = shallowMount(App)
    await flushPromises()
    await vi.advanceTimersByTimeAsync(3000)
    const vm = wrapper.vm as unknown as { checkUpdate: (force: boolean) => Promise<unknown> }
    expect(await vm.checkUpdate(true)).toBeNull()
    expect(invokeMock.mock.calls.some(([command]) => command === 'check_for_update')).toBe(false)
    expect(wrapper.findComponent(UpdateDialog).exists()).toBe(false)
    wrapper.unmount()
  })
})
