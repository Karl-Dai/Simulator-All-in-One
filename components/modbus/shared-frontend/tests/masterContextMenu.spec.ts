import { afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { ref } from 'vue'
import { useI18n } from '../src/i18n'
import ConnectionTree from '../../master-frontend/src/components/ConnectionTree.vue'
import NewScanGroupDialog from '../../master-frontend/src/components/NewScanGroupDialog.vue'
import * as App from '../../master-frontend/wailsjs/go/main/App'

vi.mock('../../master-frontend/wailsjs/go/main/App', () => ({
 ListConnections: vi.fn(), ListScanGroups: vi.fn(), UpdateScanGroup: vi.fn(), AddScanGroup: vi.fn(), Disconnect: vi.fn(),
}))
let wrapper: ReturnType<typeof mount> | undefined
const group = { ID: 'g2', Name: 'second', Function: 'read_holding_registers', StartAddress: 0, Quantity: 5, IntervalMS: 1000, Enabled: true, IsPolling: true, SlaveID: 7 }
afterEach(() => { wrapper?.unmount(); vi.resetAllMocks(); document.body.innerHTML = '' })
async function tree(state = 'Disconnected') {
 useI18n().setLocale('zh-CN')
 vi.mocked(App.ListConnections).mockResolvedValue(['c1', 'c2'].map(ID => ({ ID, State: state, TargetAddress: ID, Port: 502, SlaveID: 1, ScanGroupCount: 1 })))
 vi.mocked(App.ListScanGroups).mockResolvedValue([group])
 wrapper = mount(ConnectionTree, { attachTo: document.body, global: {
  provide: { treeRefreshKey: ref(0), refreshTree: vi.fn() },
  stubs: { NewConnectionDialog: { props: ['connectionId'], template: '<div data-test="edit-connection">{{ connectionId }}</div>' }, NewScanGroupDialog: { props: ['connectionId', 'group'], template: '<div data-test="edit-poll">{{ connectionId }} {{ group.ID }}</div>' } },
 } })
 await flushPromises()
 return wrapper
}
it('right click selects and edits the clicked connection, not the previous selection', async () => {
 const w = await tree()
 await w.findAll('.tree-node')[0]!.trigger('click')
 await w.findAll('.tree-node')[2]!.trigger('contextmenu', { clientX: 100, clientY: 100 })
 expect(w.emitted('connection-select')!.at(-1)).toEqual(['c2', 'Disconnected'])
 await w.find('button[role="menuitem"]').trigger('click')
 expect(w.find('[data-test="edit-connection"]').text()).toBe('c2')
 expect(w.find('[role="menu"]').exists()).toBe(false)
})
it('offers disconnect before editing connected parameters', async () => {
 const w = await tree('Connected')
 await w.findAll('.tree-node')[2]!.trigger('contextmenu')
 const buttons = w.findAll('button[role="menuitem"]')
 expect(buttons[0]!.attributes('disabled')).toBeDefined()
 await buttons[1]!.trigger('click')
 expect(App.Disconnect).toHaveBeenCalledWith('c2')
})
it('edits the right-clicked polling group and dismisses menus with Escape', async () => {
 const w = await tree('Connected')
 await w.findAll('.tree-child')[1]!.trigger('contextmenu')
 await w.find('button[role="menuitem"]').trigger('click')
 expect(w.find('[data-test="edit-poll"]').text()).toBe('c2 g2')
 await w.findAll('.tree-child')[1]!.trigger('contextmenu')
 document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
 await flushPromises()
 expect(w.find('[role="menu"]').exists()).toBe(false)
})
it('updates an existing group while preserving its identity, slave ID and enabled state', async () => {
 useI18n().setLocale('zh-CN')
 wrapper = mount(NewScanGroupDialog, { attachTo: document.body, props: { show: true, connectionId: 'c2', group } })
 await flushPromises()
 expect(document.body.textContent).toContain('数量')
  expect(document.body.textContent).not.toContain('table.Quantity')
  const quantity = document.querySelector<HTMLInputElement>('#poll-quantity')!
 expect(quantity.value).toBe('5')
 quantity.value = '6'; quantity.dispatchEvent(new Event('input', { bubbles: true }))
 await flushPromises()
 ;[...document.querySelectorAll<HTMLButtonElement>('button')].find(b => b.textContent === '保存')!.click()
 await flushPromises()
 expect(App.UpdateScanGroup).toHaveBeenCalledWith('c2', { ...group, Quantity: 6 })
 expect(App.AddScanGroup).not.toHaveBeenCalled()
 expect(group.Quantity).toBe(5)
 expect(wrapper.emitted('saved')?.[0]).toEqual([{ ...group, Quantity: 6 }])
})

it('creates different slave targets on one connection and can restore default inheritance', async () => {
 useI18n().setLocale('zh-CN')
 wrapper = mount(NewScanGroupDialog, { attachTo: document.body, props: { show: true, connectionId: 'c2' } })
 await flushPromises()
 const mode = document.querySelector<HTMLSelectElement>('#poll-slave-mode')!
 mode.selectedIndex = 1; mode.dispatchEvent(new Event('change', { bubbles: true }))
 await flushPromises()
 const input = document.querySelector<HTMLInputElement>('#poll-slave-id')!
 input.value = '7'; input.dispatchEvent(new Event('input', { bubbles: true }))
 await flushPromises()
 document.querySelector<HTMLFormElement>('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
 await flushPromises()
 expect(App.AddScanGroup).toHaveBeenCalledWith('c2', expect.objectContaining({ SlaveID: 7 }))
 await wrapper.setProps({ show: false })
 await wrapper.setProps({ show: true, group })
 await flushPromises()
 expect(document.querySelector<HTMLInputElement>('#poll-slave-id')!.value).toBe('7')
 const reset = document.querySelector<HTMLSelectElement>('#poll-slave-mode')!
 reset.selectedIndex = 0; reset.dispatchEvent(new Event('change', { bubbles: true }))
 await flushPromises()
 document.querySelector<HTMLFormElement>('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
 await flushPromises()
 expect(App.UpdateScanGroup).toHaveBeenCalledWith('c2', expect.objectContaining({ ID: 'g2', SlaveID: undefined }))
})
