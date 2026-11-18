# 子站「按 IOA 表达式批量写值」实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 子站新增「按 IOA 表达式批量写值」弹窗,支持一次性给散落在上万点位中的非连续点集（单点 + 区间混合）写同一个值。

**Architecture:** 纯前端。前端解析 IOA 表达式 → 与 `DataPointTable.dataMap` 中选定 `asdu_type` 的已存在点求交 → 得显式 `[{ioa, asdu_type}]` → 调用**现有** Tauri 命令 `batch_update_data_points`（含同分类校验/全或无写入/自发上送）。后端零改动。

**Tech Stack:** Vue 3 `<script setup>` + TypeScript；Tauri `invoke`；Vitest + @vue/test-utils（自动化测试）；Playwright MCP（最终无头浏览器冒烟）。

**规约：**
- 所有 git commit 用 `--author='Karl-Dai Karl <kelsoprotein@gmail.com>'`，**禁止** Claude/Claude Code 署名或 Co-Authored-By 行。
- 仓库在 `~/Developer/IEC60870-5-104-Simulator`（非 iCloud 独立 clone）。
- 设计依据：`docs/superpowers/specs/2026-06-16-slave-batch-write-by-ioa-design.md`。

---

## 文件结构

| 角色 | 文件 | 责任 |
|---|---|---|
| 解析/求交工具（扩展） | `frontend/src/components/batchAdd/ioaRanges.ts` | 新增纯函数 `parseIoaExpression` + `resolveIoaHits`，复用已有 `compressRanges`/`lowerBound`/`IOA_MAX` |
| 工具单测 | `frontend/tests/batchAdd/ioaRanges.spec.ts` | 给两个新函数补测（追加到现有文件） |
| 新弹窗组件 | `frontend/src/components/BatchWriteModal.vue` | 类型下拉 + IOA 文本框 + 命中卡片 + 值输入 + 写入；复用 `BatchAddModal` 视觉 |
| 组件测试 | `frontend/tests/batchWriteModal.spec.ts` | mount + mock invoke，验证命中/忽略/禁用态/写入传参 |
| i18n | `frontend/src/i18n/locales/zh-CN.ts`、`en-US.ts` | 新增 `batchWrite.*` 块 + `table.batchWrite` |
| 接线 | `frontend/src/components/DataPointTable.vue` | 工具栏按钮 + 挂载 `<BatchWriteModal>` + 写入后刷新 |

---

## Task 1: `parseIoaExpression` 解析器

**Files:**
- Modify: `frontend/src/components/batchAdd/ioaRanges.ts`（末尾追加）
- Test: `frontend/tests/batchAdd/ioaRanges.spec.ts`（末尾追加）

- [ ] **Step 1: 追加失败测试**

在 `frontend/tests/batchAdd/ioaRanges.spec.ts` 顶部 import 行改为同时引入新函数：

```ts
import { compressRanges, lowerBound, findNextFreeGap, parseIoaExpression, resolveIoaHits, IOA_MAX } from '../../src/components/batchAdd/ioaRanges'
```

文件末尾追加：

```ts
describe('parseIoaExpression', () => {
  it('空串 → 空结果无错', () => {
    expect(parseIoaExpression('')).toEqual({ ranges: [], singles: [], error: null })
    expect(parseIoaExpression('   ')).toEqual({ ranges: [], singles: [], error: null })
  })

  it('单点 + 多分隔符（逗号/空格/换行）', () => {
    expect(parseIoaExpression('100, 200 300\n400')).toEqual({
      ranges: [], singles: [100, 200, 300, 400], error: null,
    })
  })

  it('区间', () => {
    expect(parseIoaExpression('1000-2000')).toEqual({
      ranges: [[1000, 2000]], singles: [], error: null,
    })
  })

  it('单点与区间混合 + 单点去重排序', () => {
    expect(parseIoaExpression('5000, 100, 100, 1000-2000')).toEqual({
      ranges: [[1000, 2000]], singles: [100, 5000], error: null,
    })
  })

  it('等值区间 a-a 合法', () => {
    expect(parseIoaExpression('5-5')).toEqual({ ranges: [[5, 5]], singles: [], error: null })
  })

  it('非数字 token → error 置该 token', () => {
    expect(parseIoaExpression('100, abc').error).toBe('abc')
  })

  it('区间反向 b<a → error', () => {
    expect(parseIoaExpression('200-100').error).toBe('200-100')
  })

  it('单点越域 > IOA_MAX → error', () => {
    expect(parseIoaExpression(String(IOA_MAX + 1)).error).toBe(String(IOA_MAX + 1))
  })

  it('区间上界越域 → error', () => {
    expect(parseIoaExpression('0-99999999').error).toBe('0-99999999')
  })

  it('带空格的破折号视为非法 token', () => {
    expect(parseIoaExpression('100 - 200').error).toBe('-')
  })
})
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npx vitest run tests/batchAdd/ioaRanges.spec.ts`
Expected: FAIL —— `parseIoaExpression is not a function` / 未导出。

