import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { LogEntry, Snapshot, StartConfig } from './types'

export function start(config: StartConfig): Promise<void> {
  return invoke<void>('start', { config })
}

export function stop(): Promise<void> {
  return invoke<void>('stop')
}

export function setPoint(ioa: number, value: unknown): Promise<void> {
  return invoke<void>('setPoint', { ioa, value })
}

export function snapshot(): Promise<Snapshot> {
  return invoke<Snapshot>('snapshot')
}

export function onSnapshot(callback: (snapshot: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>('snapshot', (event) => callback(event.payload))
}

export function onLog(callback: (entry: LogEntry) => void): Promise<UnlistenFn> {
  return listen<LogEntry>('log', (event) => callback(event.payload))
}

/** 生成一条本地错误日志，时间格式与后端保持一致。 */
export function makeErrorLog(summary: string): LogEntry {
  const now = new Date()
  const pad = (value: number, width = 2): string => String(value).padStart(width, '0')
  const at = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} `
    + `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`
    + `.${pad(now.getMilliseconds(), 3)}`
  return { at, direction: 'error', hex: '', summary }
}
