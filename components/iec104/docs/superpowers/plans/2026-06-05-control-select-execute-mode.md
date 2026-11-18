# 遥控命令"仅选择/仅执行/自动两步"模式 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把遥控对话框的「选择-执行(SbO)」复选框升级为「控制模式」三选一(仅执行 / 仅选择 / 自动两步),让用户能单独下发一条 S/E=1 的选择帧。

**Architecture:** 后端核心库 `send_*_command(..., select: bool, ...)` 本就接受 S/E 布尔,直接执行分支只是写死传 `false`。改动集中在 `iec104master-app/src/commands.rs`(新增 `control_mode` 字段 + 纯函数 `resolve_control_mode` + `send_control_command` 分支按模式透传 `sel`)与前端 `ControlDialog.vue`(复选框换下拉 + 持久化迁移)。核心库 `master.rs` 不改逻辑。

**Tech Stack:** Rust(tauri command, tokio)、Vue 3 `<script setup>` + TypeScript、Playwright(headless,记录型 Tauri mock)。

**提交署名:** 遵循项目 CLAUDE.md——作者必须为 `Karl-Dai Karl <kelsoprotein@gmail.com>`,禁止任何 Claude co-author 行。每条 commit 用 `git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit ...`。

**路径说明:** 仓库无空格副本在 `/Users/daichangyu/code/IEC60870-5-104-Simulator`(与 iCloud 路径同一物理仓库),所有 cargo/npm 命令在此路径执行。

**对 spec 的细化:** spec「测试策略」第 1 条原写在 `control_e2e.rs` 加端到端用例;改为在 `commands.rs` 用 `#[cfg(test)]` 单元测试覆盖新逻辑(`resolve_control_mode` 模式映射 + `build_control_frames_single` 的 S/E 位编码),因为 e2e 断言 slave 内存无法区分"仅选择(不改值)"与"未送达"。前端行为由 Playwright 覆盖(Task 5)。

---

## File Structure

- `crates/iec104master-app/src/commands.rs` — 修改:`ControlCommandRequest` 加字段;新增 `ControlMode` enum + `resolve_control_mode` 纯函数;`send_control_command` 分支重构;新增 `#[cfg(test)]` 单元测试。
- `master-frontend/src/components/ControlDialog.vue` — 修改:`selectMode`→`controlMode` 下拉、持久化迁移、位串锁定、payload。
- `master-frontend/src/i18n/locales/zh-CN.ts` — 修改:`DictShape.control` 加 4 个 key + `control` 字典加中文文案。
- `master-frontend/src/i18n/locales/en-US.ts` — 修改:`control` 字典加英文文案。

---

## Task 1: 后端——控制模式解析(字段 + enum + 纯函数 + 单元测试)

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs:455-471`(结构体加字段)、`:472`(新增 enum + 函数)
- Test: `crates/iec104master-app/src/commands.rs`(文件末尾 `#[cfg(test)] mod tests`)

- [ ] **Step 1: 给 `ControlCommandRequest` 加 `control_mode` 字段**

在 `crates/iec104master-app/src/commands.rs` 中,把结构体末尾的 `bitstring` 字段后追加新字段(改 469-471 区域):

```rust
    /// 32-bit payload for C_BO_NA_1 (51). Required when command_type == "bitstring".
    pub bitstring: Option<u32>,
    /// 控制模式: "execute" | "select" | "sbo"。缺省时回退到旧 `select` 语义
    /// (select==Some(true) → sbo,否则 execute)。
    pub control_mode: Option<String>,
}
```

- [ ] **Step 2: 在结构体之后新增 `ControlMode` enum 与 `resolve_control_mode` 纯函数**

紧接在 `ControlCommandRequest` 闭合 `}`(原 471 行)之后插入:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlMode {
    Execute,
    Select,
    Sbo,
}

