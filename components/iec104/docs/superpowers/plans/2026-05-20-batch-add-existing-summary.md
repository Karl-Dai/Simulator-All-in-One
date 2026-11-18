# 批量添加点位 — 汇总卡片 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 BatchAddModal 里的"已有点位"脚注升级为常驻汇总卡片,并加两个"一键避让冲突"按钮 + 冲突 IOA 具体范围显示。

**Architecture:** 把两个无副作用的纯算法(范围压缩 `compressRanges`、空隙搜索 `findNextFreeGap`)抽到独立 helper 文件,先用 vitest 单测覆盖;再把 BatchAddModal 的模板/脚本/样式按设计稿替换。i18n 同步增 5 个 key。

**Tech Stack:** Vue 3 setup script + TypeScript,Tauri 命令通过 `@tauri-apps/api`,vitest 单元测试,无新依赖。

设计稿来源: `docs/superpowers/specs/2026-05-20-batch-add-existing-summary-design.md`(commit `e702616`)。

---

## 文件结构

- **新建** `frontend/src/components/batchAdd/ioaRanges.ts` — 纯函数:`compressRanges`、`lowerBound`、`findNextFreeGap`。无 Vue 依赖,易测。
- **新建** `frontend/tests/batchAdd/ioaRanges.spec.ts` — vitest 单测,覆盖上述三个函数。
- **修改** `frontend/src/components/BatchAddModal.vue` — 用 helper 替换内联实现,加 `conflictRanges` computed,加两个 action 方法,模板换成 `.summary-card` 结构,样式追加 / 替换。
- **修改** `frontend/src/i18n/locales/zh-CN.ts` — 顶部 `DictShape` 接口和下方 `dict` 字面量各增 5 个 key。
- **修改** `frontend/src/i18n/locales/en-US.ts` — `dict` 字面量增同样 5 个 key。

IOA 上限常量 `IOA_MAX = 16777215`(3 字节 IOA 上限)放在 `ioaRanges.ts` 顶部并导出。

---

## Task 1: 抽出并测试 `compressRanges` 与 `lowerBound`

**Files:**
- Create: `frontend/src/components/batchAdd/ioaRanges.ts`
- Create: `frontend/tests/batchAdd/ioaRanges.spec.ts`

- [ ] **Step 1: 写失败测试**

写入 `frontend/tests/batchAdd/ioaRanges.spec.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { compressRanges, lowerBound } from '../../src/components/batchAdd/ioaRanges'

describe('compressRanges', () => {
  it('returns empty string for []', () => {
    expect(compressRanges([])).toBe('')
  })

  it('handles single value', () => {
    expect(compressRanges([5])).toBe('5')
  })

  it('compresses one contiguous run', () => {
    expect(compressRanges([0, 1, 2, 3])).toBe('0–3')
  })

  it('compresses multiple runs with gaps', () => {
    expect(compressRanges([0, 1, 2, 5, 7, 8])).toBe('0–2, 5, 7–8')
  })

  it('keeps singletons as singletons', () => {
    expect(compressRanges([1, 3, 5])).toBe('1, 3, 5')
  })
})

describe('lowerBound', () => {
  it('returns 0 when target ≤ first element', () => {
    expect(lowerBound([10, 20, 30], 5)).toBe(0)
    expect(lowerBound([10, 20, 30], 10)).toBe(0)
  })

  it('returns length when target > last element', () => {
    expect(lowerBound([10, 20, 30], 100)).toBe(3)
  })

  it('returns index of first element ≥ target', () => {
    expect(lowerBound([10, 20, 30], 20)).toBe(1)
    expect(lowerBound([10, 20, 30], 25)).toBe(2)
  })

  it('handles empty input', () => {
    expect(lowerBound([], 5)).toBe(0)
  })
})
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm test -- tests/batchAdd/ioaRanges.spec.ts`
Expected: FAIL,因为模块不存在 / 没导出符号。

- [ ] **Step 3: 最小实现**

写入 `frontend/src/components/batchAdd/ioaRanges.ts`:

