import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { nextTick, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '@shared/i18n'
import ControlDialog from '../src/components/ControlDialog.vue'
import ValuePanel from '../src/components/ValuePanel.vue'
import type { ControlResult, ReceivedDataPointInfo } from '../src/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

function result(action: string, duration = 651): ControlResult {
  return { steps: [{ action, timestamp: '12:00:00.651' }], duration_ms: duration }
}

const wrappers: VueWrapper[] = []
let response: Promise<ControlResult>

async function dialog() {
  const wrapper = mount(ControlDialog, {
    props: { visible: false, connectionId: 'conn-1', commonAddress: 1 },
    global: { stubs: { Teleport: true, Transition: false } },
  })
  wrappers.push(wrapper)
  await wrapper.setProps({ visible: true })
  await flushPromises()
  return wrapper
}

function sendButton(wrapper: VueWrapper) {
  return wrapper.find('.modal-footer button.app-btn-primary')
}

function controlCalls() {
  return vi.mocked(invoke).mock.calls.filter(([command]) => command === 'send_control_command')
}

beforeEach(() => {
  vi.mocked(invoke).mockReset()
  localStorage.clear()
  useI18n().setLocale('en-US')
  response = Promise.resolve(result('execute_confirmed'))
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === 'list_connections') return Promise.resolve([{ id: 'conn-1', common_addresses: [1] }]) as never
    if (command === 'send_control_command') return response as never
    throw new Error(`Unexpected command ${command}`)
  })
})

afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount()
})