/// 解析控制模式。`control_mode` 优先;缺省时回退旧 `select` 字段语义。
/// 未知的 `control_mode` 字符串也走回退分支(宽容处理)。
fn resolve_control_mode(control_mode: Option<&str>, select: Option<bool>) -> ControlMode {
    match control_mode {
        Some("select") => ControlMode::Select,
        Some("sbo") => ControlMode::Sbo,
        Some("execute") => ControlMode::Execute,
        _ => {
            if select == Some(true) {
                ControlMode::Sbo
            } else {
                ControlMode::Execute
            }
        }
    }
}
```

- [ ] **Step 3: 在文件末尾写失败的单元测试**

在 `crates/iec104master-app/src/commands.rs` 文件最末尾追加:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_mode_explicit_wins() {
        assert_eq!(resolve_control_mode(Some("select"), None), ControlMode::Select);
        assert_eq!(resolve_control_mode(Some("execute"), None), ControlMode::Execute);
        assert_eq!(resolve_control_mode(Some("sbo"), None), ControlMode::Sbo);
        // 显式 control_mode 覆盖旧 select 字段
        assert_eq!(resolve_control_mode(Some("execute"), Some(true)), ControlMode::Execute);
    }

    #[test]
    fn resolve_mode_legacy_fallback() {
        assert_eq!(resolve_control_mode(None, Some(true)), ControlMode::Sbo);
        assert_eq!(resolve_control_mode(None, Some(false)), ControlMode::Execute);
        assert_eq!(resolve_control_mode(None, None), ControlMode::Execute);
        // 未知字符串回退看 select
        assert_eq!(resolve_control_mode(Some("bogus"), Some(true)), ControlMode::Sbo);
    }

    #[test]
    fn build_single_select_bit_set() {
        // value=true, select=true, qu=0 → SCO = 0x80 | 0x01 = 0x81
        let sel = build_control_frames_single(1, 1, true, true, 0, 6);
        assert_eq!(*sel.last().unwrap(), 0x81, "select 帧 SCO 应置 S/E 位(bit7)");
        // value=true, select=false → SCO = 0x01
        let exe = build_control_frames_single(1, 1, true, false, 0, 6);
        assert_eq!(*exe.last().unwrap(), 0x01, "execute 帧 SCO 不应置 S/E 位");
    }
}
```

- [ ] **Step 4: 运行测试,确认编译失败**

Run: `cargo test -p iec104master-app --lib resolve_mode 2>&1 | tail -20`
Expected: 编译失败 —— `ControlMode` / `resolve_control_mode` 未定义(若 Step 1-2 未保存)或 `control_mode` 字段缺失。若 Step 1-2 已保存则应直接 PASS,此时跳到 Step 5。

- [ ] **Step 5: 运行测试,确认通过**

Run: `cargo test -p iec104master-app --lib 2>&1 | tail -20`
Expected: `test result: ok.`,三个测试 `resolve_mode_explicit_wins`、`resolve_mode_legacy_fallback`、`build_single_select_bit_set` 全 PASS。

- [ ] **Step 6: 提交**

```bash
cd /Users/daichangyu/code/IEC60870-5-104-Simulator
git add crates/iec104master-app/src/commands.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit -m "feat(master): 控制命令新增 control_mode 解析与单元测试"
```

---

## Task 2: 后端——`send_control_command` 按模式分支(支持仅选择)

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs:506-569`(模式解析 + 直接执行分支重构)

- [ ] **Step 1: 替换模式解析与 eprintln**

把原第 506 行 `let select = request.select.unwrap_or(false);` 起到第 515 行 `);` 的这一段:

```rust
    let select = request.select.unwrap_or(false);
    let ca = request.common_address;
    let ioa = request.ioa;
    let qu = request.qualifier.unwrap_or_else(|| default_qualifier(&request.command_type));
    let cot = request.cot.unwrap_or(6);

    eprintln!(
        "[send_control_command] enter type={} ioa={} ca={} select={} | connections_read_lock={}ms",
        request.command_type, ioa, ca, select, t_lock.as_millis()
    );
```

替换为:

```rust
    let mode = resolve_control_mode(request.control_mode.as_deref(), request.select);
    let ca = request.common_address;
    let ioa = request.ioa;
    let qu = request.qualifier.unwrap_or_else(|| default_qualifier(&request.command_type));
    let cot = request.cot.unwrap_or(6);

    eprintln!(
        "[send_control_command] enter type={} ioa={} ca={} mode={:?} | connections_read_lock={}ms",
        request.command_type, ioa, ca, mode, t_lock.as_millis()
    );
```

- [ ] **Step 2: 重构"直接发一条"分支(原 518-569)支持 Execute/Select**

把原第 517-569 行(注释 `// Direct execute...` 起,到该分支 `}` 闭合,即 `return Ok(ControlResult { ... });` 后的 `}`)整体替换为:

