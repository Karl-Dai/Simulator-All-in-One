import { afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import NewConnectionDialog from '../../master-frontend/src/components/NewConnectionDialog.vue'
import { useI18n } from '../src/i18n'

vi.mock('../../master-frontend/wailsjs/go/main/App', () => ({ ListSerialPorts: vi.fn().mockResolvedValue([]), CreateConnection: vi.fn(), GetSettings: vi.fn(), UpdateConnection: vi.fn(), OpenFileDialog: vi.fn() }))
let wrapper: ReturnType<typeof mount> | undefined
afterEach(() => { wrapper?.unmount(); document.body.innerHTML = ''; vi.clearAllMocks() })
it('does not dismiss a form when text selection ends on the backdrop', async () => {
 useI18n().setLocale('zh-CN')
 wrapper = mount(NewConnectionDialog, { attachTo: document.body, props: { show: true } })
 await flushPromises()
 const field = document.querySelector<HTMLInputElement>('input[placeholder="127.0.0.1"]')!
 field.value = '10.15.205.21'; field.dispatchEvent(new Event('input', { bubbles: true }))
 field.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }))
 const backdrop = document.querySelector('.modal-backdrop')!
 backdrop.dispatchEvent(new MouseEvent('pointerup', { bubbles: true, button: 0 }))
 backdrop.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }))
 await flushPromises()
 expect(wrapper.emitted('close')).toBeUndefined()
 expect(field.value).toBe('10.15.205.21')
 // A deliberate primary-button press outside still closes; right-click does not.
 backdrop.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 2 }))
 expect(wrapper.emitted('close')).toBeUndefined()
 backdrop.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }))
 expect(wrapper.emitted('close')).toHaveLength(1)
 await wrapper.setProps({ show: false }); await wrapper.setProps({ show: true })
 await flushPromises()
 expect(document.querySelector<HTMLInputElement>('input[placeholder="127.0.0.1"]')!.value).toBe('127.0.0.1')
})
