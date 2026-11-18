# 批量添加点位 — 已有点位汇总卡片

- 日期: 2026-05-20
- 组件: `frontend/src/components/BatchAddModal.vue`
- 目标: 解决用户在「批量添加点位」对话框中无法直观看到当前 ASDU 类型已有点位分布的问题,并提供"一键避让冲突"的快捷操作。

## 背景

当前 `BatchAddModal.vue` 在 ASDU 类型下拉框下方有一个 12px 灰色脚注 `.existing-summary`,**仅在该类型下已有点时才显示**。

实际使用中典型情况:用户切到一个尚未使用的类型(如截图中的 `M_BO_NA_1`),汇总区为空,用户产生"我不知道现在到底有哪些点位"的困惑;另外即便看到了 IOA 范围,起始 IOA 还要靠手算才能避开冲突。

## 目标

1. 已有点位汇总区**常驻**显示:任何 ASDU 类型下都展示当前状态,包括空态。
2. 视觉上**更显眼**:从脚注升级为独立卡片。
3. 提供**两个一键快捷按钮**:把起始 IOA 自动跳到无冲突位置。
4. 冲突告警从"X 个点冲突"升级为**列出具体冲突 IOA 范围**。

## 非目标

- 不做跨 ASDU 类型的全局点位汇总。
- 不做侧栏点位浏览器或抽屉。
- 不做"自动避让"——不主动改 startIoa,只在用户按按钮时改。
- 不做后端改动,所有计算在前端完成。

## 信息架构

汇总卡片放在 ASDU 类型下拉框之下、名称前缀输入框之上,替换现有的 `.existing-summary` 块。

```
┌────────────────────────────────────────────────┐
│ M_BO_NA_1 · 该类型尚未添加任何点                  │   ← 空态
└────────────────────────────────────────────────┘

┌────────────────────────────────────────────────┐
│ M_SP_NA_1 · 已有 256 个点                        │
│ IOA  0–255, 300–399                            │
│ [↓ 下一个可用 IOA]  [↦ 跳到能放下的空隙]           │
└────────────────────────────────────────────────┘

┌────────────────────────────────────────────────┐
│ M_SP_NA_1 · 已有 256 个点                        │
│ IOA  0–255, 300–399                            │
│ [↓ 下一个可用 IOA]  [↦ 跳到能放下的空隙]           │
│ ⚠ 冲突 IOA  5–7, 12   (将覆盖 4 个点)            │   ← 冲突时附加行
└────────────────────────────────────────────────┘
```

## 视觉规格

- 卡片容器: `background: var(--c-mantle)`, `border: 1px solid var(--c-surface1)`, `border-radius: 6px`, `padding: 10px 12px`, 与 modal 内部其他控件保持 `margin-top: 6px`。
- 标题行: 13px,左侧类型名 `var(--c-text)` 加粗,右侧计数 `var(--c-subtext0)`,用 `·` 分隔;空态时计数文案换成「该类型尚未添加任何点」。
- IOA 范围行: 12px,等宽字体 `var(--font-mono)`,前缀 "IOA" 用 `var(--c-overlay0)` 颜色,范围正文 `var(--c-text)`。空态时本行隐藏。
- 操作按钮行: 两个 small 按钮并排,`padding: 4px 10px`, `font-size: 12px`, `background: var(--c-surface0)`, hover `var(--c-surface1)`, 圆角 4px。无现有点或容量不足时按钮 `disabled` 并附 tooltip。
- 冲突行: 与按钮行之间用 `border-top: 1px dashed var(--c-red)` 分隔(卡片内分隔)。文字 `var(--c-red)` 12px,冲突 IOA 用 `var(--font-mono)`,末尾用 `(将覆盖 N 个点)` 提示数量。
- 卡片本身**不**因冲突变红边框,仅冲突行内部红。避免视觉过激。

## 行为规格

### 「↓ 下一个可用 IOA」按钮

把 `startIoa` 设为 `max(existingSameTypeIoas) + 1`。

- 现有点为空:按钮禁用,tooltip「当前类型无现有点」。
- 计算结果超过 IOA 上限(`16777215`,3 字节 IOA 上限):按钮禁用,tooltip「容量不足」。
- 不保证 `[startIoa, endIoa]` 整段无冲突——仅保证跨过已有点的末端。

### 「↦ 跳到能放下的空隙」按钮

把 `startIoa` 设为最小的 `s ≥ 0`,使得 `[s, s + count - 1]` 与 `existingSameTypeIoas` 零交集。

算法:
1. 若 `existingSameTypeIoas` 为空,`s = 0`。
2. 否则升序扫描已有点,维护候选起点 `s`。
   - 若 `xs[i] < s`,跳过。
   - 若 `xs[i] >= s` 且 `xs[i] <= s + count - 1`:`s = xs[i] + 1`,继续扫描。
   - 若 `xs[i] > s + count - 1`:已找到空隙,返回 `s`。
