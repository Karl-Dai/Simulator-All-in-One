# 子站「按 IOA 表达式批量写值」设计

- 日期：2026-06-16
- 状态：已评审，待实现
- 范围：子站（slave）前端新增「按 IOA 表达式批量写值」弹窗；零后端改动。

## 1. 背景与缺口

子站**已具备**批量写值能力：

- 后端命令 `batch_update_data_points(server_id, common_address, points: [{ioa, asdu_type}], value)`，内含「同分类校验 → 全或无写入 → 自发上送」。
- 前端 `ValuePanel.applyBatchValue()` 对**表格当前选中行**调用该命令。
- `DataPointTable` 多选支持 Shift（范围）/ Ctrl（点选）。

缺口在于**「非连续、跨很多点」的选取**：当目标点散落在上万行里（如 IOA 100、5000、30000、99999），靠滚动 + Ctrl 逐个点选不可行。本设计补一条**文本表达式**通路来指定目标点集。

## 2. 关键决策（已评审）

1. **指定方式**：弹窗内文本框输入 IOA 表达式（单点 + 区间混合）。
2. **类型定位**：弹窗内先选**具体 asdu_type**（默认取当前表格分类对应类型）；表达式只在该类型内解析。所有命中点同类型同分类，后端同分类校验天然满足、仅作兜底。
3. **表达式语义**：
   - 区间 `a-b` = 过滤该类型下 IOA ∈ [a,b] 的**已存在**点（稀疏区间天然成立）。
   - 单点 `n` = 该类型下 IOA=n 的点（存在才算命中）。
4. **缺失处理**：合法但不存在的**单点**静默跳过，只进「忽略 M」计数；`0 命中` 禁用写入。
5. **解析架构**：前端解析表达式 → 过滤 `DataPointTable.dataMap` → 得显式 `[{ioa, asdu_type}]` → 调**现有** `batch_update_data_points`。后端不动，与现有「选中行批量」同一信任模型。

显式区分两类「无效」：

- **语法错误**（非数字 token、`b<a`、越域 `>16777215`）→ 红色提示，**禁用写入**。
- **合法但不存在的单点 IOA** → 静默跳过，计入「忽略」。

## 3. 架构与数据流

纯前端特性。

```
用户填表达式 + 选 asdu_type + 填值
   │ input（防抖 ~120ms）
   ▼
parseIoaExpression(expr) → { ranges:[lo,hi][], singles:number[], error:string|null }
   │
   ▼ 与该 asdu_type 在 dataMap 中的已存在 IOA（升序去重）求交
命中集合 targets=[{ioa, asdu_type}]，忽略集合=不存在的单点
   │ 实时显示「命中 N · 忽略 M」+ compressRanges 紧凑区间
   ▼ 点「写入 N」
invoke('batch_update_data_points', { serverId, commonAddress, points: targets, value })
   │
   ▼ 后端（现有，不改）：同分类校验（必过）→ 全或无写入 → 自发上送
表格 2s 轮询拾取变化、行闪烁；父级 toast「已写入 N 个点」
```

- 命中点可达上千，IPC 传显式 pair 列表（每点约 ~30B，1000 点 ~30KB），可接受。
- dataMap 理论上可能滞后 ≤2s（增量轮询）；命中数以本地为准，与现有「选中行批量」一致，可接受。

## 4. 组件与文件落点

| 角色 | 文件 | 说明 |
|---|---|---|
| 新弹窗 | `frontend/src/components/BatchWriteModal.vue` | 与 `BatchAddModal.vue` 同级、同骨架 |
| 解析器 | `frontend/src/components/batchAdd/ioaRanges.ts`（扩展） | 新增 `parseIoaExpression`；复用已有 `compressRanges`/`lowerBound`/`IOA_MAX` |
| 挂载/入口 | `frontend/src/components/DataPointTable.vue` | 工具栏加按钮 + 底部挂载 `<BatchWriteModal>` |
| i18n | zh-CN / en-US 字典 | 新增 `batchWrite.*` 键 |

### 4.1 解析器接口

```ts
// ioaRanges.ts 新增
export interface IoaExpr {
  ranges: Array<[number, number]>  // 已校验 lo<=hi 且 <=IOA_MAX
  singles: number[]                // 已去重
  error: string | null             // 非 null 即语法错，调用方禁用写入
}
export function parseIoaExpression(input: string): IoaExpr
```

- 分隔符：逗号、空格、换行任意混用。
- 单点：`100`；区间：`1000-2000`（闭区间）。
- 校验：token 必须为数字 / `数字-数字`；区间要求 `lo<=hi`；所有值 `0..IOA_MAX`。任一不合法 → `error` 置文案、其余字段尽量返回但调用方据 `error` 禁用。
- 纯函数，无副作用，独立单测。

### 4.2 命中计算（在 BatchWriteModal 内 computed）

```
existingIoas = dataMap 中 asdu_type === 选中类型 的 ioa 列表（升序去重）  // 同 BatchAddModal existingSameTypeIoas
hitSingles   = singles ∩ existingIoas            // Set 或 lowerBound 判定
hitFromRanges= ∪ existingIoas[lowerBound(lo) .. lowerBound(hi+1)]
hitSet       = hitSingles ∪ hitFromRanges        // 去重
missedSingles= singles \ existingIoas            // 计入「忽略」
```

