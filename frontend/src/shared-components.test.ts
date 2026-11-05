import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import StatusBadge from '../../ui/components/StatusBadge.vue'
import ErrorNotice from '../../ui/components/ErrorNotice.vue'

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach(wrapper => wrapper.unmount()))

describe('shared status badge', () => {
  it('shows the application-provided state without inventing protocol status', async () => {
    const wrapper = mount(StatusBadge, { props: { label: 'Application available', tone: 'success' } })
    mounted.push(wrapper)
    expect(wrapper.get('[role="status"]').text()).toBe('Application available')
    expect(wrapper.text()).not.toMatch(/connected/i)
    expect(wrapper.attributes('aria-live')).toBe('polite')
    await wrapper.setProps({ label: 'Connecting', busy: true, tone: 'neutral' })
    expect(wrapper.attributes('aria-busy')).toBe('true')
    expect(wrapper.find('[aria-hidden="true"]').classes()).toContain('status-badge__indicator--busy')
    expect(wrapper.text()).toBe('Connecting')
  })
})

describe('shared error notice', () => {
  it('renders untrusted details as text and provides native keyboard controls', () => {
    const wrapper = mount(ErrorNotice, { props: {
      message: 'Connection failed', details: '<script>window.alert(1)</script>',
      detailsLabel: 'Details', closeLabel: 'Close', retryLabel: 'Retry',
    } })
    mounted.push(wrapper)
    expect(wrapper.find('script').exists()).toBe(false)
    expect(wrapper.get('pre').text()).toBe('<script>window.alert(1)</script>')
    expect(wrapper.get('summary').text()).toBe('Details')
    expect(wrapper.findAll('button').every(button => button.attributes('type') === 'button')).toBe(true)
    expect(wrapper.get('[role="status"]').attributes('aria-live')).toBe('polite')
    expect(wrapper.findAll('[autofocus]')).toHaveLength(0)
  })

  it('emits retry only on an enabled user action and close separately', async () => {
    const wrapper = mount(ErrorNotice, { props: { message: 'Timeout', closeLabel: 'Close', retryLabel: 'Retry' } })
    mounted.push(wrapper)
    expect(wrapper.emitted('retry')).toBeUndefined()
    await wrapper.findAll('button')[0].trigger('click')
    expect(wrapper.emitted('retry')).toHaveLength(1)
    await wrapper.setProps({ busy: true })
    await wrapper.findAll('button')[0].trigger('click')
    expect(wrapper.emitted('retry')).toHaveLength(1)
    await wrapper.findAll('button')[1].trigger('click')
    expect(wrapper.emitted('close')).toHaveLength(1)
  })

  it('preserves focus and expanded details when the same error is refreshed', async () => {
    const wrapper = mount(ErrorNotice, { attachTo: document.body, props: {
      message: 'Still unavailable', details: 'Socket refused', detailsLabel: 'Details', closeLabel: 'Close',
    } })
    mounted.push(wrapper)
    const details = wrapper.get('details').element as HTMLDetailsElement
    details.open = true
    const button = wrapper.get('button').element as HTMLButtonElement
    button.focus()
    await wrapper.setProps({ message: 'Still unavailable' })
    expect(document.activeElement).toBe(button)
    expect(details.open).toBe(true)
    expect(wrapper.emitted('retry')).toBeUndefined()
  })

  it('accepts localized labels and has no suite-mode precondition', () => {
    delete document.documentElement.dataset.simlab
    delete document.documentElement.dataset.simlabTheme
    const wrapper = mount(ErrorNotice, { props: { message: '连接失败', closeLabel: '关闭' } })
    mounted.push(wrapper)
    expect(wrapper.text()).toContain('连接失败')
    expect(wrapper.get('button').text()).toBe('关闭')
    expect(wrapper.findAll('button')).toHaveLength(1)
    expect(wrapper.find('details').exists()).toBe(false)
    expect(document.documentElement.dataset.simlab).toBeUndefined()
  })
})