```ts
// 3-byte IOA upper bound per IEC 60870-5-101 §7.2.5.
export const IOA_MAX = 16_777_215

// Assumes xs is sorted ascending and unique.
export function compressRanges(xs: readonly number[]): string {
  if (xs.length === 0) return ''
  const fmt = (s: number, e: number) => (s === e ? String(s) : `${s}–${e}`)
  const parts: string[] = []
  let s = xs[0]
  let e = xs[0]
  for (let i = 1; i < xs.length; i++) {
    if (xs[i] === e + 1) {
      e = xs[i]
      continue
    }
    parts.push(fmt(s, e))
    s = e = xs[i]
  }
  parts.push(fmt(s, e))
  return parts.join(', ')
}

// Index of first element ≥ target in a sorted array.
export function lowerBound(xs: readonly number[], target: number): number {
  let l = 0
  let r = xs.length
  while (l < r) {
    const m = (l + r) >>> 1
    if (xs[m] < target) l = m + 1
    else r = m
  }
  return l
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm test -- tests/batchAdd/ioaRanges.spec.ts`
Expected: PASS — 9 个用例全绿。

- [ ] **Step 5: 提交**

```bash
git add frontend/src/components/batchAdd/ioaRanges.ts frontend/tests/batchAdd/ioaRanges.spec.ts
git commit -m "test(frontend): 抽出 IOA 范围压缩与二分,单测覆盖"
```

---

## Task 2: 实现并测试 `findNextFreeGap`

**Files:**
- Modify: `frontend/src/components/batchAdd/ioaRanges.ts`
- Modify: `frontend/tests/batchAdd/ioaRanges.spec.ts`

- [ ] **Step 1: 追加失败测试**

在 `frontend/tests/batchAdd/ioaRanges.spec.ts` 末尾追加:

```ts
import { findNextFreeGap, IOA_MAX } from '../../src/components/batchAdd/ioaRanges'

describe('findNextFreeGap', () => {
  it('returns 0 when there are no existing points', () => {
    expect(findNextFreeGap([], 10)).toBe(0)
  })

  it('returns the gap before the first range when it fits', () => {
    // existing 50..59, want 20 — fits at 0..19
    expect(findNextFreeGap([50, 51, 52, 53, 54, 55, 56, 57, 58, 59], 20)).toBe(0)
  })

  it('skips past first range when count would overlap it', () => {
    // existing 0..9, want 5 — 0..4 overlaps, so jump to 10
    expect(findNextFreeGap([0, 1, 2, 3, 4, 5, 6, 7, 8, 9], 5)).toBe(10)
  })

  it('finds the inner gap when first slot too small', () => {
    // existing 0..9 and 50..59, want 20 — gap 10..49 fits → 10
    const xs = [...Array(10).keys(), ...Array.from({ length: 10 }, (_, i) => 50 + i)]
    expect(findNextFreeGap(xs, 20)).toBe(10)
  })

  it('jumps past both ranges when no inner gap fits', () => {
    // existing 0..9 and 50..59, want 60 — 10..49 too small, jump to 60
    const xs = [...Array(10).keys(), ...Array.from({ length: 10 }, (_, i) => 50 + i)]
    expect(findNextFreeGap(xs, 60)).toBe(60)
  })

  it('returns null when result would exceed IOA_MAX', () => {
    // last existing point at IOA_MAX, count 1 → would land at IOA_MAX+1, no room
    expect(findNextFreeGap([IOA_MAX], 1)).toBeNull()
  })

  it('returns 0 with count = 1 and existing [5]', () => {
    expect(findNextFreeGap([5], 1)).toBe(0)
  })
})
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm test -- tests/batchAdd/ioaRanges.spec.ts`
Expected: FAIL,`findNextFreeGap` 未导出。

- [ ] **Step 3: 在 `ioaRanges.ts` 末尾追加实现**

```ts
// Smallest s ≥ 0 such that [s, s+count-1] is disjoint from xs.
// xs must be sorted ascending. Returns null if result would exceed IOA_MAX.
export function findNextFreeGap(xs: readonly number[], count: number): number | null {
  if (count <= 0) return 0
  let s = 0
  for (const x of xs) {
    if (x < s) continue
    if (x <= s + count - 1) {
      s = x + 1
      continue
    }
    break
  }
  if (s + count - 1 > IOA_MAX) return null
  return s
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm test -- tests/batchAdd/ioaRanges.spec.ts`
Expected: PASS — 16 个用例全绿。

