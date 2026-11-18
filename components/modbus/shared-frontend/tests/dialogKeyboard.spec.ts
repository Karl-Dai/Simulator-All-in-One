import { afterEach, describe, expect, it } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AppDialog from '../src/components/AppDialog.vue'
import { dialogCancel, showConfirm, showPrompt } from '../src/composables/useDialog'

let wrapper: ReturnType<typeof mount>
afterEach(() => { dialogCancel(); wrapper?.unmount() })
describe('dialog keyboard behavior', () => {
  it('does not confirm when Enter is pressed on Cancel', async () => {
    wrapper = mount(AppDialog, { global: { stubs: { teleport: true } } })
    const result = showConfirm('Delete connection?')
    await flushPromises()
    const cancel = wrapper.get('.btn-secondary')
    await cancel.trigger('keydown', { key: 'Enter' })
    expect(wrapper.find('.dialog').exists()).toBe(true)
    // The browser dispatches the focused button click after Enter.
    await cancel.trigger('click')
    expect(await result).toBe(false)
  })
  it('keeps the dialog open when Escape cancels IME composition', async () => {
    wrapper = mount(AppDialog, { global: { stubs: { teleport: true } } })
    const result = showConfirm('Delete connection?')
    await flushPromises()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true }))
    await flushPromises()
    expect(wrapper.find('.dialog').exists()).toBe(true)
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    expect(await result).toBe(false)
  })
  it('refreshes a replacement prompt and submits its current input once', async () => {
    wrapper = mount(AppDialog, { global: { stubs: { teleport: true } } })
    const first = showPrompt('First', 'old')
    await flushPromises()
    const second = showPrompt('Second', 'new')
    await flushPromises()
    expect(await first).toBeNull()
    const input = wrapper.get('input')
    expect(input.element.value).toBe('new')
    await input.trigger('keydown', { key: 'Enter' })
    expect(await second).toBe('new')
  })
})