```rust
    // 仅执行 / 仅选择: 发一条命令并立即返回(不阻塞等待 ACT_CON)。
    // sel 决定 S/E 位: 仅选择→true(S/E=1), 仅执行→false(S/E=0)。
    if mode != ControlMode::Sbo {
        let sel = mode == ControlMode::Select;
        let start = std::time::Instant::now();
        match request.command_type.as_str() {
            "single" => {
                let value = parse_bool(&request.value)?;
                conn.connection.send_single_command(ioa, value, sel, ca, qu, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            "double" => {
                let value = request.value.parse::<u8>().map_err(|e| format!("{}", e))?;
                conn.connection.send_double_command(ioa, value, sel, ca, qu, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            "step" => {
                let value = request.value.parse::<u8>().map_err(|e| format!("{}", e))?;
                conn.connection.send_step_command(ioa, value, sel, ca, qu, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            "setpoint_normalized" => {
                let value = request.value.parse::<f32>().map_err(|e| format!("{}", e))?;
                conn.connection.send_setpoint_normalized(ioa, value, sel, ca, qu, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            "setpoint_scaled" => {
                let value = request.value.parse::<i16>().map_err(|e| format!("{}", e))?;
                conn.connection.send_setpoint_scaled(ioa, value, sel, ca, qu, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            "setpoint_float" => {
                let value = request.value.parse::<f32>().map_err(|e| format!("{}", e))?;
                conn.connection.send_setpoint_float(ioa, value, sel, ca, qu, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            "bitstring" => {
                if sel {
                    return Err("位串命令 (C_BO_NA_1) 无 S/E 位,不支持「仅选择」,请改用「仅执行」".to_string());
                }
                let value = request.bitstring
                    .or_else(|| parse_u32_value(&request.value))
                    .ok_or_else(|| "bitstring 命令需要提供 32 位数值 (bitstring 字段或 value)".to_string())?;
                conn.connection.send_bitstring_command(ioa, value, ca, cot).await
                    .map_err(|e| format!("failed to send command: {}", e))?;
            }
            _ => return Err(format!("unknown command type: {}", request.command_type)),
        }
        let action = if sel { "select_sent" } else { "execute_sent" };
        return Ok(ControlResult {
            steps: vec![ControlStep {
                action: action.to_string(),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            }],
            duration_ms: start.elapsed().as_millis() as u64,
        });
    }
```

> 第 571 行起的 SBO 分支(`// SbO mode: delegate...` 到函数结尾)保持原样不动——由于 Execute/Select 分支 `return`,该段现在只在 `mode == ControlMode::Sbo` 时到达,行为与旧 `select=true` 完全一致。

- [ ] **Step 3: 编译检查**

Run: `cargo build -p iec104master-app 2>&1 | tail -20`
Expected: 编译成功,无 warning 关于未使用的 `ControlMode`/`resolve_control_mode`。

- [ ] **Step 4: 跑全量 crate 测试确认无回归**

Run: `cargo test -p iec104master-app 2>&1 | tail -15`
Expected: `test result: ok.`(含 Task 1 的三个测试)。

- [ ] **Step 5: 提交**

```bash
cd /Users/daichangyu/code/IEC60870-5-104-Simulator
git add crates/iec104master-app/src/commands.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit -m "feat(master): send_control_command 按 control_mode 分支,支持仅选择/仅执行"
```

---

## Task 3: 前端 i18n——新增控制模式文案与类型

**Files:**
- Modify: `master-frontend/src/i18n/locales/zh-CN.ts`(`DictShape.control` 类型段 + `control` 字典段)
- Modify: `master-frontend/src/i18n/locales/en-US.ts`(`control` 字典段)

- [ ] **Step 1: 在 `DictShape` 的 `control` 类型段补 4 个 key**

在 `master-frontend/src/i18n/locales/zh-CN.ts` 的 `DictShape` 里,`control:` 段中 `cotLabel: string` 一行之后(同级)追加:

```ts
    controlMode: string
    modeExecute: string
    modeSelect: string
    modeSbo: string
```

- [ ] **Step 2: 在 zh-CN `control` 字典补中文文案**

在同文件 `const dict: DictShape = { ... }` 的 `control:` 字典段,`cotLabel:` 对应中文项之后追加:

```ts
    controlMode: '控制模式',
    modeExecute: '仅执行 (Execute)',
    modeSelect: '仅选择 (Select)',
    modeSbo: '自动两步 (SBO)',
```

- [ ] **Step 3: 在 en-US `control` 字典补英文文案**

在 `master-frontend/src/i18n/locales/en-US.ts` 的 `control:` 段,`cotLabel:` 对应英文项之后追加:

