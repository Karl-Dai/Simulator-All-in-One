import { afterEach, expect, it, vi } from 'vitest'
import { startNativeWindowTheme } from './nativeWindowTheme'

let stop: (() => void) | undefined
const bridge = window as unknown as { runtime?: unknown }
afterEach(() => {
 stop?.(); stop = undefined; delete bridge.runtime
 delete document.documentElement.dataset.theme
 delete document.documentElement.dataset.simlabTheme
})
it('syncs Windows chrome with the effective suite theme and ignores duplicate DOM changes', async () => {
 const runtime = { Environment: vi.fn().mockResolvedValue({ platform: 'windows' }), WindowSetDarkTheme: vi.fn(), WindowSetLightTheme: vi.fn(), WindowSetSystemDefaultTheme: vi.fn() }
 bridge.runtime = runtime
 document.documentElement.dataset.theme = 'light'
 stop = await startNativeWindowTheme()
 expect(runtime.WindowSetLightTheme).toHaveBeenCalledOnce()
 document.documentElement.dataset.simlabTheme = 'dark'
 await Promise.resolve()
 expect(runtime.WindowSetDarkTheme).toHaveBeenCalledOnce()
 document.documentElement.dataset.theme = 'light'
 await Promise.resolve()
 expect(runtime.WindowSetLightTheme).toHaveBeenCalledOnce()
 delete document.documentElement.dataset.simlabTheme
 document.documentElement.dataset.theme = 'system'
 await Promise.resolve()
 expect(runtime.WindowSetSystemDefaultTheme).toHaveBeenCalledOnce()
 stop()
 document.documentElement.dataset.theme = 'dark'
 await Promise.resolve()
 expect(runtime.WindowSetDarkTheme).toHaveBeenCalledOnce()
})
it('does not call Windows appearance APIs on macOS', async () => {
 const dark = vi.fn()
 bridge.runtime = { Environment: vi.fn().mockResolvedValue({ platform: 'darwin' }), WindowSetDarkTheme: dark }
 document.documentElement.dataset.theme = 'dark'
 stop = await startNativeWindowTheme()
 expect(dark).not.toHaveBeenCalled()
})
it('allows browser previews without a native bridge', async () => {
 await expect(startNativeWindowTheme()).resolves.toBeTypeOf('function')
})
