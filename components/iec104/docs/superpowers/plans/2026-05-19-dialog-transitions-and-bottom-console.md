# 弹窗统一动画 + 底部终端控制台条 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给所有模态弹窗一套统一的平滑开合动画，并把底部「通信日志」区重设计成一条近黑的终端控制台条。

**Architecture:** 纯展示层改动。新增一个全局共享的 Vue `<Transition>` 过渡 `dialog-pop`（淡入遮罩 + 弹窗轻微缩放，`> *` 选择器与各弹窗内部 class 无关），10 个模态弹窗各包一层。底部 `LogPanel`（slave + master 两份）改用 `--c-crust` 近黑底 + 顶部蓝色发丝线 + 状态点。无业务逻辑改动。

**Tech Stack:** Vue 3 `<script setup>`、Tauri、Catppuccin Mocha CSS 变量（`shared-frontend/styles/tokens.css`）。验证用 `vue-tsc --noEmit` + `vite build`。

设计文档：`docs/superpowers/specs/2026-05-19-dialog-transitions-and-bottom-console-design.md`

---

## File Structure

- **Create** `shared-frontend/styles/transitions.css` — 全局共享过渡定义（`dialog-pop`）。
- **Modify** `frontend/src/main.ts`、`master-frontend/src/main.ts` — import 上面的 css。
- **Modify** 10 个弹窗组件 — 遮罩外包 `<Transition name="dialog-pop">`：
  - `shared-frontend/components/{AboutDialog,AppDialog,ParseFrameDialog,UpdateDialog}.vue`
  - `frontend/src/components/{NewServerModal,DataPointModal,BatchAddModal}.vue`
  - `master-frontend/src/components/{ControlDialog,RawSendDialog,NewConnectionModal}.vue`
- **Modify** `frontend/src/components/LogPanel.vue`、`master-frontend/src/components/LogPanel.vue` — 终端控制台条配色 + 状态点。

---

## Task 1: 共享过渡样式 `dialog-pop`

**Files:**
- Create: `shared-frontend/styles/transitions.css`
- Modify: `frontend/src/main.ts:2`
- Modify: `master-frontend/src/main.ts:2`

- [ ] **Step 1: 创建 `shared-frontend/styles/transitions.css`**

完整内容：

```css
/* Shared modal-dialog open/close transition.
   Apply with <Transition name="dialog-pop"> wrapping a backdrop element whose
   single direct child is the dialog box — `> *` then targets that box, so one
   definition fits every dialog regardless of its inner class names. */

.dialog-pop-enter-active { transition: opacity 160ms ease; }
.dialog-pop-leave-active { transition: opacity 110ms ease; }
.dialog-pop-enter-active > * { transition: transform 160ms ease-out, opacity 160ms ease-out; }
.dialog-pop-leave-active > * { transition: transform 110ms ease-in, opacity 110ms ease-in; }

.dialog-pop-enter-from,
.dialog-pop-leave-to { opacity: 0; }
.dialog-pop-enter-from > *,
.dialog-pop-leave-to > * { transform: scale(0.96); opacity: 0; }

@media (prefers-reduced-motion: reduce) {
  .dialog-pop-enter-active,
  .dialog-pop-leave-active,
  .dialog-pop-enter-active > *,
  .dialog-pop-leave-active > * { transition: none; }
}
```

- [ ] **Step 2: 在 `frontend/src/main.ts` import**

把第 2 行 `import '@shared/styles/tokens.css'` 之后加一行，结果为：

```ts
import { createApp } from 'vue'
import '@shared/styles/tokens.css'
import '@shared/styles/transitions.css'
import App from './App.vue'

createApp(App).mount('#app')
```

- [ ] **Step 3: 在 `master-frontend/src/main.ts` import**

同样改动 —— 第 2 行之后加 `import '@shared/styles/transitions.css'`，结果与上面一致。

- [ ] **Step 4: 类型检查**

Run: `cd frontend && npx --no-install vue-tsc --noEmit && cd ../master-frontend && npx --no-install vue-tsc --noEmit`
Expected: 两个都退出码 0、无输出。

