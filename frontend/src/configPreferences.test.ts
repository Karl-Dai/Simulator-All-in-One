import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { decodePreferences, encodePreferences, installSuitePreferences } from '../../ui/configPreferences'
const original = window.localStorage
const host = window as unknown as { go?: unknown }
beforeEach(() => { Object.defineProperty(window, 'localStorage', { configurable: true, value: original }); original.clear(); document.body.replaceChildren(); vi.useFakeTimers() })
afterEach(() => { vi.useRealTimers(); delete host.go; Object.defineProperty(window, 'localStorage', { configurable: true, value: original }) })
function bridge(load: ReturnType<typeof vi.fn>, save: ReturnType<typeof vi.fn>) { host.go = { main: { App: { LoadPreferences: load, SavePreferences: save } } } }
it('stores JSON objects and arrays as typed values and roundtrips plain text', () => {
 const wire = encodePreferences({ form: '{"count":123}', list: '[true,null]', text: '123', invalid: '{text' })
 expect(wire.form).toEqual({ kind: 'json', value: { count: 123 } })
 expect(wire.list).toEqual({ kind: 'json', value: [true, null] })
 expect(wire.text).toEqual({ kind: 'text', value: '123' })
 expect(decodePreferences(wire)).toEqual({ form: '{"count":123}', list: '[true,null]', text: '123', invalid: '{text' })
})
it('restores before clients read and preserves original browser data through clear', async () => {
 original.setItem('theme', 'dark')
 const load = vi.fn().mockResolvedValue(encodePreferences({ theme: 'light' })); const save = vi.fn().mockResolvedValue(undefined); bridge(load, save)
 const handle = await installSuitePreferences('modbus.master')
 expect(load).toHaveBeenCalledWith(encodePreferences({ theme: 'dark' })); expect(localStorage.getItem('theme')).toBe('light')
 localStorage.clear(); await handle.flush(); expect(save).toHaveBeenLastCalledWith({}); expect(original.getItem('theme')).toBe('dark')
})
it('serializes saves and follows an in-flight write with the latest snapshot', async () => {
 let finish!: () => void; const first = new Promise<void>(resolve => { finish = resolve })
 const save = vi.fn().mockReturnValueOnce(first).mockResolvedValue(undefined); bridge(vi.fn().mockResolvedValue({}), save)
 const handle = await installSuitePreferences('pmu.master'); localStorage.setItem('size', '1'); const flushing = handle.flush(); localStorage.setItem('size', '2')
 expect(save).toHaveBeenCalledOnce(); finish(); await flushing
 expect(save.mock.calls.map(([snapshot]) => decodePreferences(snapshot))).toEqual([{ size: '1' }, { size: '2' }])
 expect(document.querySelector('[data-saio-save-status="saved"]')).not.toBeNull()
})
it('retains dirty values after a failure and retries', async () => {
 const save = vi.fn().mockRejectedValueOnce(new Error('database busy')).mockResolvedValue(undefined); bridge(vi.fn().mockResolvedValue({}), save)
 const handle = await installSuitePreferences('pmu.sub'); localStorage.setItem('language', 'zh')
 await expect(handle.flush()).rejects.toThrow('database busy'); expect(document.querySelector('[role="alert"]')?.textContent).toContain('Configuration not saved'); expect(localStorage.getItem('language')).toBe('zh')
 await handle.flush(); expect(save).toHaveBeenLastCalledWith(encodePreferences({ language: 'zh' }))
})
it('blocks startup on restore error and retries before installing defaults', async () => {
 const load = vi.fn().mockRejectedValueOnce(new Error('corrupt')).mockResolvedValue(encodePreferences({ recovered: 'yes' })); const save = vi.fn(); bridge(load, save)
 let mounted = false; const installing = installSuitePreferences('modbus.slave').then(() => { mounted = true }); await Promise.resolve(); await Promise.resolve()
 expect(mounted).toBe(false); expect(window.localStorage).toBe(original); expect(save).not.toHaveBeenCalled()
 document.querySelector<HTMLButtonElement>('button')!.click(); await installing; expect(localStorage.getItem('recovered')).toBe('yes')
})
it('roundtrips arbitrary browser keys without changing object prototypes', async () => {
 original.setItem('__proto__', '{"safe":true}')
 original.setItem('constructor', 'custom')
 const load = vi.fn().mockImplementation(async value => value)
 const save = vi.fn().mockResolvedValue(undefined)
 bridge(load, save)
 const handle = await installSuitePreferences('modbus.master')
 expect(localStorage.getItem('__proto__')).toBe('{"safe":true}')
 expect(localStorage.getItem('constructor')).toBe('custom')
 localStorage.setItem('__proto__', '{"safe":false}')
 await handle.flush()
 expect(save.mock.calls[0][0]['__proto__']).toEqual({ kind: 'json', value: { safe: false } })
 expect(({} as Record<string, unknown>).safe).toBeUndefined()
})
