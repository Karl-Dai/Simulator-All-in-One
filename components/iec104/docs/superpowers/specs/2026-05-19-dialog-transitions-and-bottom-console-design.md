# 弹窗统一动画 + 底部终端控制台条 — 设计

日期：2026-05-19
状态：已批准设计，待写实现计划

## 背景

两处视觉问题：

1. **弹窗瞬间弹出**。12 个组件里只有 `UpdateDialog`（最近重写时加的）有
   开合动画，其余模态弹窗都是瞬间出现/消失，观感生硬、缺乏统一的动效语言。
2. **底部「通信日志」区无存在感**。slave/master 的 `LogPanel` 折叠栏背景为
   `--c-base`（#1e1e2e），与主工作区同色，几乎不可辨。它语义上是一个报文
   收发的控制台，却没有相应的视觉身份。

目标：给所有模态弹窗一套统一的平滑开合动画；把底部日志区重设计成一条有
辨识度的「终端控制台条」。纯展示层改动，不动业务逻辑。

## Part A — 弹窗统一平滑动画

### 共享过渡定义

新增 `shared-frontend/styles/transitions.css`（全局样式，非 scoped）。两个
app 的 `main.ts` 各 import 一次，与 `tokens.css` 并列。

定义一个名为 `dialog-pop` 的 Vue `<Transition>` 过渡：

- **遮罩层**：`opacity` 0↔1，入场 ~160ms，退场 ~110ms（退场略快，更跟手）。
- **弹窗本体**：用 `> *` 选择遮罩的直接子元素 —— 与各弹窗内部 class 名无关，
  因此一套定义适配所有弹窗。`transform: scale(0.96)↔1` + opacity，时长同上。
- 缓动：入场 `ease-out`，退场 `ease-in`。
- `@media (prefers-reduced-motion: reduce)`：全部 `transition: none`。

约束：`> *` 选择器要求每个弹窗的遮罩元素**恰好只有一个直接子元素**（即弹窗
本体）。实现时逐个核对；若某弹窗遮罩下还有其它兄弟元素（如内联右键菜单），
改用更具体的选择器或在该弹窗内单独处理。

### 套用范围

下列 9 个真·模态弹窗，把遮罩 `v-if` 元素外包一层 `<Transition name="dialog-pop">`
（使用 `Teleport` 的弹窗，`<Transition>` 放在 `<Teleport>` 内部）：

- `shared-frontend/components/AboutDialog.vue`
- `shared-frontend/components/AppDialog.vue`（提示 / 确认）
- `shared-frontend/components/ParseFrameDialog.vue`
- `frontend/src/components/NewServerModal.vue`
- `frontend/src/components/DataPointModal.vue`
- `frontend/src/components/BatchAddModal.vue`
- `master-frontend/src/components/ControlDialog.vue`
- `master-frontend/src/components/RawSendDialog.vue`
- `master-frontend/src/components/NewConnectionModal.vue`

`UpdateDialog.vue` 从自带的 `upd-fade` 过渡迁移到共享的 `dialog-pop`，删除其
`<style>` 中那段重复的过渡 CSS —— 全应用统一一套动画语言。

**不在范围**：`LangSwitch.vue`（内联切换控件）、`VersionBadge.vue`（版本徽章
chip）—— 都不是模态弹窗。

## Part B — 底部终端控制台条

改 `frontend/src/components/LogPanel.vue` 与 `master-frontend/src/components/LogPanel.vue`
两份，保持一致。底部 `<footer class="log-area">` 横跨整个 app 宽度。

- **背景统一近黑**：整个面板（折叠态 32px 栏 + 展开态日志区）背景改为
  `--c-crust`（#11111b），形成一条停靠底部的终端条。`.log-header` 由
  `--c-base` 改为 `--c-crust`；`.log-body` 已是 `--c-crust`，保持。
- **顶部发丝分隔线**：`.log-panel` 顶部 `border-top: 1px solid rgba(137, 180, 250, 0.25)`
  （淡蓝），与上方 `--c-base` 主区域明确分层。
- **状态小圆点**：折叠栏标题左侧新增一个小圆点。新增一个 computed：
  `logs.value.length > 0` 时圆点为 `--c-green`（有报文流过），否则
  `--c-overlay0`（暗灰）。不引入新的数据依赖。
- **控件重新配色**：标题文字 / 折叠箭头改用 `--c-subtext0`/`--c-text` 在近黑
  底上保证对比度；刷新 / 清除 / 导出 CSV 按钮改为 ghost 风格 + `--c-surface0`
  描边。
- **展开态表格层次**：日志表表头由 `--c-mantle` 改为 `--c-base`，与 `--c-crust`
  的表体拉出一点对比，避免表头与背景糊在一起。

无业务逻辑改动 —— Part B 仅 CSS + 一个 computed（状态点颜色）。

## 验证

- 两个前端 `vue-tsc --noEmit` 通过。
- 两个前端 `vite build` 通过。
- 人工目视：弹窗开合动画顺滑、`prefers-reduced-motion` 下无动画；底部终端
  条配色与分层符合预期、状态点随日志有无变色。

## 范围与非目标

- 仅展示层（CSS / 模板 / 一个 computed）。不改任何 Tauri 命令、协议、状态。
- 不改弹窗的交互逻辑、不重排弹窗内容。
- 不动 `LangSwitch` / `VersionBadge`。