describe('ControlDialog protocol results', () => {
  it('waits for confirmation, prevents repeat submits, and shows the confirmed duration', async () => {
    const pending = deferred<ControlResult>()
    response = pending.promise
    const wrapper = await dialog()
    await sendButton(wrapper).trigger('click')
    expect(wrapper.find('.result-pending').text()).toBe('Waiting for confirmation…')
    expect(sendButton(wrapper).attributes('disabled')).toBeDefined()
    expect(wrapper.find('fieldset').attributes('disabled')).toBeDefined()
    expect(wrapper.find('.result-ok').exists()).toBe(false)
    await wrapper.find('input').trigger('keydown', { key: 'Enter' })
    expect(controlCalls()).toHaveLength(1)

    pending.resolve(result('execute_confirmed', 651))
    await flushPromises()
    expect(wrapper.find('.result-text').text()).toBe('Execution confirmed · confirmation 651 ms')
    expect(wrapper.find('.result-pending').exists()).toBe(false)
    expect(sendButton(wrapper).attributes('disabled')).toBeUndefined()
    expect(wrapper.text()).not.toContain('OK')
  })

  it.each(['execute_sent', 'select_sent', 'deactivate_sent'])('never presents %s as confirmed', async (action) => {
    response = Promise.resolve(result(action, 0))
    const wrapper = await dialog()
    await sendButton(wrapper).trigger('click')
    await flushPromises()
    expect(wrapper.find('.result-unconfirmed').text()).toContain('Sent; confirmation not awaited · send 0 ms')
    expect(wrapper.find('.result-ok').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('OK')
  })

  it('does not treat an empty or unrecognized result as successful confirmation', async () => {
    response = Promise.resolve({ steps: [], duration_ms: 0 })
    const wrapper = await dialog()
    await sendButton(wrapper).trigger('click')
    await flushPromises()
    expect(wrapper.find('.result-unconfirmed').text()).toContain('Confirmation not verified')
    expect(wrapper.find('.result-ok').exists()).toBe(false)
  })

  it.each(['Control confirmation rejected (COT=7)', 'Control confirmation timed out (5s)'])('shows %s without a success result', async (error) => {
    const pending = deferred<ControlResult>()
    response = pending.promise
    const wrapper = await dialog()
    await sendButton(wrapper).trigger('click')
    pending.reject(error)
    await flushPromises()
    expect(wrapper.find('[role="alert"]').text()).toBe(error)
    expect(wrapper.find('.result-indicator').exists()).toBe(false)
    expect(sendButton(wrapper).attributes('disabled')).toBeUndefined()
  })

  it('reports selection confirmation without implying that execution followed', async () => {
    response = Promise.resolve(result('select_confirmed', 21))
    const wrapper = await dialog()
    await wrapper.find('.mode-select select').setValue('select')
    await sendButton(wrapper).trigger('click')
    await flushPromises()
    expect(controlCalls()[0]?.[1]).toMatchObject({ request: { control_mode: 'select' } })
    expect(wrapper.find('.result-text').text()).toBe('Selection confirmed · confirmation 21 ms')
    expect(wrapper.find('.result-text').text()).not.toContain('Execution')
    useI18n().setLocale('zh-CN')
    await nextTick()
    expect(wrapper.find('.result-text').text()).toBe('选择已确认 · 确认耗时 21 毫秒')
  })

  it('reports a deactivation confirmation separately', async () => {
    response = Promise.resolve(result('deactivate_confirmed', 32))
    const wrapper = await dialog()
    await wrapper.find('.cot-input').setValue(8)
    await sendButton(wrapper).trigger('click')
    await flushPromises()
    expect(wrapper.find('.result-text').text()).toBe('Deactivation confirmed · confirmation 32 ms')
  })

  it('shows sending rather than waiting for a custom non-request COT', async () => {
    const pending = deferred<ControlResult>()
    response = pending.promise
    const wrapper = await dialog()
    await wrapper.find('.cot-input').setValue(7)
    await sendButton(wrapper).trigger('click')
    expect(wrapper.find('.result-pending').text()).toBe('Sending...')
    pending.resolve(result('execute_sent', 0))
    await flushPromises()
    expect(wrapper.find('.result-unconfirmed').exists()).toBe(true)
  })

  it('does not submit when Enter is used on a native mode select', async () => {
    const wrapper = await dialog()
    await wrapper.find('.mode-select select').trigger('keydown', { key: 'Enter' })
    expect(controlCalls()).toHaveLength(0)
  })

  it('ignores a late result after closing and reopening while preserving the new pending request', async () => {
    const old = deferred<ControlResult>()
    response = old.promise
    const wrapper = await dialog()
    await sendButton(wrapper).trigger('click')
    await wrapper.find('.modal-footer button').trigger('click')
    await wrapper.setProps({ visible: false })
    await wrapper.setProps({ visible: true })
    const current = deferred<ControlResult>()
    response = current.promise
    await sendButton(wrapper).trigger('click')
    old.resolve(result('execute_confirmed', 100))
    await flushPromises()
    expect(wrapper.find('.result-ok').exists()).toBe(false)
    expect(wrapper.find('.result-pending').exists()).toBe(true)
    expect(sendButton(wrapper).attributes('disabled')).toBeDefined()
    expect(wrapper.emitted('sent')).toBeUndefined()
    current.resolve(result('execute_confirmed', 700))
    await flushPromises()
    expect(wrapper.find('.result-text').text()).toContain('700 ms')
    expect(wrapper.emitted('sent')).toHaveLength(1)
  })

  it('ignores a late error after the selected connection changes', async () => {
    const old = deferred<ControlResult>()
    response = old.promise
    const wrapper = await dialog()
    await sendButton(wrapper).trigger('click')
    await wrapper.setProps({ connectionId: 'conn-2' })
    old.reject('old connection timed out')
    await flushPromises()
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    expect(wrapper.find('.result-indicator').exists()).toBe(false)
    expect(sendButton(wrapper).attributes('disabled')).toBeUndefined()
  })
})

describe('ValuePanel protocol results', () => {
  function panel() {
    const selectedConnectionId = ref('conn-1')
    const selectedPoints = ref([{
      ioa: 25089, common_address: 1, category: 'float_measured', value: '12.5',
      asdu_type: 'M_ME_NC_1', quality: { invalid: false, not_topical: false, substituted: false, blocked: false, overflow: false },
    } as ReceivedDataPointInfo])
    const wrapper = mount(ValuePanel, {
      global: { provide: { selectedConnectionId, selectedPoints } },
    })
    wrappers.push(wrapper)
    return { wrapper, selectedConnectionId, selectedPoints }
  }

  it('uses the same confirmation status and measured duration for quick control', async () => {
    const pending = deferred<ControlResult>()
    response = pending.promise
    const { wrapper } = panel()
    await wrapper.find('.ctrl-btn-primary').trigger('click')
    expect(wrapper.find('[role="status"]').text()).toBe('Waiting for confirmation…')
    pending.resolve(result('execute_confirmed', 655))
    await flushPromises()
    expect(wrapper.find('.result-text').text()).toBe('Execution confirmed · confirmation 655 ms')
  })

  it('does not render a sent-only quick control result as successful confirmation', async () => {
    response = Promise.resolve(result('execute_sent', 0))
    const { wrapper } = panel()
    await wrapper.find('.ctrl-btn-primary').trigger('click')
    await flushPromises()
    expect(wrapper.find('.result-unconfirmed').text()).toContain('Sent; confirmation not awaited')
    expect(wrapper.find('.result-ok').exists()).toBe(false)
  })

  it('discards a result from the previously selected point', async () => {
    const pending = deferred<ControlResult>()
    response = pending.promise
    const { wrapper, selectedPoints } = panel()
    await wrapper.find('.ctrl-btn-primary').trigger('click')
    selectedPoints.value = [{ ...selectedPoints.value[0]!, common_address: 2 }]
    await nextTick()
    pending.resolve(result('execute_confirmed'))
    await flushPromises()
    expect(wrapper.find('.result-indicator').exists()).toBe(false)
  })
})
