# 子站数据表品质列改多位徽章 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把子站数据表 `DataPointTable.vue` 的品质列从只看 IV 的单点,换成与主站表格一致的紧凑只读多位徽章(`QualityIndicator`)。

**Architecture:** 复用 v1.6.0 已有的共享组件 `shared-frontend/components/QualityIndicator.vue`(支持 `compact` / `show-help` / `show-ov`),主站 `DataTable.vue` 已用同款写法。本变更只动子站表格单文件:替换品质单元格 markup + 加 import + 调列宽并删旧 `.quality-dot` CSS。无类型 / DTO / 后端改动(`DataPointInfo` 在 v1.6.0 已带 `quality_ov/bl/sb/nt`)。

**Tech Stack:** Vue 3 `<script setup>` + TypeScript,Vitest + @vue/test-utils(jsdom),`@shared` 别名指向 `shared-frontend/`。

---

## File Structure

- Modify: `frontend/src/components/DataPointTable.vue`
  - import 段加 `QualityIndicator`
  - 模板品质单元格(约 `:549-551`)替换为 `QualityIndicator`
  - `<style>` 内 `.col-quality` 宽度 40→88px,删除 `.quality-dot` 三段(`:759-778`)
- Test: `frontend/tests/dataPointTable.spec.ts`(新增一个 `it`)

---

### Task 1: 写失败测试 — 品质列渲染多位徽章

**Files:**
- Test: `frontend/tests/dataPointTable.spec.ts`

- [ ] **Step 1: 在 describe 块末尾(`8.2` 用例之后、`})` 之前)追加测试**

现有文件已有 `dp()` helper(返回全 false 品质)、`mountTable()`、`selectStation()`、`invokeMock`。新增:

```ts
  it('品质列渲染多位徽章(NT 高亮 / 正常显示 OK)', async () => {
    const ntPoint: DataPointInfo = { ...dp(1, 'M_SP_NA_1', '单点 (SP)', 'off'), quality_nt: true }
    const goodPoint = dp(2, 'M_SP_NA_1', '单点 (SP)', 'off')
    invokeMock.mockResolvedValue({ points: [ntPoint, goodPoint], seq: 1, total_count: 2 })
    const { wrapper, refs } = mountTable()
    await selectStation(refs)

    // NT 点:表格行内出现高亮 NT 徽章
    const litLetters = wrapper.findAll('.q-badge.lit').map((b) => b.text())
    expect(litLetters).toContain('NT')
    // 正常点:紧凑模式显示 OK
    expect(wrapper.find('.q-ok').exists()).toBe(true)
    wrapper.unmount()
  })
```

- [ ] **Step 2: 运行测试,确认失败**

Run: `cd frontend && npx vitest run tests/dataPointTable.spec.ts -t "品质列渲染多位徽章"`
Expected: FAIL —— 当前单元格渲染的是 `.quality-dot`,找不到 `.q-badge.lit` 与 `.q-ok`(`expect(litLetters).toContain('NT')` 不满足)。

---

### Task 2: 迁移品质单元格到 QualityIndicator

**Files:**
- Modify: `frontend/src/components/DataPointTable.vue`

- [ ] **Step 1: 加 import**

把(`:10` 一带,`EmptyState` import 之后)加入:

```ts
import EmptyState from '@shared/components/EmptyState.vue'
import QualityIndicator from '@shared/components/QualityIndicator.vue'
```

(原文件 `import EmptyState ...` 那一行保留,在其后新增 QualityIndicator 行。)

- [ ] **Step 2: 替换品质单元格 markup**

把现有(约 `:549-551`):

```html
              <td class="col-quality">
                <span v-if="point.quality_iv" class="quality-dot invalid" title="Invalid">IV</span>
                <span v-else class="quality-dot ok" title="Good"></span>
              </td>
```

替换为:

```html
              <td class="col-quality">
                <QualityIndicator
                  :quality="{ ov: point.quality_ov, bl: point.quality_bl, sb: point.quality_sb, nt: point.quality_nt, iv: point.quality_iv }"
                  :show-ov="point.asdu_type.startsWith('M_ME')"
                  :show-help="false"
                  compact
                />
              </td>
```

- [ ] **Step 3: 调列宽,删旧 .quality-dot CSS**

把(`:754-757`):

```css
.col-quality {
  width: 40px;
  text-align: center;
}
```

改为:

```css
.col-quality {
  width: 88px;
  text-align: center;
}
```

并删除紧随其后的整段(`:759-778`):

```css
.quality-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.quality-dot.ok {
  background: var(--c-green);
}

.quality-dot.invalid {
  background: var(--c-red);
  width: auto;
  height: auto;
  border-radius: 3px;
  padding: 1px 4px;
  font-size: 10px;
  font-weight: 600;
  color: var(--c-base);
}
```

(删除后 `.col-quality` 块下一段直接是 `.col-timestamp`。)

- [ ] **Step 4: 运行新测试,确认通过**

Run: `cd frontend && npx vitest run tests/dataPointTable.spec.ts -t "品质列渲染多位徽章"`
Expected: PASS

---

### Task 3: 全量验证 + 提交

**Files:**
- 无新增改动,仅校验

- [ ] **Step 1: 跑子站全量前端测试**

Run: `cd frontend && npx vitest run`
Expected: 全绿(原有用例 + 新增 1 个),无 FAIL。

- [ ] **Step 2: 类型检查**

Run: `cd frontend && npx vue-tsc -b`
Expected: exit 0,无类型错误。

- [ ] **Step 3: 提交**

先 `git status --porcelain | rg '^\?\?'` 审视未跟踪文件,剔除任何 ` 2.` 后缀的 iCloud 冲突副本(见项目记忆),再提交:

```bash
git add frontend/src/components/DataPointTable.vue frontend/tests/dataPointTable.spec.ts
git commit -m "fix(slave-ui): 数据表品质列改多位徽章, 与主站表格一致"
```

(commit message 不带任何生成署名。)

---

## Self-Review

**Spec coverage:** spec 三处改动(单元格替换 / import / CSS)分别落在 Task 2 Step 2/1/3;只读、OV 仅测量类、good→OK 行为由 `QualityIndicator` 既有逻辑保证,Task 1 测试覆盖 NT-lit 与 OK。非目标(不在表格内编辑 / 不改类型 / 不动其他列)均未引入额外改动。✓

**Placeholder scan:** 无 TBD/TODO;所有代码块为完整可粘贴文本。✓

**Type consistency:** `:quality` 对象字段 `ov/bl/sb/nt/iv` 与 `QualityIndicator` 的 `QualityBits` 接口一致;`DataPointInfo` 已含 `quality_ov/bl/sb/nt`(v1.6.0),`point.asdu_type` 为 string,`startsWith` 合法。测试用 `DataPointInfo` 类型 + 已有 `dp()` helper,字段齐全。✓