- [ ] **Step 5: Commit**

```bash
git add shared-frontend/styles/transitions.css frontend/src/main.ts master-frontend/src/main.ts
git commit -m "feat(frontend): 新增共享 dialog-pop 弹窗过渡样式"
```

---

## Task 2: shared-frontend 弹窗套用动画

**Files:**
- Modify: `shared-frontend/components/AboutDialog.vue:20-44`
- Modify: `shared-frontend/components/AppDialog.vue:38`
- Modify: `shared-frontend/components/ParseFrameDialog.vue:136`
- Modify: `shared-frontend/components/UpdateDialog.vue`（迁移）

**包裹模式（所有弹窗通用）：** 在 `<Teleport to="body">` 那行之后插入一行
`<Transition name="dialog-pop">`，在配对的 `</Teleport>` 那行之前插入一行
`</Transition>`。被包裹的 `<div v-if=...>` 遮罩块内容**不需要重新缩进**（Vue
模板缩进纯属美观）。

- [ ] **Step 1: AboutDialog.vue 套用**

`shared-frontend/components/AboutDialog.vue` 的 `<template>` 结构：
```
  <Teleport to="body">
    <div v-if="visible" class="about-backdrop" @mousedown.self="emit('close')">
      ...
    </div>
  </Teleport>
```
改为：
```
  <Teleport to="body">
    <Transition name="dialog-pop">
    <div v-if="visible" class="about-backdrop" @mousedown.self="emit('close')">
      ...
    </div>
    </Transition>
  </Teleport>
```

- [ ] **Step 2: AppDialog.vue 套用**

同样在 `shared-frontend/components/AppDialog.vue` 的 `<Teleport to="body">` 后插入
`<Transition name="dialog-pop">`、`</Teleport>` 前插入 `</Transition>`。被包裹的是
`<div v-if="state.visible" class="dialog-backdrop" ...>`。

- [ ] **Step 3: ParseFrameDialog.vue 套用**

同样处理 `shared-frontend/components/ParseFrameDialog.vue`。被包裹的是
`<div v-if="visible" class="modal-backdrop" ...>`。

- [ ] **Step 4: UpdateDialog.vue 迁移到 dialog-pop**

`shared-frontend/components/UpdateDialog.vue` 已自带 `<Transition name="upd-fade">`。

4a. 模板里把 `<Transition name="upd-fade">` 改为 `<Transition name="dialog-pop">`。

4b. 删除 `<style scoped>` 里整段 `upd-fade` 过渡规则（6 行）：
```css
.upd-fade-enter-active,
.upd-fade-leave-active { transition: opacity 160ms ease; }
.upd-fade-enter-active .upd-dialog,
.upd-fade-leave-active .upd-dialog { transition: transform 160ms ease, opacity 160ms ease; }
.upd-fade-enter-from,
.upd-fade-leave-to { opacity: 0; }
.upd-fade-enter-from .upd-dialog,
.upd-fade-leave-to .upd-dialog { transform: scale(0.96); opacity: 0; }
```

4c. 把末尾的 `prefers-reduced-motion` 块里 `.upd-fade-*` 几行删掉，**保留
`.upd-fill`**（那是进度条，不是过渡）。结果为：
```css
@media (prefers-reduced-motion: reduce) {
  .upd-fill { transition: none; }
}
```

- [ ] **Step 5: 类型检查**

Run: `cd master-frontend && npx --no-install vue-tsc --noEmit && cd ../frontend && npx --no-install vue-tsc --noEmit`
Expected: 两个都退出码 0。（shared-frontend 组件被两个 app 共用，都要过。）

- [ ] **Step 6: Commit**

```bash
git add shared-frontend/components/AboutDialog.vue shared-frontend/components/AppDialog.vue shared-frontend/components/ParseFrameDialog.vue shared-frontend/components/UpdateDialog.vue
git commit -m "feat(dialogs): shared-frontend 弹窗统一套用 dialog-pop 动画"
```