```ts
    controlMode: 'Control Mode',
    modeExecute: 'Execute only',
    modeSelect: 'Select only',
    modeSbo: 'Auto two-step (SBO)',
```

- [ ] **Step 4: 类型检查通过(确认两个 locale 都补齐,DictShape 一致)**

Run: `cd /Users/daichangyu/code/IEC60870-5-104-Simulator && npx --prefix master-frontend vue-tsc -b master-frontend 2>&1 | tail -20`
Expected: 无 `TS2353`/`TS2741` 等缺 key 错误(若某 locale 漏补会报错)。若该命令路径不便,可直接用 Task 5 的 `npm run build` 兜底校验。

- [ ] **Step 5: 提交**

```bash
cd /Users/daichangyu/code/IEC60870-5-104-Simulator
git add master-frontend/src/i18n/locales/zh-CN.ts master-frontend/src/i18n/locales/en-US.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit -m "feat(master): i18n 新增控制模式(仅执行/仅选择/自动两步)文案"
```

---

## Task 4: 前端 `ControlDialog.vue`——控制模式下拉 + 持久化迁移 + 位串锁定

**Files:**
- Modify: `master-frontend/src/components/ControlDialog.vue`(state 27-52、savePersisted 73-88、payload 162-174、模板 328-334、watch 107-109)

- [ ] **Step 1: 改持久化类型 `Persisted`**

把 `master-frontend/src/components/ControlDialog.vue` 中 `type Persisted = { ... }`(27-39)里的:

```ts
  selectMode: boolean
```

改为:

```ts
  controlMode: 'execute' | 'select' | 'sbo'
```

- [ ] **Step 2: 改 state 定义与迁移**

把第 52 行:

```ts
const selectMode = ref(saved.selectMode ?? false)
```

替换为(读旧 `selectMode` 做一次性迁移;`saved` 是 `Partial<Persisted>`,旧字段用宽松读取):

```ts
// 兼容旧持久化字段 selectMode:true→'sbo',其余→'execute'。
const legacySelectMode = (saved as { selectMode?: boolean }).selectMode
const controlMode = ref<'execute' | 'select' | 'sbo'>(
  saved.controlMode ?? (legacySelectMode ? 'sbo' : 'execute'),
)
```

- [ ] **Step 3: 改 `savePersisted`**

把 `savePersisted`(73-88)中的:

```ts
    selectMode: selectMode.value,
```

改为:

```ts
    controlMode: controlMode.value,
```

- [ ] **Step 4: 位串类型强制"仅执行"(扩展已有的 commandType watch)**

把第 107-109 行的:

```ts
watch(commandType, () => {
  qualifier.value = 0
})
```

替换为:

```ts
watch(commandType, () => {
  qualifier.value = 0
  // 位串 C_BO_NA_1 无 S/E 位,只能仅执行
  if (commandType.value === 'bitstring' && controlMode.value !== 'execute') {
    controlMode.value = 'execute'
  }
})
```

- [ ] **Step 5: 改 payload**

把 `send()` 里的 payload(162-171)中的:

```ts
      select: selectMode.value,
```

改为:

```ts
      control_mode: controlMode.value,
      select: controlMode.value === 'sbo', // 兼容后端旧字段
```

- [ ] **Step 6: 改模板——复选框换下拉**

把模板中的 `toggle-row` 块(328-334):

```html
          <div class="toggle-row">
            <label class="toggle-label" :class="{ 'is-disabled': isBitstring }">
              <input type="checkbox" v-model="selectMode" class="toggle-checkbox" :disabled="isBitstring" />
              <span>{{ t('control.sboLabel') }}</span>
            </label>
            <span class="toggle-hint">{{ isBitstring ? t('control.bitstringNoSbo') : (selectMode ? t('control.sboTwoStep') : t('control.sboDirect')) }}</span>
          </div>
```

替换为:

```html
          <div class="toggle-row">
            <label class="toggle-label">
              <span>{{ t('control.controlMode') }}</span>
              <select v-model="controlMode" class="mode-select" :disabled="isBitstring">
                <option value="execute">{{ t('control.modeExecute') }}</option>
                <option value="select">{{ t('control.modeSelect') }}</option>
                <option value="sbo">{{ t('control.modeSbo') }}</option>
              </select>
            </label>
            <span class="toggle-hint">{{ isBitstring ? t('control.bitstringNoSbo') : (controlMode === 'sbo' ? t('control.sboTwoStep') : t('control.sboDirect')) }}</span>
          </div>
```

