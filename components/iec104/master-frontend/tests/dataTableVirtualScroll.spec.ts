// 回归:在大列表(如 CA1 单点 5278 条)里向下滚动后,切到小列表(CA2 单点 21 条),
// 残留的 scrollTop 让虚拟滚动 visibleStart 越过新列表末尾,slice 返回空 → 表格全空白,
// 而 filteredPoints.length>0 又抑制了"暂无数据"提示。复现"计数 21 / 18095 但表格空白"。
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { ref, nextTick, type Ref } from 'vue'
import DataTable from '../src/components/DataTable.vue'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }))

function pt(ioa: number, value: string, ca: number) {
  return { ioa, asdu_type: 'M_SP_NA', value, category: '单点', common_address: ca, quality_ov: false, quality_bl: false, quality_sb: false, quality_nt: false, quality_iv: false, timestamp: null }
}

function provideRefs() {
  return {
    selectedConnectionId: ref<string | null>('conn-1') as Ref<string | null>,
    selectedCA: ref<number | null>(null) as Ref<number | null>,
    selectedCategory: ref<string | null>(null) as Ref<string | null>,
    dataRefreshKey: ref(0),
    changedCategories: ref(new Map()),
    categoryCounts: ref(new Map()),
  }
}

describe('DataTable 虚拟滚动:大列表滚动后切到小列表', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    vi.useFakeTimers()
  })

  it('切到点数更少的 CA/分类后,残留 scrollTop 不应让表格空白', async () => {
    // CA1 单点 50 条(够触发滚动越界),CA2 单点 3 条
    const points = [
      ...Array.from({ length: 50 }, (_, i) => pt(i + 1, 'on', 1)),
      pt(1, 'on', 2), pt(2, 'off', 2), pt(3, 'on', 2),
    ]
    invokeMock.mockResolvedValueOnce({ points, seq: 1 }).mockResolvedValue({ points: [], seq: 1 })

    const provide = provideRefs()
    const wrapper = mount(DataTable, { global: { provide } })
    await flushPromises()
    await nextTick()

    const vm = wrapper.vm as unknown as { filteredPoints: unknown[]; visibleRows: unknown[] }

    // 看 CA1 单点(50 条),模拟用户向下滚动:scrollTop=1000(>868 阈值)
    provide.selectedCA.value = 1
    provide.selectedCategory.value = '单点'
    await nextTick()
    expect(vm.filteredPoints.length).toBe(50)

    const scrollEl = wrapper.find('.table-scroll').element
    Object.defineProperty(scrollEl, 'scrollTop', { value: 1000, configurable: true, writable: true })
    Object.defineProperty(scrollEl, 'clientHeight', { value: 400, configurable: true, writable: true })
    await wrapper.find('.table-scroll').trigger('scroll')
    await nextTick()
    expect(vm.visibleRows.length).toBeGreaterThan(0) // 大列表仍渲染

    // 切到 CA2 单点(只有 3 条)
    provide.selectedCA.value = 2
    await nextTick()
    expect(vm.filteredPoints.length).toBe(3) // 计数正确
    expect(vm.visibleRows.length).toBe(3)     // 表格必须真的渲染出来,而非空白

    wrapper.unmount()
  })
})
