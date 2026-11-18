import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { shallowMount, mount } from '@vue/test-utils'
import { useI18n } from '../src/i18n'
import { createSuiteAppearance, suiteAppearanceKey, type SuiteAppearance } from '../../master-frontend/src/suiteAppearance'
import LangControl from '../src/components/LangToggle.vue'
import App from '../../master-frontend/src/App.vue'
import StatusBadge from '../../master-frontend/src/simlab-ui/components/StatusBadge.vue'
import ErrorNotice from '../../master-frontend/src/simlab-ui/components/ErrorNotice.vue'
import en from '../src/i18n/locales/en-US'
import zh from '../src/i18n/locales/zh-CN'

vi.mock('../../master-frontend/wailsjs/runtime/runtime', () => ({ EventsOn: vi.fn(() => () => {}), BrowserOpenURL: vi.fn() }))

const savedLocaleKey = 'modbussim.locale'
let session: SuiteAppearance = null
const mounted: Array<{ unmount(): void }> = []

beforeEach(() => {
  vi.useFakeTimers()
  vi.spyOn(document, 'hidden', 'get').mockReturnValue(false)
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }))
  vi.spyOn(console, 'warn').mockImplementation(() => {})
  localStorage.clear()
  const i18n = useI18n()
  i18n.setEffectiveLocale(null)
  i18n.setLocale('en-US')
})

afterEach(() => {
  mounted.splice(0).forEach(wrapper => wrapper.unmount())
  session?.dispose()
  session = null
  useI18n().setEffectiveLocale(null)
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('master shared appearance integration', () => {
  it('keeps standalone mode local and preserves language writes', async () => {
    const reader = { readAppearance: vi.fn() }
    session = createSuiteAppearance(false, reader)
    expect(session).toBeNull()
    await vi.advanceTimersByTimeAsync(3000)
    expect(reader.readAppearance).not.toHaveBeenCalled()
    const i18n = useI18n()
    expect(i18n.languageManaged.value).toBe(false)
    i18n.setLocale('zh-CN')
    expect(localStorage.getItem(savedLocaleKey)).toBe('zh-CN')
  })

  it('applies suite language transiently and disables the local language control', async () => {
    session = createSuiteAppearance(true, {
      readAppearance: vi.fn().mockResolvedValue({ schemaVersion: 1, theme: 'dark', language: 'zh-CN' }),
    })
    await vi.advanceTimersByTimeAsync(0)
    const i18n = useI18n()
    expect(i18n.locale.value).toBe('zh-CN')
    expect(i18n.languageManaged.value).toBe(true)
    expect(localStorage.getItem(savedLocaleKey)).toBe('en-US')
    const wrapper = mount(LangControl)
    mounted.push(wrapper)
    expect(wrapper.text()).toContain(zh.suiteAppearance.managed)
    expect(wrapper.findAll('button').every(button => button.attributes('disabled') !== undefined)).toBe(true)
    i18n.setLocale('en-US')
    expect(i18n.locale.value).toBe('zh-CN')
    expect(localStorage.getItem(savedLocaleKey)).toBe('en-US')
    session?.dispose()
    await nextTick()
    expect(i18n.locale.value).toBe('en-US')
    expect(wrapper.findAll('button').every(button => button.attributes('disabled') === undefined)).toBe(true)
  })

  it('does not let an older session release the newer locale override', async () => {
    const first = createSuiteAppearance(true, {
      readAppearance: vi.fn().mockResolvedValue({ schemaVersion: 1, theme: 'light', language: 'en' }),
    })
    await vi.advanceTimersByTimeAsync(0)
    session = createSuiteAppearance(true, {
      readAppearance: vi.fn().mockResolvedValue({ schemaVersion: 1, theme: 'dark', language: 'zh-CN' }),
    })
    await vi.advanceTimersByTimeAsync(0)
    first?.dispose()
    expect(useI18n().languageManaged.value).toBe(true)
    expect(useI18n().locale.value).toBe('zh-CN')
    expect(localStorage.getItem(savedLocaleKey)).toBe('en-US')
  })

  it('provides matching translated suite messages in both dictionaries', () => {
    expect(Object.keys(en.suiteAppearance).sort()).toEqual(Object.keys(zh.suiteAppearance).sort())
    const i18n = useI18n()
    i18n.setEffectiveLocale('zh-CN')
    expect(i18n.t('suiteAppearance.readFailed')).toBe(zh.suiteAppearance.readFailed)
    i18n.setEffectiveLocale('en-US')
    expect(i18n.t('suiteAppearance.readFailed')).toBe(en.suiteAppearance.readFailed)
    expect(localStorage.getItem(savedLocaleKey)).toBe('en-US')
  })

  it('shows real selected state and an actionable appearance error without exposing native details', async () => {
    const readAppearance = vi.fn().mockRejectedValue(new Error('private configuration payload'))
    session = createSuiteAppearance(true, { readAppearance })
    const wrapper = shallowMount(App, {
      global: { provide: { [suiteAppearanceKey as symbol]: session }, stubs: { StatusBadge: false, ErrorNotice: false } },
    })
    mounted.push(wrapper)
    await vi.advanceTimersByTimeAsync(0)
    await nextTick()
    expect(wrapper.getComponent(StatusBadge).text()).toBe(en.suiteAppearance.noConnection)
    const vm = wrapper.vm as unknown as { handleConnectionSelect(id: string, state: string): void }
    vm.handleConnectionSelect('test-connection', 'Connected')
    await nextTick()
    expect(wrapper.getComponent(StatusBadge).text()).toBe(en.suiteAppearance.connected)
    expect(wrapper.getComponent(ErrorNotice).text()).toContain(en.suiteAppearance.readFailed)
    expect(wrapper.text()).not.toContain('private configuration payload')
    expect(JSON.stringify(vi.mocked(console.warn).mock.calls)).not.toContain('private configuration payload')
    readAppearance.mockResolvedValue({ schemaVersion: 1, theme: 'light', language: 'en' })
    await wrapper.getComponent(ErrorNotice).findAll('button')[0].trigger('click')
    await vi.advanceTimersByTimeAsync(0)
    expect(wrapper.findComponent(ErrorNotice).exists()).toBe(false)
    readAppearance.mockRejectedValue(new Error('another read failure'))
    await vi.advanceTimersByTimeAsync(1500)
    await nextTick()
    await wrapper.getComponent(ErrorNotice).findAll('button')[1].trigger('click')
    expect(wrapper.findComponent(ErrorNotice).exists()).toBe(false)
    await vi.advanceTimersByTimeAsync(3000)
    expect(wrapper.findComponent(ErrorNotice).exists()).toBe(false)
    const callsBeforeUnmount = readAppearance.mock.calls.length
    wrapper.unmount()
    mounted.pop()
    await vi.advanceTimersByTimeAsync(3000)
    expect(readAppearance).toHaveBeenCalledTimes(callsBeforeUnmount)
    expect(useI18n().languageManaged.value).toBe(false)
  })
})