- [ ] **Step 7: 加下拉样式**

在 `<style scoped>` 中 `.toggle-checkbox { ... }` 规则之后追加:

```css
.mode-select {
  padding: 4px 8px;
  background: var(--c-surface0);
  border: 1px solid var(--c-surface1);
  border-radius: 4px;
  color: var(--c-text);
  font-size: 12px;
}
.mode-select:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
```

- [ ] **Step 8: 构建校验(类型 + 编译)**

Run: `cd /Users/daichangyu/code/IEC60870-5-104-Simulator/master-frontend && npm run build 2>&1 | tail -15`
Expected: `vue-tsc -b && vite build` 成功,`✓ built`,无 TS 错误(尤其无 `selectMode` 残留引用)。

- [ ] **Step 9: 提交**

```bash
cd /Users/daichangyu/code/IEC60870-5-104-Simulator
git add master-frontend/src/components/ControlDialog.vue
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit -m "feat(master): 遥控对话框控制模式下拉(仅执行/仅选择/自动两步)+持久化迁移"
```

---

## Task 5: 集成验证(Playwright 真实浏览器 + 全量构建)

**Files:**
- Create(临时,验证后删除): `scripts/verify-control-mode.mjs`

- [ ] **Step 1: 后台启动 master dev server**

Run(后台): `cd /Users/daichangyu/code/IEC60870-5-104-Simulator && npm --prefix master-frontend run dev > /tmp/master-dev.log 2>&1`
确认 `/tmp/master-dev.log` 出现 `Local: http://localhost:5177/`。

- [ ] **Step 2: 写验证脚本**

创建 `scripts/verify-control-mode.mjs`:

```js
/** 临时验证:遥控对话框控制模式三选一,断言 send_control_command 的 payload。跑完即删。 */
import { chromium } from 'playwright'
const MASTER = 'http://localhost:5177/'
const conn = {
  id: 'c1', target_address: '127.0.0.1', port: 2404, common_addresses: [1], state: 'Connected',
  use_tls: false, t0: 30, t1: 15, t2: 10, t3: 20, k: 12, w: 8, default_qoi: 20, default_qcc: 5,
  interrogate_period_s: 0, counter_interrogate_period_s: 0, broadcast_address: 65535,
}
function installMock(cfg) {
  try { localStorage.setItem('iec104.language', 'zh-CN') } catch {}
  try { localStorage.removeItem('iec104master.controlDialog.v1') } catch {}
  const DATA = cfg.commands || {}
  window.__INVOKES__ = []
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      if (cmd.indexOf('plugin:event|') === 0) return 1
      window.__INVOKES__.push({ cmd, args: args || null })
      if (cmd === 'send_control_command') return { steps: [{ action: 'x', timestamp: '0' }], duration_ms: 1 }
      return (cmd in DATA) ? DATA[cmd] : null
    },
    transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window['_cb' + id] = cb; return id },
    unregisterCallback: () => {}, convertFileSrc: (p) => p,
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
}
const cmds = {
  list_connections: [conn],
  get_received_data_since: { seq: 0, total_count: 0, points: [] },
  list_data_points_since: { seq: 0, total_count: 0, points: [] },
  get_communication_logs: [], check_for_update: null, set_logging_enabled: null,
}
const browser = await chromium.launch()
const results = []
const check = (n, ok, extra) => { results.push(ok); console.log(`${ok ? '✓' : '✗'} ${n}${extra ? '  ' + extra : ''}`) }
async function openDialog() {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, locale: 'zh-CN' })
  const page = await ctx.newPage()
  await page.addInitScript(installMock, { commands: cmds })
  for (let i = 0; i < 30; i++) { try { await page.goto(MASTER, { waitUntil: 'domcontentloaded' }); break } catch { await page.waitForTimeout(500) } }
  await page.waitForFunction(() => getComputedStyle(document.body).backgroundColor === 'rgb(17, 17, 27)', { timeout: 8000 }).catch(() => {})
  await page.getByText('127.0.0.1', { exact: false }).first().click()
  await page.waitForTimeout(300)
  await page.getByRole('button', { name: /自定义控制/ }).click()
  await page.locator('.mode-select').waitFor({ state: 'visible', timeout: 4000 })
  return { ctx, page }
}
const ctrl = (inv) => inv.filter((x) => x.cmd === 'send_control_command')
try {
  // 仅选择
  {
    const { ctx, page } = await openDialog()
    await page.locator('.mode-select').selectOption('select')
    await page.getByRole('button', { name: /^发送$/ }).click()
    await page.waitForTimeout(300)
    const calls = ctrl(await page.evaluate(() => window.__INVOKES__))
    check('仅选择只调一次', calls.length === 1, `count=${calls.length}`)
    check('payload.control_mode==select', calls[0]?.args?.request?.control_mode === 'select', `mode=${calls[0]?.args?.request?.control_mode}`)
    check('payload.select==false', calls[0]?.args?.request?.select === false, `select=${calls[0]?.args?.request?.select}`)
    await ctx.close()
  }
  // 自动两步
  {
    const { ctx, page } = await openDialog()
    await page.locator('.mode-select').selectOption('sbo')
    await page.getByRole('button', { name: /^发送$/ }).click()
    await page.waitForTimeout(300)
    const calls = ctrl(await page.evaluate(() => window.__INVOKES__))
    check('自动两步 control_mode==sbo', calls[0]?.args?.request?.control_mode === 'sbo', `mode=${calls[0]?.args?.request?.control_mode}`)
    check('自动两步 select==true', calls[0]?.args?.request?.select === true, `select=${calls[0]?.args?.request?.select}`)
    await ctx.close()
  }
  // 位串锁定仅执行
  {
    const { ctx, page } = await openDialog()
    await page.locator('select.form-input').first().selectOption('bitstring')
    await page.waitForTimeout(200)
    const disabled = await page.locator('.mode-select').isDisabled()
    const val = await page.locator('.mode-select').inputValue()
    check('位串下拉禁用', disabled === true, `disabled=${disabled}`)
    check('位串锁定为execute', val === 'execute', `val=${val}`)
    await ctx.close()
  }
} finally { await browser.close() }
const pass = results.filter(Boolean).length
console.log(`\n=== ${pass}/${results.length} 通过 ===`)
if (pass !== results.length) process.exit(1)
```