- 命中数 N = `hitSet.size`；忽略数 M = `missedSingles.length`。
- 命中/忽略均用 `compressRanges()` 紧凑展示。

## 5. 前端 UI

完全复用 `BatchAddModal` 的视觉与类名（`.modal-backdrop.dialog-blur`、`.modal`、`.modal-header/body/footer`、`.form-group/label/input/select`、`.summary-card*`、`.btn.btn-primary/secondary`），不引入新样式原语。主题走 Catppuccin CSS 变量。

```
┌─ 按 IOA 批量写值 ──────────────────────────  × ┐
│  类型   [ 短浮点测量 · M_ME_NC_1 · 13     ▾ ] │  .form-select，仅列本站存在类型
│  目标 IOA                                       │
│  ┌───────────────────────────────────────────┐ │  textarea(3行)，var(--font-mono)
│  │ 100, 500, 1000-2000, 5000, 9999           │ │
│  └───────────────────────────────────────────┘ │
│  ┌───────────────────────────────────────────┐ │  .summary-card
│  │ 命中 1003 个 · 忽略 2 个                    │ │  绿数=--c-green
│  │ IOA   100, 500, 1000–2000, 5000           │ │  compressRanges()
│  │ ┄┄┄ 忽略  999, 1500（不存在）             │ │  红虚线 .summary-card__conflict
│  └───────────────────────────────────────────┘ │
│  值     [ 99.9 ]                                │  .form-input，占位随类型变
├─────────────────────────────────────────────────┤
│                       [取消]   [写入 1003]      │  .btn-secondary / .btn-primary
└─────────────────────────────────────────────────┘
```

### 5.1 入口
`DataPointTable` 工具栏在现有「批量添加」按钮旁加「批量写值」按钮；无选中站或站内 0 点时禁用。点击置 `batchWriteVisible=true`。

### 5.2 类型下拉
- 选项 = 当前站 `existingPoints` 里实际出现的 `asdu_type`（去重）；`value`=后端可解析的大写串如 `"M_ME_NC_1"`（与 dataMap 键一致，也是 `batch_update_data_points` 入参）；`label`=本地化名 · typeId（用 `ASDU_TYPE_OPTIONS` 建大写串→label 反查表）。
- 默认值 = 当前表格分类过滤对应类型；无过滤则取首个。
- 切换类型 → 命中实时重算。

### 5.3 值占位
随类型给 placeholder：归一化「原始 NVA -32768..32767」、单点「1/0 或 ON/OFF」、双点「0/1/2/3」、浮点「如 99.9」、标度化「i16 整数」、累计量「i32 整数」。值合法性最终由后端 `parse_value_for` 兜底（全或无）。

### 5.4 状态矩阵
| 状态 | 命中卡片 | 写入按钮 |
|---|---|---|
| 表达式为空 | 隐藏卡片 | 禁用，文案「写入」 |
| 语法错 | 红虚线「无法解析: `xxx`」 | 禁用 |
| 0 命中 | 「命中 0 个」 | 禁用 |
| 正常 N>0 | 绿色命中数 + 命中/忽略区间 | 可点，文案「写入 N」 |
| 值为空 | 不变 | 禁用 |
| 提交中 | 不变 | 禁用，文案「写入中…」 |

### 5.5 键盘与提交
- `Esc` / 点遮罩关闭；`Cmd/Ctrl+Enter` 提交；textarea 内 `Enter` 正常换行。
- 提交成功 → emit `written` + 关弹窗 + 父级 toast「已写入 N 个点」；失败 → `showAlert(后端错误)`，弹窗保留不关。

### 5.6 i18n 键（zh-CN + en-US 均加）
`batchWrite.title / typeLabel / ioaLabel / ioaPlaceholder / valueLabel / hit / ignored / parseError / write / writing / emptyHint / successToast`，并补 `table.batchWrite`（工具栏按钮）。

## 6. 错误处理

- 0 命中 / 语法错 / 值空 → 禁用写入（前端拦截）。
- 后端值解析失败或同分类兜底失败 → `showAlert` 显示后端原文，整批不写，弹窗保留。
- 关弹窗 / 切站 / 切类型时重置表达式与值。

## 7. 测试（无头）

- **解析器单测**（`parseIoaExpression` 纯函数）：单点、区间、混合、去重、空白/换行分隔、非数字 token、`b<a`、越域 `>IOA_MAX`、空串、前后空格。
- **命中计算单测**：稀疏区间过滤、单点命中/缺失、区间+单点并集去重。
- **Playwright + Tauri mock**（遵循项目无头规范）：mock `batch_update_data_points`，验证命中/忽略计数、禁用态切换、`写入 N` 文案、提交传参、成功 toast、失败保留弹窗。
- 后端 `batch_update_data_points` 已有测试，不动。

## 8. 范围之外（YAGNI）

- 跨多个 asdu_type 同时写。
- 按值 / 品质条件过滤选点。
- 每点不同值（批量仍是统一值）。
- 保存表达式预设 / 历史。
- 批量改品质（本特性只改值；`batch_set_data_point_quality` 已另有入口）。
