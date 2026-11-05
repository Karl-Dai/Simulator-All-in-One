import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, nextTick } from 'vue'
import { vModal } from '../../ui/modal'

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => {
  mounted.reverse().forEach(wrapper => wrapper.unmount())
  mounted.length = 0
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

async function modal(close = vi.fn()) {
  // jsdom has no layout; mark these rendered controls visible for focus tests.
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockReturnValue([{}] as unknown as DOMRectList)
  const wrapper = mount(defineComponent({
    directives: { modal: vModal },
    props: ['close'],
    template: `<div v-modal="close"><h2>Dialog</h2><button>Cancel</button><button disabled>Disabled</button><input><button>Save</button></div>`,
  }), { attachTo: document.body, props: { close } })
  mounted.push(wrapper)
  await nextTick()
  await Promise.resolve()
  return { wrapper, close }
}

function key(value: string, shiftKey = false) {
  document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: value, shiftKey, bubbles: true, cancelable: true }))
}

describe('modal focus lifecycle', () => {
  it('focuses a visible control, wraps Tab, and restores the opener', async () => {
    const opener = document.createElement('button')
    document.body.append(opener)
    opener.focus()
    const { wrapper } = await modal()
    const buttons = wrapper.findAll('button')
    expect(document.activeElement).toBe(buttons[0].element)
    key('Tab', true)
    expect(document.activeElement).toBe(buttons[2].element)
    key('Tab')
    expect(document.activeElement).toBe(buttons[0].element)
    ;(wrapper.element as HTMLElement).focus()
    key('Tab', true)
    expect(document.activeElement).toBe(buttons[2].element)
    wrapper.unmount()
    mounted.pop()
    expect(document.activeElement).toBe(opener)
  })

  it('dismisses only the top dialog and uses the latest busy/close callback', async () => {
    const first = await modal()
    const second = await modal()
    const replacement = vi.fn()
    await second.wrapper.setProps({ close: replacement })
    key('Escape')
    expect(first.close).not.toHaveBeenCalled()
    expect(second.close).not.toHaveBeenCalled()
    expect(replacement).toHaveBeenCalledOnce()
    second.wrapper.unmount()
    mounted.pop()
    key('Escape')
    expect(first.close).toHaveBeenCalledOnce()
  })

  it('does not reinterpret Enter or Escape while composing text', async () => {
    const { close } = await modal()
    key('Enter')
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true }))
    expect(close).not.toHaveBeenCalled()
  })
})