- [ ] **Step 5: 提交**

```bash
git add frontend/src/components/batchAdd/ioaRanges.ts frontend/tests/batchAdd/ioaRanges.spec.ts
git commit -m "feat(frontend): findNextFreeGap 空隙搜索 + 测试"
```

---

## Task 3: 新增 i18n key

**Files:**
- Modify: `frontend/src/i18n/locales/zh-CN.ts` (interface 在 ~154,字面量在 ~393)
- Modify: `frontend/src/i18n/locales/en-US.ts` (字面量在 ~156)

- [ ] **Step 1: 在 `zh-CN.ts` 的 `DictShape.batchModal` 接口里加 5 个 key**

把第 154–168 行的 `batchModal` 接口改为:

```ts
  batchModal: {
    title: string
    startIoa: string
    count: string
    asduTypeLabel: string
    namePrefix: string
    namePrefixPlaceholder: string
    countWarn: string
    rangeHint: string
    existingSameType: string
    conflictWarn: string
    saving: string
    add: string
    failedPrefix: string
    summaryEmpty: string
    nextIoaBtn: string
    nextGapBtn: string
    nextIoaTooltipEmpty: string
    capacityFullTooltip: string
    conflictDetail: string
  }
```

(`conflictWarn` 仍保留,不再使用,但 zh-CN 类型一致性优先;Task 4 会从模板中移除它的使用。我们暂时保留 key,等本 plan 全部完成后做不到的事可以单独清理。)

- [ ] **Step 2: 在 `zh-CN.ts` 的 `batchModal` 字面量里加 5 个 key**

把第 393–407 行的 `batchModal` 字面量改为:

```ts
  batchModal: {
    title: '批量添加数据点',
    startIoa: '起始 IOA',
    count: '数量',
    asduTypeLabel: 'ASDU 类型',
    namePrefix: '名称前缀（可选）',
    namePrefixPlaceholder: '如 SP → SP_0, SP_1, ...',
    countWarn: '范围过大（最多 100000）',
    rangeHint: 'IOA 范围：{startIoa} ~ {endIoa}，共将添加 {count} 个数据点',
    existingSameType: '已有 {count} 个同类型点位',
    conflictWarn: '与 {count} 个已存在 IOA 冲突，这些将被跳过',
    saving: '添加中...',
    add: '确认',
    failedPrefix: '批量添加失败：{err}',
    summaryEmpty: '该类型尚未添加任何点',
    nextIoaBtn: '↓ 下一个可用 IOA',
    nextGapBtn: '↦ 跳到能放下的空隙',
    nextIoaTooltipEmpty: '当前类型无现有点',
    capacityFullTooltip: 'IOA 容量不足',
    conflictDetail: '冲突 IOA {ranges}（将覆盖 {count} 个点）',
  },
```

- [ ] **Step 3: 在 `en-US.ts` 的 `batchModal` 字面量加同样 5 个 key**

把第 156–170 行的 `batchModal` 改为:

```ts
  batchModal: {
    title: 'Batch Add Data Points',
    startIoa: 'Start IOA',
    count: 'Count',
    asduTypeLabel: 'ASDU Type',
    namePrefix: 'Name Prefix (optional)',
    namePrefixPlaceholder: 'e.g., SP → SP_0, SP_1, ...',
    countWarn: 'Range too large (max 100000)',
    rangeHint: 'IOA range: {startIoa} ~ {endIoa}, will add {count} points',
    existingSameType: '{count} existing point(s) of this type',
    conflictWarn: 'Conflicts with {count} existing IOA(s); they will be skipped',
    saving: 'Adding...',
    add: 'Confirm',
    failedPrefix: 'Batch add failed: {err}',
    summaryEmpty: 'No points of this type yet',
    nextIoaBtn: '↓ Next free IOA',
    nextGapBtn: '↦ Next fitting gap',
    nextIoaTooltipEmpty: 'No existing points to skip past',
    capacityFullTooltip: 'IOA capacity exhausted',
    conflictDetail: 'Conflicting IOA {ranges} (overwrites {count} points)',
  },
```

- [ ] **Step 4: 类型检查 + i18n 一致性测试**