---

## Task 3: slave 前端弹窗套用动画

**Files:**
- Modify: `frontend/src/components/NewServerModal.vue:71`
- Modify: `frontend/src/components/DataPointModal.vue:77`
- Modify: `frontend/src/components/BatchAddModal.vue:81`

包裹模式同 Task 2：`<Teleport to="body">` 后插 `<Transition name="dialog-pop">`，
`</Teleport>` 前插 `</Transition>`。

- [ ] **Step 1: NewServerModal.vue 套用** — 被包裹 `<div v-if="visible" class="modal-overlay" @mousedown.self="close">`。

- [ ] **Step 2: DataPointModal.vue 套用** — 被包裹 `<div v-if="visible" class="modal-backdrop" @click="handleBackdropClick">`。

- [ ] **Step 3: BatchAddModal.vue 套用** — 被包裹 `<div v-if="visible" class="modal-backdrop" @click="handleBackdropClick">`。

- [ ] **Step 4: 类型检查**

Run: `cd frontend && npx --no-install vue-tsc --noEmit`
Expected: 退出码 0。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/NewServerModal.vue frontend/src/components/DataPointModal.vue frontend/src/components/BatchAddModal.vue
git commit -m "feat(dialogs): slave 弹窗套用 dialog-pop 动画"
```

---

## Task 4: master 前端弹窗套用动画

**Files:**
- Modify: `master-frontend/src/components/ControlDialog.vue:240`
- Modify: `master-frontend/src/components/RawSendDialog.vue:115`
- Modify: `master-frontend/src/components/NewConnectionModal.vue:242`

包裹模式同 Task 2。

- [ ] **Step 1: ControlDialog.vue 套用** — 被包裹 `<div v-if="visible" class="modal-backdrop" @mousedown.self="emit('close')" @keydown="handleKeydown">`。

- [ ] **Step 2: RawSendDialog.vue 套用** — 被包裹 `<div v-if="visible" class="modal-backdrop" @mousedown.self="emit('close')" @keydown="handleKeydown">`。

- [ ] **Step 3: NewConnectionModal.vue 套用** — 被包裹 `<div v-if="visible" class="modal-backdrop" @mousedown.self="close">`。

- [ ] **Step 4: 类型检查**

Run: `cd master-frontend && npx --no-install vue-tsc --noEmit`
Expected: 退出码 0。

- [ ] **Step 5: Commit**

```bash
git add master-frontend/src/components/ControlDialog.vue master-frontend/src/components/RawSendDialog.vue master-frontend/src/components/NewConnectionModal.vue
git commit -m "feat(dialogs): master 弹窗套用 dialog-pop 动画"
```

---

## Task 5: slave LogPanel 终端控制台条

**Files:**
- Modify: `frontend/src/components/LogPanel.vue`（`<script setup>`、`<template>` header、`<style scoped>`）

- [ ] **Step 1: 新增 hasLogs computed**

`<script setup>` 里 `computed` 已 import（用于 `displayLogs`）。在 `displayLogs`
定义之后加一行：

```ts
// 折叠栏状态点:有报文流过为绿,空为暗灰。
const hasLogs = computed(() => logs.value.length > 0)
```

- [ ] **Step 2: 模板 header 加状态点**

`.log-header` 里 `<span class="log-toggle">` 与 `<span class="log-title">` 之间插入：

```html
      <span class="log-status-dot" :class="hasLogs ? 'active' : 'idle'" aria-hidden="true"></span>
```

- [ ] **Step 3: 改 `<style scoped>` 配色**

3a. `.log-panel` 选择器内加一行 `border-top`（与现有属性并列）：
```css
  border-top: 1px solid rgba(137, 180, 250, 0.25);