- [ ] **Step 3: 实现 `parseIoaExpression`**

在 `frontend/src/components/batchAdd/ioaRanges.ts` 末尾追加（`IOA_MAX`、`lowerBound` 已在本文件定义）：

```ts
export interface IoaExpr {
  ranges: Array<[number, number]>  // 已校验 lo<=hi 且 hi<=IOA_MAX
  singles: number[]                // 升序去重
  error: string | null             // 非 null = 语法错（值为肇事 token），调用方据此禁用写入
}

// 解析 IOA 表达式：逗号/空格/换行分隔；单点 `n`、闭区间 `a-b`。
// 任一非法 token（非数字、b<a、越域）立即返回 error=该 token。
export function parseIoaExpression(input: string): IoaExpr {
  const ranges: Array<[number, number]> = []
  const singles = new Set<number>()
  const tokens = input.split(/[\s,]+/).filter((t) => t.length > 0)
  for (const tok of tokens) {
    const m = tok.match(/^(\d+)-(\d+)$/)
    if (m) {
      const lo = Number(m[1])
      const hi = Number(m[2])
      if (lo > hi || hi > IOA_MAX) {
        return { ranges, singles: sortedUnique(singles), error: tok }
      }
      ranges.push([lo, hi])
      continue
    }
    if (/^\d+$/.test(tok)) {
      const n = Number(tok)
      if (n > IOA_MAX) return { ranges, singles: sortedUnique(singles), error: tok }
      singles.add(n)
      continue
    }
    return { ranges, singles: sortedUnique(singles), error: tok }
  }
  return { ranges, singles: sortedUnique(singles), error: null }
}

function sortedUnique(s: Set<number>): number[] {
  return Array.from(s).sort((a, b) => a - b)
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/batchAdd/ioaRanges.spec.ts`
Expected: PASS（含原有 compressRanges/lowerBound/findNextFreeGap 用例）。

- [ ] **Step 5: 提交**

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add frontend/src/components/batchAdd/ioaRanges.ts frontend/tests/batchAdd/ioaRanges.spec.ts
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "feat(slave): IOA 表达式解析器 parseIoaExpression"
```

---

## Task 2: `resolveIoaHits` 命中求交

**Files:**
- Modify: `frontend/src/components/batchAdd/ioaRanges.ts`（末尾追加）
- Test: `frontend/tests/batchAdd/ioaRanges.spec.ts`（末尾追加）

- [ ] **Step 1: 追加失败测试**

末尾追加：

```ts
describe('resolveIoaHits', () => {
  const existing = [100, 1000, 1500, 2000] // 升序去重

  it('区间过滤已存在点（稀疏区间天然成立）', () => {
    const r = resolveIoaHits(parseIoaExpression('1000-2000'), existing)
    expect(r).toEqual({ hitIoas: [1000, 1500, 2000], missedSingles: [] })
  })

  it('单点命中 + 缺失单点计入 missed', () => {
    const r = resolveIoaHits(parseIoaExpression('100, 999'), existing)
    expect(r).toEqual({ hitIoas: [100], missedSingles: [999] })
  })

  it('区间与单点并集去重', () => {
    const r = resolveIoaHits(parseIoaExpression('1000-1500, 1000'), existing)
    expect(r).toEqual({ hitIoas: [1000, 1500], missedSingles: [] })
  })

  it('区间命中不计 missed（即便区间内多数 IOA 不存在）', () => {
    const r = resolveIoaHits(parseIoaExpression('0-100000'), existing)
    expect(r).toEqual({ hitIoas: [100, 1000, 1500, 2000], missedSingles: [] })
  })

  it('语法错 → 空命中', () => {
    const r = resolveIoaHits(parseIoaExpression('abc'), existing)
    expect(r).toEqual({ hitIoas: [], missedSingles: [] })
  })

  it('空 existing → 单点全 missed、区间空', () => {
    const r = resolveIoaHits(parseIoaExpression('100, 1-9'), [])
    expect(r).toEqual({ hitIoas: [], missedSingles: [100] })
  })
})
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npx vitest run tests/batchAdd/ioaRanges.spec.ts`
Expected: FAIL —— `resolveIoaHits is not a function`。

- [ ] **Step 3: 实现 `resolveIoaHits`**

在 `frontend/src/components/batchAdd/ioaRanges.ts` 末尾追加（依赖已有 `lowerBound` 与 Task 1 的 `IoaExpr`）：

```ts
export interface IoaHits {
  hitIoas: number[]        // 升序去重：实际存在且被表达式覆盖的 IOA
  missedSingles: number[]  // 升序去重：合法但不存在的单点（区间不计入）
}