Run: `cd frontend && npm run build`
Expected: vue-tsc 通过(类型对齐)。如果报错,通常是 en-US.ts 缺 key,补上即可。

Run: `cd frontend && npm test -- tests/i18n.spec.ts`
Expected: PASS — 现有 i18n 测试不应受影响。

- [ ] **Step 5: 提交**

```bash
git add frontend/src/i18n/locales/zh-CN.ts frontend/src/i18n/locales/en-US.ts
git commit -m "i18n(batch-modal): 新增汇总卡片相关 5 个 key (zh/en)"
```

---

## Task 4: 在 BatchAddModal 中接入 helper 并加新 computed / 方法

**Files:**
- Modify: `frontend/src/components/BatchAddModal.vue`(脚本部分,行 1–124)

仅改 `<script setup>` 部分,模板与样式留到 Task 5。这一步保持原渲染行为不变(还在用旧 `.existing-summary` + 旧 `conflict-warn`),只是把内联算法换成 helper、并加未渲染的新 computed。

- [ ] **Step 1: 引入 helper**

在第 2 行后插入 import(放到最后一个本地 import 之后):

```ts
import {
  IOA_MAX,
  compressRanges,
  lowerBound,
  findNextFreeGap,
} from './batchAdd/ioaRanges'
```

- [ ] **Step 2: 用 helper 替换 `existingRangesText`**

把第 55–68 行的 `existingRangesText` computed 改为:

```ts
const existingRangesText = computed<string>(() =>
  compressRanges(existingSameTypeIoas.value),
)
```

- [ ] **Step 3: 用 helper 替换 `conflictCount`**

把第 73–84 行的 `conflictCount` computed 改为:

```ts
const conflictCount = computed<number>(() => {
  const xs = existingSameTypeIoas.value
  if (xs.length === 0 || count.value <= 0 || startIoa.value < 0) return 0
  const lo = startIoa.value
  const hi = lo + count.value - 1
  return lowerBound(xs, hi + 1) - lowerBound(xs, lo)
})
```

- [ ] **Step 4: 新增 `conflictRanges` computed**

紧跟 `conflictCount` 之后加:

```ts
const conflictRanges = computed<string>(() => {
  const xs = existingSameTypeIoas.value
  if (xs.length === 0 || conflictCount.value === 0) return ''
  const lo = startIoa.value
  const hi = lo + count.value - 1
  const start = lowerBound(xs, lo)
  const end = lowerBound(xs, hi + 1)
  return compressRanges(xs.slice(start, end))
})
```

- [ ] **Step 5: 新增 quick-fill computed + action**

紧跟 `conflictRanges` 之后加:

```ts
const nextAvailableIoa = computed<number | null>(() => {
  const xs = existingSameTypeIoas.value
  if (xs.length === 0) return null
  const next = xs[xs.length - 1] + 1
  return next > IOA_MAX ? null : next
})

const nextFreeGapStart = computed<number | null>(() =>
  findNextFreeGap(existingSameTypeIoas.value, count.value),
)

const canApplyNextIoa = computed(() => nextAvailableIoa.value !== null)
const canApplyNextGap = computed(() => nextFreeGapStart.value !== null)

const nextIoaDisabledTooltip = computed(() => {
  if (existingSameTypeIoas.value.length === 0) return t('batchModal.nextIoaTooltipEmpty')
  if (nextAvailableIoa.value === null) return t('batchModal.capacityFullTooltip')
  return ''
})

const nextGapDisabledTooltip = computed(() =>
  nextFreeGapStart.value === null ? t('batchModal.capacityFullTooltip') : '',
)

function applyNextAvailableIoa() {
  if (nextAvailableIoa.value !== null) startIoa.value = nextAvailableIoa.value
}

function applyNextFreeGap() {
  if (nextFreeGapStart.value !== null) startIoa.value = nextFreeGapStart.value
}
```

- [ ] **Step 6: 类型检查**

Run: `cd frontend && npm run build`
Expected: 通过。如果 vue-tsc 抱怨,通常是 import 路径错或 t() 调用类型不匹配,按报错修。

- [ ] **Step 7: 跑现有测试套件,确保没把别处弄坏**

Run: `cd frontend && npm test --`
Expected: 全部 PASS。

