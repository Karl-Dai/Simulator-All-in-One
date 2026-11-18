import { nextTick, watch } from 'vue'
import { registerConfigurationFlusher } from 'simlab-ui/configClose'
import type { StartConfig } from './types'

const KEY = 'iec102.slave:form:v1'

/** 套件配置存储的保存入口，由 main.ts 在 installSuitePreferences 之后注入。 */
let preferencesFlush: (() => Promise<void>) | undefined

/** 注入 SQLite 保存函数；关闭流程提交表单后必须等待它完成。 */
export function setPreferencesFlusher(flush: () => Promise<void>): void {
  preferencesFlush = flush
}

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
  const snapshot = () => JSON.stringify(Object.assign({}, ...sources))
  const save = () => {
    localStorage.setItem(KEY, snapshot())
  }
  // 立即提交最新值，不等 300ms 防抖；关闭流程与页面隐藏时调用。
  const commit = () => {
    if (timer) {
      clearTimeout(timer)
      timer = undefined
    }
    save()
  }
  watch(
    sources,
    () => {
      if (timer) clearTimeout(timer)
      timer = setTimeout(() => {
        timer = undefined
        void nextTick().then(save)
      }, 300)
    },
    { deep: true },
  )
  void nextTick().then(save)
  window.addEventListener('pagehide', commit)
  // 关闭时先提交最新表单，再等待 SQLite 保存完成；保存失败会抛出，
  // 由 installConfigurationClose 保持窗口打开并允许重试。
  // 即使本 flusher 排在存储 flusher 之后，显式等待也能保证最终写入最新值。
  registerConfigurationFlusher(async () => {
    commit()
    if (preferencesFlush) await preferencesFlush()
  })
}

export type PersistedForm = StartConfig