// existingIoas 必须升序去重（区间用 lowerBound 二分切片）。
export function resolveIoaHits(expr: IoaExpr, existingIoas: readonly number[]): IoaHits {
  if (expr.error) return { hitIoas: [], missedSingles: [] }
  const existingSet = new Set(existingIoas)
  const hit = new Set<number>()
  const missed = new Set<number>()
  for (const n of expr.singles) {
    if (existingSet.has(n)) hit.add(n)
    else missed.add(n)
  }
  for (const [lo, hi] of expr.ranges) {
    const start = lowerBound(existingIoas, lo)
    const end = lowerBound(existingIoas, hi + 1)
    for (let i = start; i < end; i++) hit.add(existingIoas[i])
  }
  return {
    hitIoas: Array.from(hit).sort((a, b) => a - b),
    missedSingles: Array.from(missed).sort((a, b) => a - b),
  }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/batchAdd/ioaRanges.spec.ts`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add frontend/src/components/batchAdd/ioaRanges.ts frontend/tests/batchAdd/ioaRanges.spec.ts
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "feat(slave): IOA 命中求交 resolveIoaHits"
```

---

## Task 3: i18n 键（接口 + zh-CN + en-US）

**Files:**
- Modify: `frontend/src/i18n/locales/zh-CN.ts`
- Modify: `frontend/src/i18n/locales/en-US.ts`

> 字典文件结构：先是 `Messages` 接口（约 126–215 行的类型声明），后是各 locale 的实现对象。**接口与两个实现都要加**，否则 `vue-tsc` 报错。

- [ ] **Step 1: zh-CN 接口加键**

文件 `frontend/src/i18n/locales/zh-CN.ts`。

(a) 在 `table` 接口块里 `batchAdd: string` 之后加一行：

```ts
    batchWrite: string
```

(b) 在 `batchModal: {...}` 接口块**之后**插入新接口块：

```ts
  batchWrite: {
    title: string
    typeLabel: string
    ioaLabel: string
    ioaPlaceholder: string
    valueLabel: string
    hit: string
    ignored: string
    ignoredDetail: string
    parseError: string
    write: string
    writeN: string
    writing: string
    failedPrefix: string
    phSingle: string
    phDouble: string
    phStep: string
    phBitstring: string
    phNormalized: string
    phScaled: string
    phFloat: string
    phTotal: string
  }
```

- [ ] **Step 2: zh-CN 实现加键**

(a) 在 zh 实现的 `table:` 对象里 `batchAdd: '批量',` 之后加：

```ts
    batchWrite: '写值',
```

(b) 在 zh 实现的 `batchModal: {...}` 对象**之后**插入：

```ts
  batchWrite: {
    title: '按 IOA 批量写值',
    typeLabel: '类型',
    ioaLabel: '目标 IOA',
    ioaPlaceholder: '如 100, 500, 1000-2000, 5000（逗号/空格/换行分隔）',
    valueLabel: '值',
    hit: '命中 {count} 个',
    ignored: '忽略 {count} 个',
    ignoredDetail: '忽略 {ranges}（不存在）',
    parseError: '无法解析：{token}',
    write: '写入',
    writeN: '写入 {count}',
    writing: '写入中…',
    failedPrefix: '批量写值失败：{err}',
    phSingle: '1/0 或 ON/OFF',
    phDouble: '0/1/2/3',
    phStep: '-64..63',
    phBitstring: 'u32 位串（十进制）',
    phNormalized: '原始 NVA 整数 -32768..32767',
    phScaled: 'i16 整数 -32768..32767',
    phFloat: '如 99.9',
    phTotal: 'i32 整数',
  },
```

- [ ] **Step 3: en-US 实现加键**

文件 `frontend/src/i18n/locales/en-US.ts`。

(a) 在 `table:` 对象里 `batchAdd: 'Batch',` 之后加：

```ts
    batchWrite: 'Set',
```

(b) 在 `batchModal: {...}` 对象**之后**插入：

```ts
  batchWrite: {
    title: 'Batch Write by IOA',
    typeLabel: 'Type',
    ioaLabel: 'Target IOA',
    ioaPlaceholder: 'e.g. 100, 500, 1000-2000, 5000 (comma / space / newline)',
    valueLabel: 'Value',
    hit: '{count} matched',
    ignored: '{count} ignored',
    ignoredDetail: 'Ignored {ranges} (not present)',
    parseError: 'Cannot parse: {token}',
    write: 'Write',
    writeN: 'Write {count}',
    writing: 'Writing…',
    failedPrefix: 'Batch write failed: {err}',
    phSingle: '1/0 or ON/OFF',
    phDouble: '0/1/2/3',
    phStep: '-64..63',
    phBitstring: 'u32 bitstring (decimal)',
    phNormalized: 'raw NVA int -32768..32767',
    phScaled: 'i16 int -32768..32767',
    phFloat: 'e.g. 99.9',
    phTotal: 'i32 int',
  },
```

> en-US 文件若也带独立的 `Messages` 接口声明则同步加键；若是 `import type { Messages }` 复用 zh-CN 的接口，则只改实现对象即可（按文件实际情况判断）。

- [ ] **Step 4: 类型检查 + i18n 一致性测试**

Run: `cd frontend && npx vitest run tests/i18n.spec.ts`
Expected: PASS（zh/en 键集一致）。

- [ ] **Step 5: 提交**

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add frontend/src/i18n/locales/zh-CN.ts frontend/src/i18n/locales/en-US.ts
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "i18n(slave): batchWrite 文案 zh/en"
```

---

## Task 4: `BatchWriteModal.vue` 组件

**Files:**
- Create: `frontend/src/components/BatchWriteModal.vue`

- [ ] **Step 1: 写组件全文**

创建 `frontend/src/components/BatchWriteModal.vue`，内容如下（视觉与类名完全沿用 `BatchAddModal.vue`）：

```vue
<script setup lang="ts">
import { ref, computed, watch, inject } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { dialogKey } from '@shared/composables/useDialog'
import type { showAlert as ShowAlert } from '@shared/composables/useDialog'
import { useI18n } from '@shared/i18n'
import type { DataPointInfo } from '../types'
import { compressRanges, parseIoaExpression, resolveIoaHits } from './batchAdd/ioaRanges'

const { t } = useI18n()
const { showAlert } = inject<{ showAlert: typeof ShowAlert }>(dialogKey)!

interface Props {
  visible: boolean
  serverId: string
  commonAddress: number
  // 父级传入本站全部点（IOA 升序），(ioa, asdu_type) 在上游唯一。
  existingPoints: ReadonlyArray<Pick<DataPointInfo, 'ioa' | 'asdu_type' | 'category'>>
  // 默认选中类型（取当前表格分类对应类型）；空则取首个可用类型。
  defaultType?: string
}
const props = defineProps<Props>()
const emit = defineEmits<{ close: []; written: [] }>()

const asduType = ref('')
const ioaText = ref('')
const value = ref('')
const isSaving = ref(false)

// 本站实际存在的 asdu_type（去重），带其中文 category 作下拉 label。
const typeOptions = computed(() => {
  const seen = new Map<string, string>()
  for (const p of props.existingPoints) {
    if (!seen.has(p.asdu_type)) seen.set(p.asdu_type, p.category)
  }
  return Array.from(seen, ([type, category]) => ({ type, category }))
    .sort((a, b) => a.type.localeCompare(b.type))
})

// 选定类型下的已存在 IOA，升序去重（喂给 resolveIoaHits）。
const existingIoas = computed<number[]>(() => {
  const xs = props.existingPoints
    .filter((p) => p.asdu_type === asduType.value)
    .map((p) => p.ioa)
  xs.sort((a, b) => a - b)
  return xs
})

const parsed = computed(() => parseIoaExpression(ioaText.value))
const hits = computed(() => resolveIoaHits(parsed.value, existingIoas.value))

const hasExpr = computed(() => ioaText.value.trim().length > 0)
const parseError = computed(() => parsed.value.error)
const hitCount = computed(() => hits.value.hitIoas.length)
const missedCount = computed(() => hits.value.missedSingles.length)
const hitRangesText = computed(() => compressRanges(hits.value.hitIoas))
const missedRangesText = computed(() => compressRanges(hits.value.missedSingles))

const canWrite = computed(
  () => !isSaving.value && !parseError.value && hitCount.value > 0 && value.value.trim().length > 0,
)

function valuePlaceholder(type: string): string {
  if (/^M_SP_/.test(type)) return t('batchWrite.phSingle')
  if (/^M_DP_/.test(type)) return t('batchWrite.phDouble')
  if (/^M_ST_/.test(type)) return t('batchWrite.phStep')
  if (/^M_BO_/.test(type)) return t('batchWrite.phBitstring')
  if (/^M_ME_(NA|ND|TD)/.test(type)) return t('batchWrite.phNormalized')
  if (/^M_ME_(NB|TE)/.test(type)) return t('batchWrite.phScaled')
  if (/^M_ME_(NC|TF)/.test(type)) return t('batchWrite.phFloat')
  if (/^M_IT_/.test(type)) return t('batchWrite.phTotal')
  return ''
}

// immediate: true —— 组件常驻挂载（visible 初始 false），需在每次 visible 转 true
// 时初始化；且测试以 visible=true 挂载，无 immediate 则 watch 不触发、asduType 永空。
watch(
  () => props.visible,
  (v) => {
    if (v) {
      ioaText.value = ''
      value.value = ''
      isSaving.value = false
      asduType.value = props.defaultType || typeOptions.value[0]?.type || ''
    }
  },
  { immediate: true },
)

async function handleWrite() {
  if (!canWrite.value) return
  isSaving.value = true
  try {
    const points = hits.value.hitIoas.map((ioa) => ({ ioa, asdu_type: asduType.value }))
    await invoke('batch_update_data_points', {
      serverId: props.serverId,
      commonAddress: props.commonAddress,
      points,
      value: value.value,
    })
    emit('written')
  } catch (e) {
    await showAlert(t('batchWrite.failedPrefix', { err: String(e) }))
  } finally {
    isSaving.value = false
  }
}

function handleBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains('modal-backdrop')) emit('close')
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close')
  } else if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
    e.preventDefault()
    handleWrite()
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-pop">
      <div v-if="visible" class="modal-backdrop dialog-blur" @click="handleBackdropClick" @keydown="handleKeydown">
        <div class="modal">
          <div class="modal-header">
            <span class="modal-title">{{ t('batchWrite.title') }}</span>
            <button class="btn-close" @click="$emit('close')">×</button>
          </div>

          <div class="modal-body">
            <div class="form-group">
              <label class="form-label">{{ t('batchWrite.typeLabel') }}</label>
              <select v-model="asduType" class="form-select">
                <option v-for="opt in typeOptions" :key="opt.type" :value="opt.type">
                  {{ opt.category }} · {{ opt.type }}
                </option>
              </select>
            </div>

            <div class="form-group">
              <label class="form-label">{{ t('batchWrite.ioaLabel') }}</label>
              <textarea
                v-model="ioaText"
                class="form-input ioa-textarea"
                rows="3"
                :placeholder="t('batchWrite.ioaPlaceholder')"
              />
              <div v-if="hasExpr" class="summary-card">
                <div v-if="parseError" class="summary-card__conflict no-border">
                  {{ t('batchWrite.parseError', { token: parseError }) }}
                </div>
                <template v-else>
                  <div class="summary-card__title">
                    <span class="summary-card__count hit-count">{{ t('batchWrite.hit', { count: hitCount }) }}</span>
                    <template v-if="missedCount > 0">
                      <span class="summary-card__sep">·</span>
                      <span class="summary-card__count">{{ t('batchWrite.ignored', { count: missedCount }) }}</span>
                    </template>
                  </div>
                  <div v-if="hitCount > 0" class="summary-card__ranges">
                    <span class="summary-card__ranges-label">IOA</span>
                    <span class="summary-card__ranges-value">{{ hitRangesText }}</span>
                  </div>
                  <div v-if="missedCount > 0" class="summary-card__conflict">
                    {{ t('batchWrite.ignoredDetail', { ranges: missedRangesText }) }}
                  </div>
                </template>
              </div>
            </div>

            <div class="form-group">
              <label class="form-label">{{ t('batchWrite.valueLabel') }}</label>
              <input v-model="value" type="text" class="form-input" :placeholder="valuePlaceholder(asduType)" />
            </div>
          </div>

          <div class="modal-footer">
            <button class="btn btn-secondary" :disabled="isSaving" @click="$emit('close')">
              {{ t('common.cancel') }}
            </button>
            <button class="btn btn-primary" :disabled="!canWrite" @click="handleWrite">
              {{ isSaving ? t('batchWrite.writing') : hitCount > 0 ? t('batchWrite.writeN', { count: hitCount }) : t('batchWrite.write') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.modal {
  background: var(--c-base);
  border: 1px solid var(--c-surface1);
  border-radius: 8px;
  width: 420px;
  max-width: 90vw;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--c-surface0);
}

.modal-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--c-text);
}

