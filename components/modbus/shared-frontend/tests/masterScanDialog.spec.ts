import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { ref } from 'vue'
import ScanDialog from '../../master-frontend/src/components/ScanDialog.vue'
import * as App from '../../master-frontend/wailsjs/go/main/App'
import { useI18n } from '../src/i18n'
const listeners = vi.hoisted(() => new Map<string, (event: unknown) => void>())
vi.mock('../../master-frontend/wailsjs/runtime/runtime', () => ({ EventsOn: vi.fn((name, callback) => { listeners.set(name, callback); return () => listeners.delete(name) }) }))
vi.mock('../../master-frontend/wailsjs/go/main/App', () => ({ GetSettings: vi.fn(), StartSlaveIDScan: vi.fn(), StartRegisterScan: vi.fn(), CancelScan: vi.fn(), AddScanGroup: vi.fn(), ListScanGroups: vi.fn() }))
let wrapper: ReturnType<typeof mount>
beforeEach(() => {
 useI18n().setLocale('zh-CN')
 vi.mocked(App.GetSettings).mockResolvedValue({ SlaveID: 1 } as never)
 vi.mocked(App.StartSlaveIDScan).mockResolvedValue('ids-1')
 vi.mocked(App.StartRegisterScan).mockResolvedValue('regs-1')
 vi.mocked(App.ListScanGroups).mockResolvedValue([])
 vi.mocked(App.CancelScan).mockResolvedValue(undefined)
})
afterEach(() => { wrapper?.unmount(); vi.resetAllMocks(); listeners.clear(); document.body.innerHTML = '' })
async function open(slaveID?: number) {
 wrapper = mount(ScanDialog, { attachTo: document.body, global: { provide: { selectedConnectionId: ref('c1'), selectedScanGroup: ref(slaveID == null ? null : { SlaveID: slaveID, Function: 'read_holding_registers', StartAddress: 0, Quantity: 20 }), refreshTree: vi.fn() }, stubs: { teleport: true } } })
 await flushPromises(); return wrapper
}
function button(label: string) { return wrapper.findAll('button').find(button => button.text() === label)! }
function emitProgress(kind: 'slave' | 'register', overrides: Record<string, unknown> = {}) {
 const payload = kind === 'slave' ? { ConnectionID: 'c1', TaskID: 'ids-1', CurrentID: 8, Completed: 8, Total: 8, FoundIDs: [1, 2, 7], Done: true, Cancelled: false }
  : { ConnectionID: 'c1', TaskID: 'regs-1', CurrentAddress: 19, EndAddress: 19, Completed: 20, Total: 20, FoundCount: 4, FoundRegisters: [{Address: 2, Value: 7002}, {Address: 3, Value: 7003}, {Address: 10, Value: 7010}, {Address: 11, Value: 7011}], Done: true, Cancelled: false }
 listeners.get(`scan-${kind}-progress`)?.(JSON.parse(JSON.stringify({ ...payload, ...overrides })))
}
it('discovers numeric IDs without creating unverified groups and scans the chosen slave', async () => {
 await open(); await button('开始扫描').trigger('click'); await flushPromises()
 emitProgress('slave'); await flushPromises()
 expect(wrapper.findAll('.result-table tbody tr')).toHaveLength(3)
 expect(App.AddScanGroup).not.toHaveBeenCalled()
 await wrapper.findAll('.result-table tbody tr')[2]!.find('button').trigger('click')
 expect(wrapper.get<HTMLInputElement>('#scan-slave-id').element.value).toBe('7')
 await button('开始扫描').trigger('click'); await flushPromises()
 expect(App.StartRegisterScan).toHaveBeenCalledWith('c1', expect.objectContaining({ SlaveID: 7 }))
})
it('keeps the scanned target when the form changes and skips already-added ranges', async () => {
 await open(7); await button('寄存器扫描').trigger('click'); await button('开始扫描').trigger('click'); await flushPromises()
 emitProgress('register'); await flushPromises()
 await wrapper.get('#scan-slave-id').setValue(9)
 const created: any[] = []
 vi.mocked(App.AddScanGroup).mockImplementation(async (_id, group) => { const result = {...group, ID: `g${created.length}`}; created.push(result); return result })
 vi.mocked(App.ListScanGroups).mockImplementation(async () => [...created])
 await button('将结果添加为轮询组').trigger('click'); await flushPromises()
 expect(created.map(g => [g.SlaveID, g.StartAddress, g.Quantity])).toEqual([[7,2,2],[7,10,2]])
 await button('将结果添加为轮询组').trigger('click'); await flushPromises()
 expect(App.AddScanGroup).toHaveBeenCalledTimes(2)
})
it('accepts completion that arrives before the start call returns', async () => {
 vi.mocked(App.StartSlaveIDScan).mockImplementation(async () => { emitProgress('slave'); return 'ids-1' })
 await open(); await button('开始扫描').trigger('click'); await flushPromises()
 expect(wrapper.findAll('.result-table tbody tr')).toHaveLength(3)
 expect(button('开始扫描').attributes('disabled')).toBeUndefined()
})
it('cancels a pending start after its task has been registered', async () => {
 let resolve!: (id: string) => void
 vi.mocked(App.StartSlaveIDScan).mockReturnValue(new Promise(done => { resolve = done }))
 vi.mocked(App.CancelScan).mockImplementation(async () => { emitProgress('slave', { Done: false, Cancelled: true }); })
 await open(); await button('开始扫描').trigger('click'); await button('取消').trigger('click')
 expect(App.CancelScan).not.toHaveBeenCalled()
 resolve('ids-1'); await flushPromises()
 expect(App.CancelScan).toHaveBeenCalledWith('c1', 'slave_scan')
 expect(wrapper.text()).toContain('已中止')
})
it('ignores other tasks and connections and shows terminal errors without adding groups', async () => {
 await open(7); await button('寄存器扫描').trigger('click'); await button('开始扫描').trigger('click'); await flushPromises()
 emitProgress('register', { ConnectionID: 'other' }); emitProgress('register', { TaskID: 'old' }); await flushPromises()
 expect(wrapper.findAll('.result-table tbody tr')).toHaveLength(0)
 emitProgress('register', { Error: 'Modbus exception: 0x01' }); await flushPromises()
 expect(wrapper.get('[role="alert"]').text()).toContain('0x01')
 expect(App.AddScanGroup).not.toHaveBeenCalled()
})
it('uses completed counts for non-one-based ID ranges and paginates large results', async () => {
 await open(); await button('开始扫描').trigger('click'); await flushPromises()
 emitProgress('slave', { CurrentID: 102, Completed: 3, Total: 3 }); await flushPromises()
 expect(wrapper.get('.progress-text').text()).toContain('3 / 3')
 await button('寄存器扫描').trigger('click'); await button('开始扫描').trigger('click'); await flushPromises()
 emitProgress('register', { FoundCount: 2000, FoundRegisters: Array.from({length:2000}, (_, Address) => ({Address, Value: Address})) }); await flushPromises()
 expect(wrapper.findAll('.result-table tbody tr')).toHaveLength(100)
 await button('下一页').trigger('click')
 expect(wrapper.find('.result-table tbody td').text()).toBe('100')
})
