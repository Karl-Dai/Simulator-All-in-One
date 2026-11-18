// slave-batch-edit-points 验证项 7.2:ValuePanel 多选区批量编辑。
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import { dialogKey } from '@shared/composables/useDialog'
import ValuePanel from '../src/components/ValuePanel.vue'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }))

// category 为后端 DataPointInfo 必带的稳定分类键,allSameCategory 直接比较它
type Pt = { ioa: number; asdu_type: string; category: string; value: string }
function mountPanel(points: Pt[]) {
  const selectedPoints = ref<Pt[]>(points)
  const w = mount(ValuePanel, {
    global: {
      provide: {
        selectedServerId: ref<string | null>('s1'),
        selectedCA: ref<number | null>(1),
        selectedPoints,
        [dialogKey as symbol]: { showAlert: () => Promise.resolve() },
      },
    },
  })
  return { w }
}

describe('ValuePanel 批量编辑', () => {
  beforeEach(() => invokeMock.mockReset())

  it('多选同类(SP):写值输入可用、无 OV 徽章、无混类提示', () => {
    const { w } = mountPanel([
      { ioa: 1, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
      { ioa: 2, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
    ])
    expect((w.find('.write-input').element as HTMLInputElement).disabled).toBe(false)
    expect(w.findAll('.q-badge').map((b) => b.text())).not.toContain('OV')
    expect(w.find('.batch-hint').exists()).toBe(false)
  })

  it('多选混类:写值禁用 + 显示提示', () => {
    const { w } = mountPanel([
      { ioa: 1, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
      { ioa: 2, asdu_type: 'M_ME_NC_1', category: 'float_measured', value: '0' },
    ])
    expect((w.find('.write-input').element as HTMLInputElement).disabled).toBe(true)
    expect(w.find('.batch-hint').exists()).toBe(true)
  })

  it('全测量类:显示 OV 徽章', () => {
    const { w } = mountPanel([
      { ioa: 1, asdu_type: 'M_ME_NC_1', category: 'float_measured', value: '0' },
      { ioa: 2, asdu_type: 'M_ME_NC_1', category: 'float_measured', value: '0' },
    ])
    expect(w.findAll('.q-badge').map((b) => b.text())).toContain('OV')
  })

  it('应用品质:勾 NT 后点击触发 batch_set_data_point_quality', async () => {
    invokeMock.mockResolvedValue(2)
    const { w } = mountPanel([
      { ioa: 1, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
      { ioa: 2, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
    ])
    await w.findAll('.q-badge').find((b) => b.text() === 'NT')!.trigger('click')
    await w.findAll('.write-btn')[0].trigger('click') // 第一个 write-btn = 应用品质
    expect(invokeMock).toHaveBeenCalledWith(
      'batch_set_data_point_quality',
      expect.objectContaining({
        serverId: 's1',
        commonAddress: 1,
        nt: true,
        points: [
          { ioa: 1, asdu_type: 'M_SP_NA_1' },
          { ioa: 2, asdu_type: 'M_SP_NA_1' },
        ],
      }),
    )
  })

  it('应用值:同类时点击触发 batch_update_data_points', async () => {
    invokeMock.mockResolvedValue(2)
    const { w } = mountPanel([
      { ioa: 1, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
      { ioa: 2, asdu_type: 'M_SP_NA_1', category: 'single_point', value: 'OFF' },
    ])
    await w.find('.write-input').setValue('ON')
    await w.findAll('.write-btn')[1].trigger('click') // 第二个 write-btn = 应用值
    expect(invokeMock).toHaveBeenCalledWith(
      'batch_update_data_points',
      expect.objectContaining({ serverId: 's1', commonAddress: 1, value: 'ON' }),
    )
  })
})