- [ ] **Step 8: 提交**

```bash
git add frontend/src/components/BatchAddModal.vue
git commit -m "refactor(batch-modal): 接入 ioaRanges helper + 新增冲突详情 / 快捷填充 computed"
```

---

## Task 5: 替换模板为汇总卡片 + 样式

**Files:**
- Modify: `frontend/src/components/BatchAddModal.vue`(模板 + 样式)

- [ ] **Step 1: 改 ASDU 类型 form-group 内的汇总部分**

定位到旧模板:

```vue
            <div v-if="existingSameTypeIoas.length > 0" class="existing-summary">
              {{ t('batchModal.existingSameType', { count: existingSameTypeIoas.length }) }}
              <span class="ioa-ranges">IOA: {{ existingRangesText }}</span>
            </div>
```

替换为:

```vue
            <div class="summary-card">
              <div class="summary-card__title">
                <span class="summary-card__type">{{ formAsduType }}</span>
                <span class="summary-card__sep">·</span>
                <span class="summary-card__count">
                  <template v-if="existingSameTypeIoas.length > 0">
                    {{ t('batchModal.existingSameType', { count: existingSameTypeIoas.length }) }}
                  </template>
                  <template v-else>{{ t('batchModal.summaryEmpty') }}</template>
                </span>
              </div>
              <div v-if="existingSameTypeIoas.length > 0" class="summary-card__ranges">
                <span class="summary-card__ranges-label">IOA</span>
                <span class="summary-card__ranges-value">{{ existingRangesText }}</span>
              </div>
              <div class="summary-card__actions">
                <button
                  type="button"
                  class="summary-card__btn"
                  :disabled="!canApplyNextIoa"
                  :title="nextIoaDisabledTooltip"
                  @click="applyNextAvailableIoa"
                >
                  {{ t('batchModal.nextIoaBtn') }}
                </button>
                <button
                  type="button"
                  class="summary-card__btn"
                  :disabled="!canApplyNextGap"
                  :title="nextGapDisabledTooltip"
                  @click="applyNextFreeGap"
                >
                  {{ t('batchModal.nextGapBtn') }}
                </button>
              </div>
              <div v-if="conflictCount > 0" class="summary-card__conflict">
                {{ t('batchModal.conflictDetail', { ranges: conflictRanges, count: conflictCount }) }}
              </div>
            </div>
```

- [ ] **Step 2: 删除模板末尾独立的 `.conflict-warn` 块**

把模板里 `<div v-if="conflictCount > 0" class="conflict-warn">…</div>` 整行删掉(因为冲突信息已并入卡片)。

- [ ] **Step 3: 改样式 — 替换旧规则,追加新规则**

把 `<style scoped>` 中三个旧规则全部删除:`.existing-summary`、`.existing-summary .ioa-ranges`、`.conflict-warn`。

在 `.count-info` / `.count-warn` 块之后追加:

```css
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

.summary-card__type {
  font-weight: 600;
  color: var(--c-text);
}

.summary-card__sep {
  color: var(--c-overlay0);
}

.summary-card__count {
  color: var(--c-subtext0);
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

.summary-card__actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.summary-card__btn {
  padding: 4px 10px;
  font-size: 12px;
  background: var(--c-surface0);
  border: 1px solid var(--c-surface1);
  color: var(--c-text);
  border-radius: 4px;
  cursor: pointer;
}

.summary-card__btn:hover:not(:disabled) {
  background: var(--c-surface1);
}

.summary-card__btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.summary-card__conflict {
  margin-top: 4px;
  padding-top: 6px;
  border-top: 1px dashed var(--c-red);
  color: var(--c-red);
  font-size: 12px;
  font-family: var(--font-mono);
}
```

- [ ] **Step 4: 类型检查与构建**

Run: `cd frontend && npm run build`
Expected: vue-tsc 通过,vite build 成功。

- [ ] **Step 5: 跑测试**

Run: `cd frontend && npm test --`
Expected: 全部 PASS。

- [ ] **Step 6: 提交**

```bash
git add frontend/src/components/BatchAddModal.vue
git commit -m "feat(batch-modal): 已有点位汇总卡片 + 一键避让冲突按钮"
```

---

## Task 6: 人工 UI 烟雾测试

