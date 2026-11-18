import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { ref } from 'vue'
import { dialogKey } from '@shared/composables/useDialog'
import LogPanel from '../src/components/LogPanel.vue'

const invokeMock = vi.fn()
const saveMock = vi.fn()
const showAlertMock = vi.fn(() => Promise.resolve())

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: (...args: unknown[]) => saveMock(...args),
}))

function mountPanel(serverId: string | null = 'server-1') {
  return mount(LogPanel, {
    props: { expanded: false },
    global: {
      provide: {
        selectedServerId: ref(serverId),
        openParseFrame: vi.fn(),
        [dialogKey as symbol]: { showAlert: showAlertMock },
      },
    },
  })
}

describe('LogPanel CSV export', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    saveMock.mockReset()
    showAlertMock.mockClear()
  })

  it('uses the native save dialog and backend file writer even while logs are collapsed', async () => {
    saveMock.mockResolvedValue('C:\\Temp\\iec104.csv')
    invokeMock.mockResolvedValue(undefined)
    const wrapper = mountPanel()

    await wrapper.findAll('.log-btn')[2].trigger('click')
    await flushPromises()

    expect(saveMock).toHaveBeenCalledWith(expect.objectContaining({
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    }))
    expect(invokeMock).toHaveBeenCalledWith('save_logs_csv', {
      serverId: 'server-1',
      path: 'C:\\Temp\\iec104.csv',
    })
    wrapper.unmount()
  })

  it('does not invoke the backend when the save dialog is cancelled', async () => {
    saveMock.mockResolvedValue(null)
    const wrapper = mountPanel()

    await wrapper.findAll('.log-btn')[2].trigger('click')
    await flushPromises()

    expect(invokeMock).not.toHaveBeenCalled()
    wrapper.unmount()
  })
})
