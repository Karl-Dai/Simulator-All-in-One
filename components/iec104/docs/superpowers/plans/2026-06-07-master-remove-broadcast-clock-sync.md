# 主站移除「广播对时」前端入口 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 从主站工具栏「广播 ▾」下拉中移除「广播对时」入口(及其 i18n key),后端命令保留。

**Architecture:** 纯前端删除。删 `Toolbar.vue` 的下拉 `<li>` 与 `sendBroadcastClockSync()` 函数,再删三处现在失去消费者的 i18n key(zh 类型+值、en 值)。`vue-tsc -b`(build 一部分)做静态一致性门,Playwright 无头实测做真实浏览器门。后端 `send_broadcast_clock_sync` 命令与注册不动。

**Tech Stack:** Vue 3 + TypeScript + Vite(`master-frontend`),Playwright(无头,Tauri mock via `addInitScript`)。

**关联 spec:** `docs/superpowers/specs/2026-06-07-master-remove-broadcast-clock-sync-design.md`

---

## File Structure

| 文件 | 改动 | 责任 |
|---|---|---|
| `master-frontend/src/components/Toolbar.vue` | Modify | 删广播下拉的对时 `<li>` + `sendBroadcastClockSync()` 函数 |
| `master-frontend/src/i18n/locales/zh-CN.ts` | Modify | 删 `broadcastClockSync` 类型声明 + 中文值 |
| `master-frontend/src/i18n/locales/en-US.ts` | Modify | 删 `broadcastClockSync` 英文值 |

**不动**:`crates/iec104master-app/src/commands.rs`(`send_broadcast_clock_sync` 保留)、`crates/iec104master-app/src/lib.rs`(注册保留)、`crates/iec104sim-core/src/master.rs`(`build_clock_sync_command` 及其单测保留)、常规逐 CA 对时(`sendClockSync` / i18n `clockSync`)、广播总召、广播计量召唤。

**注意**:会话起始 `master-frontend/src/components/Toolbar.vue` 已有未提交改动(git `M`)。本计划叠加其上,**只提交本次删除涉及的 3 个文件**,不裹挟既有改动(见 Task 5)。

---

### Task 1: 删除「广播对时」下拉项与处理函数

**Files:**
- Modify: `master-frontend/src/components/Toolbar.vue`

- [ ] **Step 1: 删除下拉菜单中的「广播对时」`<li>`**

`master-frontend/src/components/Toolbar.vue` 内,把广播下拉的三项:

```vue
            <li @click="sendBroadcastGI">{{ t('toolbar.broadcastGi') }}</li>
            <li @click="sendBroadcastClockSync">{{ t('toolbar.broadcastClockSync') }}</li>
            <li @click="sendBroadcastCounterRead">{{ t('toolbar.broadcastCounterRead') }}</li>
```

改为两项(删中间一行):

```vue
            <li @click="sendBroadcastGI">{{ t('toolbar.broadcastGi') }}</li>
            <li @click="sendBroadcastCounterRead">{{ t('toolbar.broadcastCounterRead') }}</li>
```

- [ ] **Step 2: 删除 `sendBroadcastClockSync()` 函数**

删除整段函数(含其后的一个空行),即把:

```ts
async function sendBroadcastClockSync() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('send_broadcast_clock_sync', { id: selectedConnectionId.value })
  } catch (e) { await showAlert(String(e)) }
  broadcastMenuOpen.value = false
}

async function sendBroadcastCounterRead() {
```

替换为(只留下一个函数的起始):

```ts
async function sendBroadcastCounterRead() {
```

- [ ] **Step 3: 校验本文件已无对时广播引用**

Run: `grep -n "sendBroadcastClockSync\|broadcastClockSync" master-frontend/src/components/Toolbar.vue`
Expected: 无输出(0 命中)。

---

### Task 2: 删除失去消费者的 i18n key

**Files:**
- Modify: `master-frontend/src/i18n/locales/zh-CN.ts`
- Modify: `master-frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1: 删 zh-CN 类型声明**

`master-frontend/src/i18n/locales/zh-CN.ts`,删除 `toolbar` 接口里的这一行(在 `broadcastGi` 与 `broadcastCounterRead` 之间):

```ts
    broadcastClockSync: string
