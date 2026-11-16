import { nextTick, watch } from 'vue'
import type { StartConfig } from './types'

const KEY = 'iec101.slave:form:v1'

/** 启动前恢复上一次保存的表单；只在套件模式调用。 */
export function loadPersistedForm<T extends object>(targets: T[]): void {
  const raw = localStorage.getItem(KEY)
  if (!raw) return
  let saved: unknown
  try {
    saved = JSON.parse(raw)
  } catch {
    return
  }
  if (!saved || typeof saved !== 'object') return
  const values = saved as Record<string, unknown>
  targets.forEach((target) => {
    for (const key of Object.keys(target)) {
      const value = values[key]
      if (value === undefined) continue
      if (Array.isArray(target[key as keyof T])) {
        if (Array.isArray(value)) target[key as keyof T] = value as T[keyof T]
      } else if (value && typeof value === 'object') {
        Object.assign(target[key as keyof T] as object, value)
      } else if (typeof value === typeof target[key as keyof T]) {
        target[key as keyof T] = value as T[keyof T]
      }
    }
  })
}

/** 表单变化后写回 localStorage；套件模式下它由 SQLite 支撑。 */
export function watchPersistedForm(sources: object[]): void {
  let timer: ReturnType<typeof setTimeout> | undefined
  const save = () => {
    const snapshot = sources.map((source) => source)
    localStorage.setItem(KEY, JSON.stringify(Object.assign({}, ...snapshot)))
  }
  watch(
    sources,
    () => {
      clearTimeout(timer)
      timer = setTimeout(() => void nextTick().then(save), 300)
    },
    { deep: true },
  )
  void nextTick().then(save)
  window.addEventListener('pagehide', save)
}

export type PersistedForm = StartConfig