3. 扫完未冲突,返回 `s`。
4. 若结果 `s + count - 1 > 16777215`,按钮禁用,tooltip「容量不足」。

时间复杂度 O(k),k = 已有点数量。

### 冲突 IOA 列表

新增 computed `conflictRanges`: 对 `existingSameTypeIoas`(已排序)用二分(复用现有 `lowerBound`)取 `[startIoa, endIoa]` 子数组,用现有 `fmt` 范围压缩函数生成字符串。

仅在 `conflictCount > 0` 时显示。

### 「容量不足」判定

- 「↓ 下一个可用 IOA」:`max(existingSameTypeIoas) + 1 + 0 > 16777215`(即起点已超界)。
- 「↦ 跳到能放下的空隙」:算法返回的 `s` 满足 `s + count - 1 > 16777215`。
- count 自身已被 `min(1, max(100000))` 约束,所以不会出现 count 单独超界。

## 代码改动范围

仅改 `frontend/src/components/BatchAddModal.vue` 一个文件,加上两份 i18n locale。

### 脚本部分

- 抽出当前内联的范围压缩 `fmt` + 范围扫描逻辑为脚本顶层函数 `compressRanges(xs: number[]): string`,供 `existingRangesText` 与新增的 `conflictRanges` 共用。
- 抽出 `lowerBound` 为脚本顶层函数。
- 新增 computed `conflictRanges: string`。
- 新增方法 `applyNextAvailableIoa()`:写 `startIoa`。
- 新增方法 `applyNextFreeGap()`:写 `startIoa`。
- 新增 computed `canApplyNextIoa: boolean` / `canApplyNextGap: boolean`,驱动按钮 disabled 与 tooltip。

### 模板部分

替换现有 `<div v-if="existingSameTypeIoas.length > 0" class="existing-summary">…</div>` 整段为常驻的 `.summary-card` 结构,内含标题行、IOA 范围行(`v-if="existingSameTypeIoas.length > 0"`)、按钮行、冲突行(`v-if="conflictCount > 0"`)。

移除模板末尾独立的 `<div v-if="conflictCount > 0" class="conflict-warn">`,因为冲突行已并入卡片。

### 样式部分

- 移除 `.existing-summary` / `.ioa-ranges` / `.conflict-warn` 三个旧样式。
- 新增 `.summary-card` 及其子元素 `.summary-card__title` `.summary-card__ranges` `.summary-card__actions` `.summary-card__conflict` 样式。
- 复用现有 modal 的颜色变量,不引入新主题变量。

### i18n key 新增

`frontend/src/i18n/locales/zh-CN/*.json` 与 `en-US/*.json` 中 `batchModal` 命名空间下追加:

- `summaryEmpty`: "该类型尚未添加任何点" / "No points of this type yet"
- `nextIoaBtn`: "下一个可用 IOA" / "Next free IOA"
- `nextGapBtn`: "跳到能放下的空隙" / "Next fitting gap"
- `nextIoaTooltipEmpty`: "当前类型无现有点" / "No existing points to skip past"
- `capacityFullTooltip`: "IOA 容量不足" / "IOA capacity exhausted"
- `conflictDetail`: "冲突 IOA {ranges}(将覆盖 {count} 个点)" / "Conflicting IOA {ranges} (overwrites {count} points)"

`batchModal.existingSameType` 与 `batchModal.conflictWarn` 保持原样,仍由相应位置使用。

## 测试计划

- 手工:在子站 UI 中,按以下场景验证(headless 不适用,需要人工观察 UI):
  - 空模型:打开 modal,看到卡片显示"该类型尚未添加任何点",两按钮都灰。
  - 已有 0–255 的 M_SP_NA_1:切到 M_SP_NA_1 看到范围;按「↓」startIoa 变 256;按「↦」count=10 时 startIoa 变 256;count=300 时仍 256。
  - 已有 0–9, 50–59 的 M_SP_NA_1,count=20:「↓」→ 60;「↦」→ 10。
  - 起始 IOA=5、count=10、已有 5–7 与 12:冲突行显示「冲突 IOA  5–7, 12 (将覆盖 4 个点)」。
  - 切换 ASDU 类型,卡片随之刷新。
- 单元测试不新增(组件级,且只是 UI 调整);新增的 `compressRanges` 与「跳到空隙」算法可考虑在后续抽到独立 ts 文件后补测,但不在本次范围。

## 风险与回滚

- 风险点 1:`existingSameTypeIoas` 在父组件 `DataPointTable` 已保证升序去重——若上游契约破坏,「跳到空隙」算法行为不可预测。设计依赖该不变量,不增加防御性排序。
- 风险点 2:i18n 文案在两份 locale 中失衡——CI 已有 i18n 一致性脚本可覆盖。
- 回滚:本次改动局限单文件 + locale 增量 key,git revert 即可。
