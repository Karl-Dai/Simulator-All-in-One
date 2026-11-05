import type { ObjectDirective } from 'vue'

const stack: HTMLElement[] = []
const callbacks = new WeakMap<HTMLElement, () => void>()
const cleanups = new WeakMap<HTMLElement, () => void>()

/** Focus and Escape lifecycle for existing teleported dialogs. */
export const vModal: ObjectDirective<HTMLElement, () => void> = {
  mounted(el, binding) {
    const previous = document.activeElement as HTMLElement | null
    callbacks.set(el, binding.value)
    stack.push(el)
    const dialog = el.querySelector<HTMLElement>('[role="dialog"], [role="alertdialog"]') ?? el
    dialog.setAttribute('role', dialog.getAttribute('role') ?? 'dialog')
    dialog.setAttribute('aria-modal', 'true')
    if (!dialog.hasAttribute('aria-label') && !dialog.hasAttribute('aria-labelledby')) {
      const title = el.querySelector<HTMLElement>('.modal-title, .dialog-title, .about-title, h2, h3')?.textContent
      if (title) dialog.setAttribute('aria-label', title.trim())
    }
    el.tabIndex = -1
    const focusables = () => Array.from(el.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), a[href], summary, [tabindex]',
    )).filter(node => node.tabIndex >= 0 && node.getClientRects().length > 0 && !node.closest('[inert], [hidden]'))
    const focusFirst = () => (focusables()[0] ?? el).focus()
    const keydown = (event: KeyboardEvent) => {
      if (stack[stack.length - 1] !== el || (event.isComposing || event.keyCode === 229)) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopImmediatePropagation()
        callbacks.get(el)?.()
        return
      }
      if (event.key !== 'Tab') return
      const nodes = focusables()
      const first = nodes[0], last = nodes[nodes.length - 1]
      if (!first) { event.preventDefault(); el.focus(); return }
      if (event.shiftKey && (document.activeElement === first || document.activeElement === el || !el.contains(document.activeElement))) {
        event.preventDefault(); last?.focus()
      } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === el || !el.contains(document.activeElement))) {
        event.preventDefault(); first.focus()
      }
    }
    document.addEventListener('keydown', keydown, true)
    queueMicrotask(() => {
      if (el.isConnected && stack[stack.length - 1] === el && !el.contains(document.activeElement)) focusFirst()
    })
    cleanups.set(el, () => {
      document.removeEventListener('keydown', keydown, true)
      const wasTop = stack[stack.length - 1] === el
      const index = stack.indexOf(el)
      if (index >= 0) stack.splice(index, 1)
      if (wasTop && previous?.isConnected && (!stack.length || stack[stack.length - 1]?.contains(previous))) previous.focus()
    })
  },
  updated(el, binding) { callbacks.set(el, binding.value) },
  unmounted(el) {
    cleanups.get(el)?.()
    cleanups.delete(el)
    callbacks.delete(el)
  },
}
