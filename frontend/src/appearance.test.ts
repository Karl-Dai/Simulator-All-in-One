import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { startAppearanceSync, type AppearanceSync } from '../../ui/appearance'
import {
  AppearanceContractError,
  parseAppearance,
  resolveLanguage,
  type AppearanceV1,
  type Language,
  type ResolvedLocale,
} from '../../ui/contracts/appearance'
import cases from '../../ui/contracts/appearance-cases.json'

const appearance = (theme: AppearanceV1['theme'] = 'light', language: Language = 'en'): AppearanceV1 => ({
  schemaVersion: 1, theme, language,
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

describe('appearance interface contract', () => {
  for (const sample of cases.interfaceCases) {
    it(sample.name, () => {
      if (sample.error) {
        expect(() => parseAppearance(sample.input)).toThrow(AppearanceContractError)
        try { parseAppearance(sample.input) } catch (error) {
          expect((error as AppearanceContractError).code).toBe(sample.error)
        }
      } else {
        expect(parseAppearance(sample.input)).toEqual(sample.appearance)
      }
    })
  }
  for (const sample of cases.languageCases) {
    it(`resolves ${sample.language} with ${sample.systemLanguage || 'no system language'}`, () => {
      expect(resolveLanguage(sample.language as Language, sample.systemLanguage)).toBe(sample.locale)
    })
  }
})

describe('injected appearance synchronization', () => {
  let sync: AppearanceSync | undefined
  let dark = false
  let hidden = false
  let systemLanguage = 'en-US'
  let media: EventTarget & { matches: boolean }
  let onLocale = vi.fn<(locale: ResolvedLocale) => void>()
  let onError = vi.fn<(error: Error | null) => void>()

  function visibility(value: boolean) {
    hidden = value
    document.dispatchEvent(new Event('visibilitychange'))
  }

  beforeEach(() => {
    vi.useFakeTimers()
    dark = false
    hidden = false
    systemLanguage = 'en-US'
    media = Object.assign(new EventTarget(), { matches: false })
    Object.defineProperty(media, 'matches', { get: () => dark })
    vi.stubGlobal('matchMedia', () => media)
    vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden)
    vi.spyOn(navigator, 'languages', 'get').mockImplementation(() => [systemLanguage])
    onLocale = vi.fn()
    onError = vi.fn()
    delete document.documentElement.dataset.simlab
    delete document.documentElement.dataset.simlabTheme
    document.documentElement.lang = ''
  })

  afterEach(() => {
    sync?.dispose()
    sync = undefined
    delete document.documentElement.dataset.simlab
    delete document.documentElement.dataset.simlabTheme
    delete document.documentElement.dataset.theme
    document.documentElement.style.colorScheme = ''
    document.documentElement.lang = ''
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it('polls injected settings and changes effective locale without persistence', async () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem')
    const readAppearance = vi.fn().mockResolvedValue(appearance('light', 'zh-CN'))
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await sync.refresh()
    expect(document.documentElement.dataset.simlabTheme).toBe('light')
    expect(document.documentElement.lang).toBe('zh-CN')
    expect(onLocale).toHaveBeenLastCalledWith('zh-CN')
    const localeCalls = onLocale.mock.calls.length
    await vi.advanceTimersByTimeAsync(1500)
    expect(onLocale).toHaveBeenCalledTimes(localeCalls)
    readAppearance.mockResolvedValue(appearance('dark', 'en'))
    await vi.advanceTimersByTimeAsync(1500)
    expect(readAppearance).toHaveBeenCalledTimes(3)
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(onLocale).toHaveBeenLastCalledWith('en')
    expect(setItem).not.toHaveBeenCalled()
  })

  it('keeps the last valid value on errors, deduplicates errors and clears on recovery', async () => {
    const readAppearance = vi.fn().mockResolvedValue(appearance('dark', 'zh-CN'))
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await sync.refresh()
    readAppearance.mockRejectedValue(new Error('Preference file is unreadable'))
    await vi.advanceTimersByTimeAsync(4500)
    expect(onError).toHaveBeenCalledTimes(1)
    expect(document.documentElement.dataset.simlabTheme).toBe('dark')
    expect(document.documentElement.lang).toBe('zh-CN')
    readAppearance.mockResolvedValue(appearance('light', 'en'))
    await vi.advanceTimersByTimeAsync(1500)
    expect(onError).toHaveBeenLastCalledWith(null)
    expect(document.documentElement.dataset.simlabTheme).toBe('light')
  })

  it('uses defaults on initial failure and reports incompatible interface versions', async () => {
    dark = true
    const readAppearance = vi.fn().mockResolvedValue({ schemaVersion: 2, theme: 'light', language: 'zh-CN' })
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await sync.refresh()
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(document.documentElement.lang).toBe('en')
    expect((onError.mock.calls[0][0] as AppearanceContractError).code).toBe('unsupported_version')
    expect(document.documentElement.dataset.simlab).toBe('component')
  })

  it('pauses while hidden and refreshes immediately when visible', async () => {
    const readAppearance = vi.fn().mockResolvedValue(appearance())
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await sync.refresh()
    visibility(true)
    await vi.advanceTimersByTimeAsync(10000)
    expect(readAppearance).toHaveBeenCalledTimes(1)
    readAppearance.mockResolvedValue(appearance('dark'))
    visibility(false)
    await sync.refresh()
    expect(readAppearance).toHaveBeenCalledTimes(2)
    expect(document.documentElement.dataset.theme).toBe('dark')
  })

  it('does not request settings while initially hidden', async () => {
    hidden = true
    const readAppearance = vi.fn().mockResolvedValue(appearance())
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await vi.advanceTimersByTimeAsync(3000)
    expect(readAppearance).not.toHaveBeenCalled()
    visibility(false)
    await sync.refresh()
    expect(readAppearance).toHaveBeenCalledOnce()
  })

  it('ignores a stale hidden-window response without delaying the visible refresh', async () => {
    const old = deferred<AppearanceV1>()
    const readAppearance = vi.fn().mockReturnValueOnce(old.promise).mockResolvedValue(appearance('dark', 'zh-CN'))
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await vi.advanceTimersByTimeAsync(0)
    visibility(true)
    visibility(false)
    await sync.refresh()
    expect(document.documentElement.dataset.theme).toBe('dark')
    old.resolve(appearance('light', 'en'))
    await vi.advanceTimersByTimeAsync(0)
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(document.documentElement.lang).toBe('zh-CN')
    await vi.advanceTimersByTimeAsync(1500)
    expect(readAppearance).toHaveBeenCalledTimes(3)
  })

  it('coalesces simultaneous refreshes into one native read', async () => {
    const next = deferred<AppearanceV1>()
    const readAppearance = vi.fn().mockReturnValue(next.promise)
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    const first = sync.refresh()
    const second = sync.refresh()
    expect(first).toBe(second)
    await vi.advanceTimersByTimeAsync(0)
    expect(readAppearance).toHaveBeenCalledOnce()
    next.resolve(appearance())
    await first
  })

  it('ignores pending results and removes polling and listeners after disposal', async () => {
    const next = deferred<AppearanceV1>()
    const readAppearance = vi.fn().mockReturnValue(next.promise)
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await vi.advanceTimersByTimeAsync(0)
    sync.dispose()
    next.resolve(appearance('dark', 'zh-CN'))
    dark = true
    media.dispatchEvent(new Event('change'))
    systemLanguage = 'zh-CN'
    window.dispatchEvent(new Event('languagechange'))
    visibility(false)
    await vi.advanceTimersByTimeAsync(5000)
    expect(readAppearance).toHaveBeenCalledOnce()
    expect(document.documentElement.dataset.theme).toBe('light')
    expect(onLocale).toHaveBeenCalledTimes(1)
    expect(onError).not.toHaveBeenCalled()
  })

  it('does not call the reader when disposed before its first microtask', async () => {
    const readAppearance = vi.fn().mockResolvedValue(appearance())
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    sync.dispose()
    await vi.advanceTimersByTimeAsync(0)
    expect(readAppearance).not.toHaveBeenCalled()
  })

  it('replaces an existing synchronizer and prevents its old response from winning', async () => {
    const previous = deferred<AppearanceV1>()
    const oldRead = vi.fn().mockReturnValue(previous.promise)
    startAppearanceSync({ readAppearance: oldRead }, { onLocale, onError })
    await vi.advanceTimersByTimeAsync(0)
    const readAppearance = vi.fn().mockResolvedValue(appearance('dark', 'zh-CN'))
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await sync.refresh()
    previous.resolve(appearance('light'))
    await vi.advanceTimersByTimeAsync(1500)
    expect(oldRead).toHaveBeenCalledOnce()
    expect(readAppearance).toHaveBeenCalledTimes(2)
    expect(document.documentElement.dataset.theme).toBe('dark')
  })

  it('follows OS appearance and language changes only in automatic modes', async () => {
    const readAppearance = vi.fn().mockResolvedValue(appearance('system', 'auto'))
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError })
    await sync.refresh()
    dark = true
    media.dispatchEvent(new Event('change'))
    systemLanguage = 'zh-HK'
    window.dispatchEvent(new Event('languagechange'))
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(onLocale).toHaveBeenLastCalledWith('zh-CN')
    readAppearance.mockResolvedValue(appearance('light', 'en'))
    await sync.refresh()
    media.dispatchEvent(new Event('change'))
    window.dispatchEvent(new Event('languagechange'))
    expect(document.documentElement.dataset.theme).toBe('light')
    expect(onLocale).toHaveBeenLastCalledWith('en')
  })

  it('supports a standalone local reader without enabling suite CSS or accessing storage', async () => {
    const getItem = vi.spyOn(Storage.prototype, 'getItem')
    const setItem = vi.spyOn(Storage.prototype, 'setItem')
    const readAppearance = vi.fn().mockResolvedValue(appearance('dark', 'zh-CN'))
    sync = startAppearanceSync({ readAppearance }, { onLocale, onError, mode: 'standalone' })
    await sync.refresh()
    expect(document.documentElement.dataset.simlab).toBeUndefined()
    expect(document.documentElement.dataset.simlabTheme).toBeUndefined()
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(getItem).not.toHaveBeenCalled()
    expect(setItem).not.toHaveBeenCalled()
  })
})