> 注意:命令类型下拉的 selector 用 `select.form-input`(模板 274 行,`commandType` 那个 `<select class="form-input">`);控制模式下拉用 `.mode-select`。若"自定义控制"按钮文案不同,以 `master-frontend/src/i18n/locales/zh-CN.ts` 的 `toolbar.customControl` 为准。

- [ ] **Step 3: 运行验证**

Run: `cd /Users/daichangyu/code/IEC60870-5-104-Simulator && node scripts/verify-control-mode.mjs 2>&1`
Expected: `=== 7/7 通过 ===`(仅选择 3 项 + 自动两步 2 项 + 位串 2 项)。

- [ ] **Step 4: 全量构建 + Rust 测试兜底**

Run: `cd /Users/daichangyu/code/IEC60870-5-104-Simulator && npm --prefix master-frontend run build 2>&1 | tail -5 && cargo test -p iec104master-app 2>&1 | tail -8`
Expected: 前端 `✓ built`;Rust `test result: ok.`。

- [ ] **Step 5: 清理临时脚本 + 停 dev server**

Run: `cd /Users/daichangyu/code/IEC60870-5-104-Simulator && rm -f scripts/verify-control-mode.mjs && pkill -f "vite --port 5177"; git status --short`
Expected: 工作区只剩本计划已提交的改动,无 `verify-control-mode.mjs` 残留(应为 clean 或仅未追踪的 dev log)。

- [ ] **Step 6: 无新增改动则跳过提交;若 Step 4 触发了任何源文件改动则提交**

```bash
cd /Users/daichangyu/code/IEC60870-5-104-Simulator
git status --short
# 如有源文件改动:
git add -A -- ':!scripts/verify-control-mode.mjs'
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit -m "test(master): 控制模式前端行为验证(Playwright)" || echo "无需提交"
```

---

## 验收对照(self-review)

- 三模式下拉、默认仅执行、与旧"直接执行"一致 → Task 4 Step 2/6。
- 仅选择发 S/E=1 一条、不跟发执行 → Task 2(分支透传 sel)+ Task 1(SCO 编码测试)+ Task 5(payload 验证)。
- 自动两步与现状一致 → Task 2(SBO 分支不动)+ Task 5。
- 位串无法选 select/sbo → Task 2(select 报错)+ Task 4 Step 4/6(锁定)+ Task 5。
- 旧 `selectMode=true` → 自动两步 → Task 4 Step 2。
- 向后兼容 `select` 字段 → Task 1(resolve_control_mode 回退)。
- Rust + 前端构建测试全绿 → Task 2/3/4/5。