**Files:** 无代码改动,仅手工验证。

**重要**:本项目用户偏好 — 不要随便启动主/子站 GUI,优先无头测试。但本任务是纯 UI 改动,**必须**人工启动看一眼;让用户自己跑 `npm run dev` 或 `npm run tauri dev` 看效果,你不要自动启动 GUI。

- [ ] **Step 1: 请用户启动子站 dev 模式并依次验证下列场景**

在 plan 执行结尾告诉用户:

> 请运行 `cd frontend && npm run dev` (或 `npm run tauri dev`),打开「批量添加数据点」对话框,验证以下场景:
>
> 1. 新模型未加任何点 → 切到任意 ASDU 类型,卡片显示"该类型尚未添加任何点",两按钮都灰,hover「↓」按钮 tooltip 显示「当前类型无现有点」。
> 2. 已加 256 个 M_SP_NA_1 (IOA 0–255),切到 M_SP_NA_1 → 卡片显示"已有 256 个同类型点位",IOA 行显示 `0–255`,两按钮可点。按「↓」,startIoa 变 256;按「↦」(count=10),startIoa 也变 256。
> 3. 设 count=20,先按「↓」让 startIoa 跳过 255,然后手动改 startIoa=5,卡片冲突行显示「冲突 IOA 5–19(将覆盖 15 个点)」。
> 4. 已加 0–9, 50–59,count=20 → 「↦」让 startIoa 变成 10。
> 5. 切回 M_BO_NA_1(空类型),卡片立即刷新成空态。
> 6. 切语言(zh ↔ en),卡片中文字 / English 均显示正确。

- [ ] **Step 2: 若有视觉/行为不符,记录并修正,回到对应 Task**

若任何场景 fail,定位到具体 Task(模板 → Task 5,行为 → Task 4,算法 → Task 1/2),修补 + 跑测试 + 提交。

- [ ] **Step 3: 全部通过后,确认 plan 完成**

无需额外 commit。把此 plan 文件标记完成即可。

---

## Self-Review

**Spec 覆盖:**

- 常驻汇总区(目标 1) → Task 5 模板用 `.summary-card` 不带顶层 `v-if`。✓
- 视觉显眼(目标 2) → Task 5 样式独立卡片容器,主题色对齐。✓
- 两个一键按钮(目标 3) → Task 4 加 `applyNextAvailableIoa` / `applyNextFreeGap`,Task 5 模板渲染。✓
- 冲突 IOA 范围(目标 4) → Task 4 加 `conflictRanges`,Task 5 模板渲染。✓
- 空态文案 → Task 3 i18n `summaryEmpty`,Task 5 模板 `v-else`。✓
- 算法 — 「↓」= max+1 → Task 4 `nextAvailableIoa`。✓
- 算法 — 「↦」空隙搜索 → Task 2 `findNextFreeGap`。✓
- 容量不足判定 → Task 1 `IOA_MAX`,Task 2 返回 null,Task 4 tooltip。✓
- i18n 新 key → Task 3 zh + en 同步。✓
- 不改后端 → 全部前端任务。✓

**Placeholder 扫描:** 无 TBD/TODO。所有"实现"步骤都给了完整代码。手工测试有具体场景而非"测一下就行"。

**类型/命名一致性:**
- `compressRanges` / `lowerBound` / `findNextFreeGap` / `IOA_MAX`:Task 1、2 定义,Task 4 import 同名使用。✓
- `nextAvailableIoa` / `nextFreeGapStart` / `canApplyNextIoa` / `canApplyNextGap` / `applyNextAvailableIoa` / `applyNextFreeGap`:Task 4 定义,Task 5 模板使用同名。✓
- i18n key:`summaryEmpty`、`nextIoaBtn`、`nextGapBtn`、`nextIoaTooltipEmpty`、`capacityFullTooltip`、`conflictDetail` — Task 3 注册,Task 4 / 5 使用,同名。✓

**遗留:** Spec 中说"移除 `conflictWarn`"未做(Task 3 注释也说明保留)。这是 i18n 死 key,清理可作为单独的小 commit,不影响本 plan 完成度;保留是为了 plan 内每个 commit 都可独立 revert,无 zh/en 类型不同步风险。