```

3b. `.log-header` 内 `background: var(--c-base);` 改为 `background: var(--c-crust);`

3c. `.log-table th` 内 `background: var(--c-mantle);` 改为 `background: var(--c-base);`

3d. `.log-btn` 内 `border: 1px solid var(--c-surface1);` 改为 `border: 1px solid var(--c-surface0);`

3e. 在 `<style scoped>` 末尾追加状态点样式：
```css
.log-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.log-status-dot.active { background: var(--c-green); }
.log-status-dot.idle { background: var(--c-overlay0); }
```

- [ ] **Step 4: 类型检查**

Run: `cd frontend && npx --no-install vue-tsc --noEmit`
Expected: 退出码 0。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/LogPanel.vue
git commit -m "feat(slave-ui): 底部日志区改为终端控制台条配色"
```

---

## Task 6: master LogPanel 终端控制台条

**Files:**
- Modify: `master-frontend/src/components/LogPanel.vue`（`<script setup>`、`<template>` header、`<style scoped>`）

改动与 Task 5 一致，套用到 master 的 LogPanel。

- [ ] **Step 1: 新增 hasLogs computed**

`computed` 已 import（第 2 行）。在 `displayLogs`（第 29 行）定义之后加：

```ts
// 折叠栏状态点:有报文流过为绿,空为暗灰。
const hasLogs = computed(() => logs.value.length > 0)
```

- [ ] **Step 2: 模板 header 加状态点**

`.log-header` 里 `<span class="log-toggle">` 与 `<span class="log-title">` 之间插入：

```html
      <span class="log-status-dot" :class="hasLogs ? 'active' : 'idle'" aria-hidden="true"></span>
```

- [ ] **Step 3: 改 `<style scoped>` 配色**

3a. `.log-panel` 选择器内加一行：
```css
  border-top: 1px solid rgba(137, 180, 250, 0.25);
```

3b. `.log-header` 内 `background: var(--c-base);`（第 343 行附近）改为 `background: var(--c-crust);`

3c. `.log-table th` 内 `background: var(--c-mantle);`（第 424 行附近）改为 `background: var(--c-base);`

3d. `.log-btn` 内 `border: 1px solid var(--c-surface1);`（第 385 行附近）改为 `border: 1px solid var(--c-surface0);`

3e. 在 `<style scoped>` 末尾追加状态点样式：
```css
.log-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.log-status-dot.active { background: var(--c-green); }
.log-status-dot.idle { background: var(--c-overlay0); }
```

- [ ] **Step 4: 类型检查**

Run: `cd master-frontend && npx --no-install vue-tsc --noEmit`
Expected: 退出码 0。

- [ ] **Step 5: Commit**

```bash
git add master-frontend/src/components/LogPanel.vue
git commit -m "feat(master-ui): 底部日志区改为终端控制台条配色"
```

---

## Task 7: 全量构建验证

**Files:** 无改动 —— 仅验证。

- [ ] **Step 1: 两个前端构建**

Run: `cd frontend && npx --no-install vite build && cd ../master-frontend && npx --no-install vite build`
Expected: 两个都 `✓ built`、退出码 0。

- [ ] **Step 2: 人工目视确认（执行者本地 `npm run dev` 或对照截图）**

- 打开任一弹窗（如「关于」「检查更新」）—— 应有遮罩淡入 + 弹窗轻微放大，关闭有淡出。
- 系统开启「减弱动态效果」后，弹窗应瞬间出现/消失（无动画）。
- 底部「通信日志」栏 —— 近黑底、顶部一条淡蓝发丝线、与主区分层明显；左侧状态点在有日志时绿、空时灰。

若构建失败或目视不符，回到对应 Task 修正后再继续。

- [ ] **Step 3: 无新增 commit**（Task 1–6 已分别提交；本任务仅验证）

---

## Notes

- `dialog-pop` 的 `> *` 选择器要求遮罩元素只有一个直接子元素。已核对 10 个弹窗
  均为「遮罩 div → 单个 box」结构，满足。若后续有弹窗在遮罩下加了兄弟元素，需
  改用更具体的选择器。
- `transitions.css` 是全局（非 scoped）样式，必须经 `main.ts` import 才生效。
- 全程无业务逻辑改动；`vue-tsc` + `vite build` 通过即视为结构正确，动效与配色
  以人工目视为准。