```

- [ ] **Step 2: 删 zh-CN 中文值**

同文件,删除值对象里的这一行:

```ts
    broadcastClockSync: '广播对时',
```

- [ ] **Step 3: 删 en-US 英文值**

`master-frontend/src/i18n/locales/en-US.ts`,删除这一行:

```ts
    broadcastClockSync: 'Broadcast Clock Sync',
```

- [ ] **Step 4: 全仓校验前端已无残留引用**

Run: `grep -rn "broadcastClockSync\|sendBroadcastClockSync" master-frontend/src/`
Expected: 无输出(0 命中)。后端 `send_broadcast_clock_sync` 不在此范围,保留。

---

### Task 3: 静态门 —— 类型 + 构建

**Files:** 无(只跑命令)

- [ ] **Step 1: 构建(含 `vue-tsc -b` 类型检查)**

Run: `npm --prefix master-frontend run build`
Expected: 退出码 0,无 TS 报错。`vue-tsc -b` 确认 i18n key 从「类型 + 两语言值」三处一致删除后类型自洽,且模板再无对 `broadcastClockSync` 的引用。

> 若 `vue-tsc` 报 `broadcastClockSync` 相关错误,说明某处(类型/值/模板)漏删或多删,回 Task 1/2 修正。

---

### Task 4: 真实浏览器门 —— Playwright 无头实测

**Files:**
- Create(throwaway,**不提交**): `scripts/_verify-broadcast-menu.mjs`

> 遵循 memory `feedback_frontend_headless_verify`:前端改动必须 Playwright 真实浏览器实测。Tauri mock 必须在 app boot 前经 `addInitScript` 注入,故用独立脚本(MCP 工具无法 boot 前注入)。Chromium 已缓存于 `~/Library/Caches/ms-playwright`。

- [ ] **Step 1: 准备 Playwright(临时,不写入 package.json)**

Run: `npm --prefix scripts i --no-save playwright`
Expected: 安装完成。若后续启动报缺浏览器,再跑 `npx --prefix scripts playwright install chromium`(通常因缓存命中而 no-op)。

- [ ] **Step 2: 启动 master 开发服务器(后台)**

后台运行:`npm --prefix master-frontend run dev`(Vite 监听 5177)。
等待就绪:`until curl -sf http://localhost:5177 >/dev/null; do sleep 0.5; done`

- [ ] **Step 3: 写入临时验证脚本**

写入 `scripts/_verify-broadcast-menu.mjs`:

```js
import { chromium } from 'playwright'

const MASTER = 'http://localhost:5177/'
const DARK_BG = 'rgb(17, 17, 27)'

// 一条 Connected 连接,足以点亮广播按钮组
const conn = {
  id: 'c1', target_address: '127.0.0.1', port: 2404, common_addresses: [1],
  state: 'Connected', use_tls: false, t0: 30, t1: 15, t2: 10, t3: 20, k: 12, w: 8,
  default_qoi: 20, default_qcc: 6, interrogate_period_s: 60,
  counter_interrogate_period_s: 60, broadcast_address: 65535,
}
const cfg = { locale: 'zh-CN', commands: {
  list_connections: [conn],
  get_received_data_since: { seq: 0, total_count: 0, points: [] },
  get_communication_logs: [],
  check_for_update: null,
  set_logging_enabled: null,
} }

// 在 app boot 前安装 Tauri IPC mock(纯浏览器闭包,勿引用 Node 作用域)
function installTauriMock(cfg) {
  try { localStorage.setItem('iec104.language', cfg.locale) } catch (e) { /* ignore */ }
  const DATA = cfg.commands || {}
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd) => {
      if (cmd.indexOf('plugin:event|') === 0) return 1
      return (cmd in DATA) ? DATA[cmd] : null
    },
    transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window['_cb' + id] = cb; return id },
    unregisterCallback: () => {},
    convertFileSrc: (p) => p,
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
}

const browser = await chromium.launch()
let ok = false
let menu = []
try {
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 }, locale: 'zh-CN' })
  const page = await ctx.newPage()
  await page.addInitScript(installTauriMock, cfg)
  await page.goto(MASTER, { waitUntil: 'domcontentloaded' })
  await page.waitForFunction((bg) => getComputedStyle(document.body).backgroundColor === bg, DARK_BG, { timeout: 8000 })
  // 选中连接 → isConnected() 为真 → 广播按钮组启用
  await page.getByText('127.0.0.1', { exact: false }).first().click()
  await page.waitForTimeout(300)
  // 点广播组的 ▾(用 body 文本恰为「广播」的按钮定位该 split-btn,避免命中总召组)
  await page.locator('.split-btn:has(button:text-is("广播")) .split-toggle').click()
  menu = (await page.locator('ul.split-menu li').allInnerTexts()).map((s) => s.trim())
  const hasClock = menu.some((t) => t.includes('对时'))
  ok = menu.length === 2 && menu.includes('广播总召') && menu.includes('广播计量召唤') && !hasClock
} finally {
  await browser.close()
}
console.log('广播菜单项:', JSON.stringify(menu))
if (!ok) { console.error('✗ 验证失败:期望恰为 [广播总召, 广播计量召唤],无「广播对时」'); process.exit(1) }
console.log('✓ 验证通过:广播菜单 2 项,无「广播对时」')
```

