import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { startSuiteTheme } from '../../ui/theme'

let dispose: (() => void) | undefined
let systemChanged: (() => void) | undefined
let dark = false
const bridge = window as unknown as {
  __TAURI_INTERNALS__?: { invoke: ReturnType<typeof vi.fn> }
}

beforeEach(() => {
  vi.useFakeTimers()
  dark = false
  vi.stubGlobal('matchMedia', () => ({
    get matches() { return dark },
    addEventListener: (_: string, handler: () => void) => { systemChanged = handler },
    removeEventListener: vi.fn(),
  }))
})
afterEach(() => {
  dispose?.()
  dispose = undefined
  delete bridge.__TAURI_INTERNALS__
  delete document.documentElement.dataset.simlabTheme
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  vi.useRealTimers()
})

describe('suite appearance across independent windows', () => {
  it('reads native preferences, polls changes and stops after disposal', async () => {
    const invoke = vi.fn().mockResolvedValue('light')
    bridge.__TAURI_INTERNALS__ = { invoke }
    dispose = startSuiteTheme()
    await vi.advanceTimersByTimeAsync(0)
    expect(invoke).toHaveBeenCalledWith('simlab_theme')
    expect(document.documentElement.dataset.simlabTheme).toBe('light')
    invoke.mockResolvedValue('dark')
    await vi.advanceTimersByTimeAsync(1500)
    expect(document.documentElement.dataset.simlabTheme).toBe('dark')
    dispose()
    const count = invoke.mock.calls.length
    await vi.advanceTimersByTimeAsync(5000)
    expect(invoke).toHaveBeenCalledTimes(count)
  })

  it('follows OS changes in system mode and retains the last theme on IPC failure', async () => {
    const invoke = vi.fn().mockResolvedValue('system')
    bridge.__TAURI_INTERNALS__ = { invoke }
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => {})
    dispose = startSuiteTheme()
    await vi.advanceTimersByTimeAsync(0)
    dark = true
    systemChanged?.()
    expect(document.documentElement.dataset.simlabTheme).toBe('dark')
    invoke.mockRejectedValue(new Error('Unavailable'))
    await vi.advanceTimersByTimeAsync(4500)
    expect(document.documentElement.dataset.simlabTheme).toBe('dark')
    expect(warning).toHaveBeenCalledTimes(1)
  })

  it('uses the Wails appearance bridge for both Modbus roles', async () => {
    const Theme = vi.fn().mockResolvedValue('dark')
    const go = window as unknown as { go?: unknown }
    go.go = { suitehost: { Appearance: { Theme } } }
    dispose = startSuiteTheme()
    await vi.advanceTimersByTimeAsync(0)
    expect(Theme).toHaveBeenCalledOnce()
    expect(document.documentElement.dataset.simlabTheme).toBe('dark')
    delete go.go
  })
})
