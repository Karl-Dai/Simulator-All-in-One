# 主站移除「广播对时」前端入口设计

- 日期: 2026-06-07
- 范围: `master-frontend/`(仅前端)
- 状态: 设计已批准,待写 spec 审阅
- 关联: `docs/superpowers/specs/2026-05-28-master-broadcast-interrogation-design.md`(广播功能本体);子站侧「对齐主站全召唤广播」设计(子站广播只对齐 GI+CI,不含对时)互为主/子站两侧

## 1. 目标

撤掉主站工具栏「广播 ▾」菜单中的**广播对时**入口,使主站广播能力与子站广播应答语义闭环——子站广播只应答 GI(总召)与 CI(计量召唤),不应答对时,故主站不再向用户暴露「广播对时」。

**删除深度(用户决策):仅摸前端入口,后端命令保留。** 后端 `send_broadcast_clock_sync` 命令与注册原样不动,变成前端不可达但仍注册的命令(留作 API/预留)。

## 2. 背景与现状

主站「广播 ▾」拆分按钮(`master-frontend/src/components/Toolbar.vue`)下拉含三项:
- 广播总召 → `invoke('send_broadcast_gi')`
- **广播对时 → `invoke('send_broadcast_clock_sync')`** ← 本次移除
- 广播计量召唤 → `invoke('send_broadcast_counter_read')`

后端 `crates/iec104master-app/src/commands.rs::send_broadcast_clock_sync`(读连接 `broadcast_address` → `conn.send_clock_sync(bcast)`)与 `crates/iec104master-app/src/lib.rs` 的 `invoke_handler` 注册均存在。`MasterClient::send_clock_sync` / `build_clock_sync_command` 同时服务常规逐 CA 对时,**不可误删**。

## 3. 非目标

- 不删后端 `send_broadcast_clock_sync` 命令及其 `lib.rs` 注册(用户明确保留)
- 不动常规逐 CA 对时(`sendClockSync` / `send_clock_sync` / 工具栏「对时」按钮 / i18n `clockSync`)
- 不动广播总召、广播计量召唤
- 不动 `master.rs` 的 `build_clock_sync_command` 及 `build_clock_sync_broadcast_ffff_emits_le_ff_ff` 单测(后端能力与测试仍有效)
- 不改子站(子站对齐为独立设计)

## 4. 改动清单(仅前端)

| 文件 | 改动 |
|---|---|
| `master-frontend/src/components/Toolbar.vue` | 删下拉项 `<li @click="sendBroadcastClockSync">{{ t('toolbar.broadcastClockSync') }}</li>`(约 line 381);删 `sendBroadcastClockSync()` 函数(约 line 299-305,自包含:仅 `invoke` + 关菜单) |
| `master-frontend/src/i18n/locales/zh-CN.ts` | 删类型声明 `broadcastClockSync: string`(约 line 39)+ 值 `broadcastClockSync: '广播对时'`(约 line 292) |
| `master-frontend/src/i18n/locales/en-US.ts` | 删值 `broadcastClockSync: 'Broadcast Clock Sync'`(约 line 41);类型接口集中在 zh-CN,en 仅实现值 |

结果:「广播 ▾」菜单 **3 项 → 2 项**(广播总召 / 广播计量召唤)。i18n key 三处(类型 + 两语言值)一致删除,消费者(唯一引用 = 被删的 `<li>`)同步移除,类型自洽。

行号为快照,实现时以符号为准。

## 5. 验证

- **`npm run build`(master-frontend)通过**——i18n key 从「类型 + 两语言值」三处一致删除后类型自洽(遵循 memory `shared_frontend_tauri_imports`:前端验证用 `npm run build`,非 `vue-tsc --noEmit`)。
- **Playwright 无头实测**(遵循 memory `feedback_frontend_headless_verify`:前端改动须真实浏览器):启动主站 GUI → 点「广播 ▾」→ 断言下拉**恰好 2 项**(广播总召 / 广播计量召唤),无「广播对时」。
- `cargo build` workspace 仍编译(后端未动;保留的 `send_broadcast_clock_sync` 仍在 `invoke_handler` 引用,无死代码告警)。

## 6. 风险

- **会话起始 `master-frontend/src/components/Toolbar.vue` 已有未提交改动**(git `M`)——本删除叠加其上。遵循 memory `project_icloud_reverts_edits`:改完 `grep` 复核(确认 `broadcastClockSync` / `sendBroadcastClockSync` 全仓 0 命中)+ 尽快 commit,靠 build/Playwright 守门。
- iCloud 长会话异步还原(memory `project_icloud_reverts_edits` / `project_icloud_git_dir_sync_hazard`):本变更不引入新写盘路径,但编辑后须立即 grep 复核 + commit。
