// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AppDialog from '@shared/components/AppDialog.vue'
import { showConfirm, showPrompt, dialogCancel } from '@shared/composables/useDialog'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
let wrapper: ReturnType<typeof mount> | undefined
afterEach(() => { dialogCancel(); wrapper?.unmount(); document.body.innerHTML = '' })

describe('dialog keyboard actions', () => {
  it('keeps Enter on Cancel from confirming', async () => {
    wrapper = mount(AppDialog, { attachTo: document.body })
    const result = showConfirm('Delete this connection?')
    await flushPromises()
    const cancel = document.querySelector<HTMLButtonElement>('.btn-secondary')!
    cancel.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    // Native button activation follows keydown in the browser.
    cancel.click()
    expect(await result).toBe(false)
  })

  it('submits the entered prompt value once and ignores IME composition', async () => {
    wrapper = mount(AppDialog, { attachTo: document.body })
    const result = showPrompt('Name', 'original')
    await flushPromises()
    const input = document.querySelector<HTMLInputElement>('input')!
    input.value = 'renamed'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    const composingEnter = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, isComposing: true, cancelable: true })
    input.dispatchEvent(composingEnter)
    expect(composingEnter.defaultPrevented).toBe(false)
    await flushPromises()
    expect(document.querySelector('.dialog')).not.toBeNull()
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    expect(await result).toBe('renamed')
  })
})
