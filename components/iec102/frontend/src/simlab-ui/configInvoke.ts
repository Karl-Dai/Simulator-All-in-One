import { invoke as nativeInvoke, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core'
import { trackConfigurationRequest } from './configClose'
export * from '@tauri-apps/api/core'

// Keep the SDK bridge untouched: its native invoke property is read-only.
export function invoke<T>(command: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  const request = arguments.length > 2 ? nativeInvoke<T>(command, args, options)
    : arguments.length > 1 ? nativeInvoke<T>(command, args) : nativeInvoke<T>(command)
  return trackConfigurationRequest(command, request)
}