- [ ] **Step 4: 运行验证**

Run: `node scripts/_verify-broadcast-menu.mjs`
Expected: 打印 `广播菜单项: ["广播总召","广播计量召唤"]` 与 `✓ 验证通过`,退出码 0。

> 若打印含「广播对时」或项数 ≠ 2 → 删除不完整,回 Task 1。

- [ ] **Step 5: 收尾 —— 停服务器、删临时脚本**

- 停掉后台 dev server(结束 Step 2 启动的进程)。
- Run: `rm scripts/_verify-broadcast-menu.mjs`
- Run: `git status --porcelain scripts/` → 期望无 `_verify-broadcast-menu.mjs` 残留(`scripts/node_modules` 本就 gitignore)。

---

### Task 5: 提交(仅本次 3 个源文件)

**Files:** 无新增

- [ ] **Step 1: 仅暂存本次删除涉及的文件**

> 会话起始 `Toolbar.vue` 已有未提交改动且仓库可能有其他 `M`。**精确按路径暂存**,不要 `git add -A`(亦避免裹挟 iCloud 冲突副本,见 memory `project_icloud_dup_files`)。但注意:`Toolbar.vue` 的既有改动会随该文件一并提交——执行前先 `git diff master-frontend/src/components/Toolbar.vue` 复核,确认其中非本次的改动也属可提交内容;若不属于,先与用户确认。

Run:
```bash
git add master-frontend/src/components/Toolbar.vue \
        master-frontend/src/i18n/locales/zh-CN.ts \
        master-frontend/src/i18n/locales/en-US.ts
git status --porcelain
```
Expected: 仅上述 3 文件处于已暂存状态。

- [ ] **Step 2: 提交(作者按规范,无 Claude 署名)**

Run:
```bash
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(master-fe): 移除「广播对时」前端入口(保留后端命令)" \
  -m "子站广播只对齐 GI+CI 不含对时;广播下拉 3 项→2 项(总召/计量召唤)。后端 send_broadcast_clock_sync 命令与注册保留。"
```
Expected: 提交成功。

- [ ] **Step 3: 复核作者与改动范围**

Run: `git show --stat HEAD | head -20`
Expected: 作者 `Karl-Dai Karl <kelsoprotein@gmail.com>`;改动文件为 `Toolbar.vue` + `zh-CN.ts` + `en-US.ts`。

---

## Self-Review

- **Spec coverage**:范围(仅前端删 li+函数+i18n,保留后端命令/共享函数/常规对时/总召/计量召唤)→ Task 1/2 覆盖删除,明确「不动」清单与 File Structure 一致;验证(build / Playwright 2 项菜单 / cargo 未受影响)→ Task 3/4 覆盖(后端未动无需 cargo 步骤,build 失败才回查)。
- **Placeholder scan**:无 TBD/TODO;每个代码步给出完整 old/new 与可执行命令及期望输出。
- **Type/命名一致**:`sendBroadcastClockSync` / `broadcastClockSync` / `send_broadcast_clock_sync`(后端,保留)三个名字在全文用法一致;Playwright 选择器 `.split-btn:has(button:text-is("广播")) .split-toggle` 与 `Toolbar.vue` 实际结构(广播 body 按钮文本 = `t('toolbar.broadcast')` = 「广播」)对应。