.btn-close {
  background: none;
  border: none;
  color: var(--c-overlay0);
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}

.btn-close:hover {
  color: var(--c-text);
}

.modal-body {
  padding: 20px;
}

.form-group {
  margin-bottom: 16px;
}

.form-label {
  display: block;
  font-size: 13px;
  color: var(--c-overlay0);
  margin-bottom: 6px;
}

.form-input,
.form-select {
  width: 100%;
  padding: 8px 12px;
  background: var(--c-crust);
  border: 1px solid var(--c-surface1);
  border-radius: 6px;
  color: var(--c-text);
  font-size: 14px;
  box-sizing: border-box;
}

.form-input:focus,
.form-select:focus {
  outline: none;
  border-color: var(--c-blue);
}

.ioa-textarea {
  font-family: var(--font-mono);
  resize: vertical;
  min-height: 64px;
  line-height: 1.5;
}

.summary-card {
  margin-top: 6px;
  padding: 10px 12px;
  background: var(--c-mantle);
  border: 1px solid var(--c-surface1);
  border-radius: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.summary-card__title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.summary-card__sep {
  color: var(--c-overlay0);
}

.summary-card__count {
  color: var(--c-subtext0);
}

.hit-count {
  color: var(--c-green);
  font-weight: 600;
}

.summary-card__ranges {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 12px;
}

.summary-card__ranges-label {
  color: var(--c-overlay0);
}

.summary-card__ranges-value {
  font-family: var(--font-mono);
  color: var(--c-text);
  word-break: break-all;
}

.summary-card__conflict {
  margin-top: 4px;
  padding-top: 6px;
  border-top: 1px dashed var(--c-red);
  color: var(--c-red);
  font-size: 12px;
  font-family: var(--font-mono);
}

.summary-card__conflict.no-border {
  margin-top: 0;
  padding-top: 0;
  border-top: none;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 16px 20px;
  border-top: 1px solid var(--c-surface0);
}

.btn {
  padding: 8px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.btn-primary {
  background: var(--c-blue);
  color: var(--c-base);
  font-weight: 600;
}

.btn-primary:hover {
  background: var(--c-sapphire);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  background: var(--c-surface1);
  color: var(--c-text);
}

.btn-secondary:hover {
  background: var(--c-surface2);
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
```

- [ ] **Step 2: 类型检查通过**

Run: `cd frontend && npx vue-tsc -b --noEmit`
Expected: 无 BatchWriteModal 相关报错（i18n 键已在 Task 3 加好）。

> 注：项目正式验证用 `npm run build`（含 vue-tsc + vite），此处先用 `vue-tsc -b` 快速查类型；完整 build 在 Task 7。

- [ ] **Step 3: 提交**

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add frontend/src/components/BatchWriteModal.vue
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "feat(slave): BatchWriteModal 组件"
```

---

## Task 5: 组件测试（mount + mock invoke）

**Files:**
- Create: `frontend/tests/batchWriteModal.spec.ts`

> 组件用 `<Teleport to="body">`，测试里用 `global.stubs.teleport: true` 让内容渲染到 wrapper 内可查。i18n 在测试中返回真实译文，故只断言**结构 / 禁用态 / compressRanges 文本 / invoke 入参**这些与译文无关的部分。

- [ ] **Step 1: 写测试**

创建 `frontend/tests/batchWriteModal.spec.ts`：

```ts
// slave-batch-write-by-ioa：BatchWriteModal 命中/忽略/禁用/写入。
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { dialogKey } from '@shared/composables/useDialog'
import BatchWriteModal from '../src/components/BatchWriteModal.vue'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }))

const existing = [
  { ioa: 100, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 1000, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 1500, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 2000, asdu_type: 'M_ME_NC_1', category: '短浮点测量' },
  { ioa: 5, asdu_type: 'M_SP_NA_1', category: '单点信息' },
]

function mountModal() {
  return mount(BatchWriteModal, {
    props: { visible: true, serverId: 's1', commonAddress: 1, existingPoints: existing, defaultType: 'M_ME_NC_1' },
    global: {
      stubs: { teleport: true },
      provide: { [dialogKey as symbol]: { showAlert: () => Promise.resolve() } },
    },
  })
}

const writeBtn = (w: ReturnType<typeof mountModal>) => w.find('.btn-primary').element as HTMLButtonElement

describe('BatchWriteModal', () => {
  beforeEach(() => invokeMock.mockReset())

  it('区间命中：1000-2000 命中 3 点，命中区间文本正确', async () => {
    const w = mountModal()
    await w.find('.ioa-textarea').setValue('1000-2000')
    expect(w.find('.summary-card__ranges-value').text()).toBe('1000–2000')
    expect(w.find('.summary-card__conflict').exists()).toBe(false)
  })

  it('单点缺失：100, 999 → 命中 100、忽略 999', async () => {
    const w = mountModal()
    await w.find('.ioa-textarea').setValue('100, 999')
    expect(w.find('.summary-card__ranges-value').text()).toBe('100')
    expect(w.find('.summary-card__conflict').text()).toContain('999')
  })

  it('语法错：abc → 显示 parseError、写入禁用', async () => {
    const w = mountModal()
    await w.find('.ioa-textarea').setValue('abc')
    expect(w.find('.summary-card__conflict.no-border').exists()).toBe(true)
    await w.find('input[type="text"]').setValue('99.9')
    expect(writeBtn(w).disabled).toBe(true)
  })

  it('0 命中 / 空值 → 写入禁用', async () => {
    const w = mountModal()
    // 有命中但值为空 → 禁用
    await w.find('.ioa-textarea').setValue('1000-2000')
    expect(writeBtn(w).disabled).toBe(true)
    // 值有了但表达式 0 命中 → 禁用
    await w.find('input[type="text"]').setValue('99.9')
    await w.find('.ioa-textarea').setValue('99999')
    expect(writeBtn(w).disabled).toBe(true)
  })

  it('正常写入：点击触发 batch_update_data_points 并带显式 points + 值', async () => {
    invokeMock.mockResolvedValue(3)
    const w = mountModal()
    await w.find('.ioa-textarea').setValue('1000-2000')
    await w.find('input[type="text"]').setValue('99.9')
    expect(writeBtn(w).disabled).toBe(false)
    await w.find('.btn-primary').trigger('click')
    expect(invokeMock).toHaveBeenCalledWith('batch_update_data_points', {
      serverId: 's1',
      commonAddress: 1,
      value: '99.9',
      points: [
        { ioa: 1000, asdu_type: 'M_ME_NC_1' },
        { ioa: 1500, asdu_type: 'M_ME_NC_1' },
        { ioa: 2000, asdu_type: 'M_ME_NC_1' },
      ],
    })
    expect(w.emitted('written')).toBeTruthy()
  })
})
```

- [ ] **Step 2: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/batchWriteModal.spec.ts`
Expected: PASS（5 个用例）。
若 Teleport 内容查不到，确认 `global.stubs.teleport: true` 已设。

- [ ] **Step 3: 提交**

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add frontend/tests/batchWriteModal.spec.ts
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "test(slave): BatchWriteModal 组件测试"
```

---

## Task 6: 接入 `DataPointTable`

**Files:**
- Modify: `frontend/src/components/DataPointTable.vue`

- [ ] **Step 1: import + 状态 + 默认类型 + 写入后回调**

(a) 在 import 区 `import BatchAddModal from './BatchAddModal.vue'`（第 8 行）之后加：

```ts
import BatchWriteModal from './BatchWriteModal.vue'
```

(b) 在 `const showBatchModal = ref(false)`（第 45 行）之后加：

```ts
const showBatchWriteModal = ref(false)
// 默认写值类型：取当前分类过滤命中的首个点的 asdu_type；无过滤则空（弹窗回退首个可用类型）。
const batchWriteDefaultType = computed(() => {
  if (!selectedCategory.value) return ''
  const p = displayPoints.value.find((pt) => pt.category === selectedCategory.value)
  return p?.asdu_type ?? ''
})
```

(c) 在 `function onPointAdded` 附近（或任意函数区）加写入后回调：

```ts
function onBatchWritten() {
  showBatchWriteModal.value = false
  loadDataPoints()
}
```

> `loadDataPoints` 已在本组件定义（增量拉取并触发行闪烁）。若该函数名在本文件实际为别名，按文件内实际名称调用。

- [ ] **Step 2: 工具栏加按钮**

在「批量添加」按钮（约 578–583 行那段 `<button class="add-btn batch" ...>`）**之后**插入：

```html
      <button
        class="add-btn batch"
        :disabled="!selectedServerId || currentCA === null || displayPoints.length === 0"
        @click="showBatchWriteModal = true"
        :title="t('batchWrite.title')"
      >{{ t('table.batchWrite') }}</button>
```

- [ ] **Step 3: 挂载弹窗**

在 `<BatchAddModal ... />`（约 749–756 行）**之后**插入：

```html
    <!-- Batch Write Modal -->
    <BatchWriteModal
      :visible="showBatchWriteModal"
      :server-id="selectedServerId ?? ''"
      :common-address="currentCA ?? 0"
      :existing-points="showBatchWriteModal ? displayPoints : []"
      :default-type="batchWriteDefaultType"
      @close="showBatchWriteModal = false"
      @written="onBatchWritten"
    />
```

- [ ] **Step 4: 类型检查**

Run: `cd frontend && npx vue-tsc -b --noEmit`
Expected: 无报错。

- [ ] **Step 5: 现有测试回归**

Run: `cd frontend && npm run test`
Expected: 全绿（含原有 dataPointTable / valuePanelBatch 等用例）。

- [ ] **Step 6: 提交**

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add frontend/src/components/DataPointTable.vue
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "feat(slave): DataPointTable 接入批量写值弹窗"
```

---

## Task 7: 全量构建 + 无头浏览器冒烟

**Files:** 无新增（验证步骤）

- [ ] **Step 1: 全量构建（正式类型 + 打包门槛）**

Run: `cd frontend && npm run build`
Expected: `vue-tsc -b` 类型零错 + `vite build` 成功产出 `dist/`。
> 这是前端的权威验证（非 `vue-tsc --noEmit`）；shared/Tauri import 约束在此暴露。

- [ ] **Step 2: 全量单测**

Run: `cd frontend && npm run test`
Expected: 全部 PASS（ioaRanges / batchWriteModal / i18n / 既有用例）。

- [ ] **Step 3: 无头浏览器冒烟（Playwright MCP，注入 Tauri mock）**

> 项目无 Playwright 测试基建；用 Playwright MCP 浏览器工具对 `vite` 预览做真实浏览器冒烟（jsdom 不足以覆盖渲染）。Tauri `invoke` 在浏览器无后端，需注入 mock（参考 `scripts/screenshots/capture.mjs` 的注入方式）。

具体步骤：
1. 起预览：`cd frontend && npm run build && npx vite preview --port 4173`（后台运行）。
2. `browser_navigate` 打开 `http://localhost:4173`。
3. 用 `browser_evaluate` 注入 `window.__TAURI_INTERNALS__` 的 `invoke` mock：
   - `list_data_points_since` → 返回若干 `M_ME_NC_1` 点（IOA 如 100/1000/1500/2000）；
   - `batch_update_data_points` → 记录入参并返回命中数。
4. 选中一个站 → 工具栏点「写值」按钮 → 弹窗出现。
5. 类型选 `M_ME_NC_1`，IOA 输入 `1000-2000, 99999`，确认卡片显示「命中 3 · 忽略 1」、忽略行含 `99999`。
6. 值填 `88.8` → 点「写入 3」→ 断言 mock 收到 `batch_update_data_points`，`points` 为 3 个 `{ioa, asdu_type:'M_ME_NC_1'}`、`value:'88.8'`，弹窗关闭。
7. `browser_take_screenshot` 留档（暗色：等 `body` 背景就绪）。
8. 关预览进程。

Expected: 弹窗渲染正常、命中/忽略计数正确、写入传参正确、视觉与 BatchAddModal 一致。

- [ ] **Step 4: 收尾提交（如冒烟中有微调）**

若 Step 3 暴露样式/交互小问题，定点修复后：

```bash
cd /Users/daichangyu/Developer/IEC60870-5-104-Simulator
git add -A
git commit --author='Karl-Dai Karl <kelsoprotein@gmail.com>' -m "fix(slave): 批量写值弹窗冒烟修正"
```

---

## 自查（Spec 覆盖）

- 文本 IOA 表达式指定散点 → Task 1（解析）+ Task 4（文本框）。✅
- 弹窗内先选具体 asdu_type，默认当前分类 → Task 4（`typeOptions`/`defaultType`）+ Task 6（`batchWriteDefaultType`）。✅
- 区间=过滤已存在点、单点=精确 → Task 2（`resolveIoaHits` 二分切片 + 单点 Set 命中）。✅
- 缺失单点静默跳过 + 计数、0 命中禁用 → Task 2（`missedSingles`）+ Task 4（`canWrite`/命中卡片）。✅
- 语法错（非数字/`b<a`/越域）红色提示 + 禁用 → Task 1（`error`）+ Task 4（`.summary-card__conflict.no-border`/`canWrite`）。✅
- 前端解析 + 复用 `batch_update_data_points`、零后端改动 → Task 4（`handleWrite`）。✅
- 值占位随类型、值由后端兜底（全或无、失败弹错保留弹窗）→ Task 4（`valuePlaceholder`/`showAlert`）。✅
- 入口工具栏按钮、写入后刷新闪烁 → Task 6。✅
- i18n zh + en → Task 3。✅
- 测试：解析器/求交单测、组件测试、build、无头浏览器冒烟 → Task 1/2/5/7。✅

**范围之外（未建任务，符合设计）**：跨多类型同写、按值/品质过滤选点、每点不同值、表达式预设、批量改品质。
