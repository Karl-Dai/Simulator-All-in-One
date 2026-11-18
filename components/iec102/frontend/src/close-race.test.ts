import { beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick, reactive } from 'vue'
import { setPreferencesFlusher, watchPersistedForm } from './suite-persist'
import { installSuitePreferences } from 'simlab-ui/configPreferences'
import { flushAllConfiguration, installConfigurationClose } from 'simlab-ui/configClose'

const ROLE = 'iec102.slave'
const KEY = 'iec102.slave:form:v1'

type SaveValues = Record<string, { kind: string; value: unknown }>

function mockNative() {
  const state = { saves: [] as { values: SaveValues }[], failNext: false }
  ;(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: async (command: string, args: { values?: SaveValues }) => {
      if (command === 'simlab_load_preferences') return {}
      if (command === 'simlab_save_preferences') {
        if (state.failNext) {
          state.failNext = false
          throw new Error('database is locked')
        }
        state.saves.push({ values: args.values! })
        return undefined
      }
      throw new Error('unexpected command ' + command)
    },
  }
  return state
}

function lastSavedPort(state: ReturnType<typeof mockNative>): number | undefined {
  const item = state.saves[state.saves.length - 1]?.values[KEY]
  return item?.kind === 'json' ? (item.value as { port?: number }).port : undefined
}

beforeEach(() => {
  document.body.innerHTML = '<div id="app"></div><input id="field" />'
})

describe('关闭前保存最新表单', () => {
  it('修改后不足 300ms 立即关闭，提交最新值后才确认', async () => {
    const native = mockNative()
    const preferences = await installSuitePreferences(ROLE)
    setPreferencesFlusher(preferences.flush)
    const form = reactive({ host: '127.0.0.1', port: 2401 })
    watchPersistedForm([form])
    await nextTick()
    await flushAllConfiguration()
    let confirmed = 0
    let request!: () => void
    await installConfigurationClose(
      (cb) => { request = cb; return () => {} },
      async () => { confirmed++ },
      async () => {},
    )
    form.port = 2601
    await nextTick()
    request()
    await vi.waitFor(() => expect(confirmed).toBe(1))
    expect(lastSavedPort(native)).toBe(2601)
  })

  it('数据库保存失败不确认关闭，重试成功后才关闭', async () => {
    const native = mockNative()
    const preferences = await installSuitePreferences(ROLE)
    setPreferencesFlusher(preferences.flush)
    const form = reactive({ host: '127.0.0.1', port: 2401 })
    watchPersistedForm([form])
    await nextTick()
    await flushAllConfiguration()
    let confirmed = 0
    let request!: () => void
    await installConfigurationClose(
      (cb) => { request = cb; return () => {} },
      async () => { confirmed++ },
      async () => {},
    )
    form.port = 2701
    await nextTick()
    native.failNext = true
    request()
    await vi.waitFor(() => expect(document.querySelector('[data-saio-close-error]')).not.toBeNull())
    expect(confirmed).toBe(0)
    request()
    await vi.waitFor(() => expect(confirmed).toBe(1))
    expect(lastSavedPort(native)).toBe(2701)
    expect(document.querySelector('[data-saio-close-error]')).toBeNull()
  })
})
