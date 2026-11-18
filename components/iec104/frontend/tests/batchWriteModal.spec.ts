// slave-batch-write-by-ioa：BatchWriteModal 命中/忽略/禁用/写入/键盘。
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, flushPromises, VueWrapper } from '@vue/test-utils'
import { dialogKey } from '@shared/composables/useDialog'
import BatchWriteModal from '../src/components/BatchWriteModal.vue'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }))

const existing = [
  { ioa: 100, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 1000, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 1500, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 2000, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 5, asdu_type: 'M_SP_NA_1', category: '单点信息' },
]

let w: VueWrapper | null = null
function mountModal(): VueWrapper {
  w = mount(BatchWriteModal, {
    props: { visible: true, serverId: 's1', commonAddress: 1, existingPoints: existing, defaultType: 'M_ME_NC_1' },
    global: {
      stubs: { teleport: true },
      provide: { [dialogKey as symbol]: { showAlert: () => Promise.resolve() } },
    },
  })
  return w
}
const writeBtn = (ww: VueWrapper) => ww.find('.btn-primary').element as HTMLButtonElement

describe('BatchWriteModal', () => {
  beforeEach(() => invokeMock.mockReset())
  afterEach(() => {
    w?.unmount()
    w = null
  })

  it('区间命中：1000-2000 命中 3 点，命中区间文本正确', async () => {
    const ww = mountModal()
    await ww.find('.ioa-textarea').setValue('1000-2000')
    expect(ww.find('.summary-card__ranges-value').text()).toBe('1000, 1500, 2000')
    expect(ww.find('.summary-card__conflict').exists()).toBe(false)
  })

  it('单点缺失：100, 999 → 命中 100、忽略 999', async () => {
    const ww = mountModal()
    await ww.find('.ioa-textarea').setValue('100, 999')
    expect(ww.find('.summary-card__ranges-value').text()).toBe('100')
    expect(ww.find('.summary-card__conflict').text()).toContain('999')
  })

  it('语法错：abc → 显示 parseError、写入禁用', async () => {
    const ww = mountModal()
    await ww.find('.ioa-textarea').setValue('abc')
    expect(ww.find('.summary-card__conflict.no-border').exists()).toBe(true)
    await ww.find('input[type="text"]').setValue('99.9')
    expect(writeBtn(ww).disabled).toBe(true)
  })

  it('0 命中 / 空值 → 写入禁用', async () => {
    const ww = mountModal()
    await ww.find('.ioa-textarea').setValue('1000-2000')
    expect(writeBtn(ww).disabled).toBe(true)
    await ww.find('input[type="text"]').setValue('99.9')
    await ww.find('.ioa-textarea').setValue('99999')
    expect(writeBtn(ww).disabled).toBe(true)
  })

  it('正常写入：点击触发 batch_update_data_points 并带显式 points + 值', async () => {
    invokeMock.mockResolvedValue(3)
    const ww = mountModal()
    await ww.find('.ioa-textarea').setValue('1000-2000')
    await ww.find('input[type="text"]').setValue('99.9')
    expect(writeBtn(ww).disabled).toBe(false)
    await ww.find('.btn-primary').trigger('click')
    expect(invokeMock).toHaveBeenCalledWith('batch_update_data_points', {
      serverId: 's1',
      commonAddress: 1,
      value: '99.9',
      points: [
        { ioa: 1000, asdu_type: 'M_ME_NC_1' },
        { ioa: 1500, asdu_type: 'M_ME_NC_1' },
        { ioa: 2000, asdu_type: 'M_ME_NC_1' },
      ],
    })
    expect(ww.emitted('written')).toBeTruthy()
  })

  it('键盘：Esc 触发 close；Cmd+Enter 在可写时触发写入', async () => {
    invokeMock.mockResolvedValue(3)
    const ww = mountModal()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    expect(ww.emitted('close')).toBeTruthy()
    await ww.find('.ioa-textarea').setValue('1000-2000')
    await ww.find('input[type="text"]').setValue('99.9')
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', metaKey: true }))
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledWith('batch_update_data_points', expect.objectContaining({ value: '99.9' }))
  })
})
