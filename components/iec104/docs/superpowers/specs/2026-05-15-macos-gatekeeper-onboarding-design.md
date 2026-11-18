# macOS Gatekeeper 首次启动引导文档更新

- 日期: 2026-05-15
- 状态: 已批准 (用户在 brainstorming 阶段确认方案 B)
- 范围: 文档 (README × 2 + release-notes 模板); 不动 CI / 签名 / Tauri 配置

## 1. 背景

GitHub Releases 下载的 macOS dmg 在首次双击时,被 macOS 15 (Sequoia) 起的新 Gatekeeper 拦截,弹窗为:

> 未打开 "IEC104Slave" — Apple 无法验证 "IEC104Slave" 是否包含可能危害 Mac 安全或泄漏隐私的恶意软件。

按钮仅 *完成* / *移到废纸篓*,没有 *打开*。这是因为 app 仅做了 ad-hoc 签名 (codesign --sign -),没有付费的 Apple Developer ID + Notarization。

现有 `README_CN.md` L144–157 与 `README.md` L146–160 的 "macOS 安装提示 / install note" 章节写的是旧版 *右键 → 打开* 路径,该路径在 macOS 15+ 已被 Apple 移除,内容过时。

## 2. 决策

不投入付费 Apple Developer Program,**接受 Gatekeeper 警告永远出现**;通过更新文档让用户能清晰地放行 app。

替代方案 (已否决):

| 方案 | 否决原因 |
|---|---|
| A. Developer ID + Notarization | $99/年,需用户提供苹果实名开发者账号;此次不投入 |
| C. 额外 ad-hoc 签名 | tauri-action 已默认做 ad-hoc 签名,加无加;且 ad-hoc 并不能消除 Gatekeeper 警告 |

## 3. 触达点

| 文件 | 位置 | 改动 |
|---|---|---|
| `README_CN.md` | L144–157 整段 | 替换为下方 §4.1 中文章节 |
| `README.md` | L146–160 整段 | 替换为下方 §4.2 英文章节 |
| `scripts/build-release-notes.mjs` | 在 `lines.push('---')` (现 L65) 之前注入 macOS 首次启动段;将段落字符串提取为顶层常量 `MACOS_FIRST_LAUNCH_NOTE` 便于复用与测试 | 见 §4.3 |
| `scripts/build-release-notes.test.mjs` | 新增一个 `node:test` 用例 | 见 §5 |

**不动:**

- `.github/workflows/release.yml`
- `crates/iec104sim-app/tauri.conf.json` / `crates/iec104master-app/tauri.conf.json`
- `CHANGELOG.md` (这是文档修正,不是功能变更)
- `scripts/gen-update-manifest.mjs` 与其测试

## 4. 文档措辞

### 4.1 README_CN.md 新章节

```markdown
### macOS 首次启动

应用未做 Apple 公证(Notarization),首次双击 `.app` 时,macOS 会弹窗
"未打开 IEC104Slave / IEC104Master — Apple 无法验证…",只提供 *完成* 与
*移到废纸篓* 两个按钮。这是 macOS 15 (Sequoia) 起的标准拦截,**不是软件损坏**。

**放行步骤(任选其一):**

1. **图形界面**:
   - 双击 `.app`,出现拦截弹窗,点 *完成*
   - 打开 *系统设置 → 隐私与安全性*,滚到底部
   - 看到"已阻止 IEC104Slave 的使用…",点 *仍要打开* → 输入密码
   - 弹窗变为 *打开*,点击即可,以后双击直接启动

2. **终端一行命令**(快):

   ```bash
   xattr -dr com.apple.quarantine "/Applications/IEC104Slave.app"
   xattr -dr com.apple.quarantine "/Applications/IEC104Master.app"
   ```

   命令执行后 `.app` 不再被隔离标记,双击即开。

如果你看到 *"已损坏,无法打开"* 而不是上面的对话框,那是 v1.1.1 及更早完全无签名的旧版,
请升级到 v1.1.2 以上(应用内"检查更新"也会推过来),或用上面的 `xattr` 命令清掉
隔离属性即可。
```

### 4.2 README.md 新章节

```markdown
### First launch on macOS

The bundles are **not Apple-notarized** (no paid Developer Program). On first launch
macOS shows *"IEC104Slave / IEC104Master cannot be opened — Apple could not verify…"*
with only *Done* and *Move to Trash* buttons. This is the standard macOS 15 (Sequoia)
block for ad-hoc-signed apps — the app is **not damaged**.

**Allow it (pick one):**

1. **GUI path**:
   - Double-click the `.app`, see the block dialog, click *Done*
   - Open *System Settings → Privacy & Security*, scroll to the bottom
   - You'll see *"IEC104Slave was blocked…"*, click *Open Anyway* → enter password
   - The next dialog has an *Open* button; click it. Subsequent launches go straight through.

2. **One-line Terminal**:

   ```bash
   xattr -dr com.apple.quarantine "/Applications/IEC104Slave.app"
   xattr -dr com.apple.quarantine "/Applications/IEC104Master.app"
   ```

   Strips the quarantine flag so macOS stops blocking.

If you instead see *"is damaged, can't be opened"*, that's a v1.1.1-or-earlier build
with no signature at all — upgrade to v1.1.2+ (the in-app updater will push it) or
run the `xattr` command above.
```

### 4.3 Release notes 模板增量

在 `scripts/build-release-notes.mjs` 顶部新增常量:

```javascript
const MACOS_FIRST_LAUNCH_NOTE = [
  '## macOS 首次启动 / First launch on macOS',
  '',
  '首次双击 `.app` 会被 Gatekeeper 拦截("Apple 无法验证…")。放行:打开',
  '*系统设置 → 隐私与安全性*,滚到底点 *仍要打开*;或终端执行',
  '`xattr -dr com.apple.quarantine "/Applications/IEC104Slave.app"`。',
  `详细步骤见 [README](${REPO_URL}#first-launch-on-macos)。`,
  '',
  'First launch is blocked by Gatekeeper ("Apple could not verify…"). To allow:',
  '*System Settings → Privacy & Security*, scroll to bottom, click *Open Anyway*; or',
  'run `xattr -dr com.apple.quarantine "/Applications/IEC104Slave.app"` in Terminal.',
  `Full steps in the [README](${REPO_URL}#first-launch-on-macos).`,
].join('\n')
```

`buildBody()` 在 `lines.push('---')` 前插入:

```javascript
lines.push(MACOS_FIRST_LAUNCH_NOTE)
lines.push('')
```

## 5. 测试

`scripts/build-release-notes.test.mjs` 新增用例 (使用现有 `node:test` 风格):

```javascript
test('buildBody includes macOS first-launch guidance', () => {
  const body = buildBody('v9.9.9', '## v9.9.9 - 2026-05-15\n\n- test\n')
  assert.match(body, /macOS 首次启动 \/ First launch on macOS/)
  assert.match(body, /xattr -dr com\.apple\.quarantine/)
  assert.match(body, /System Settings → Privacy & Security/)
  assert.match(body, /#first-launch-on-macos/)
})
```

### 验证清单

1. `node --test scripts/build-release-notes.test.mjs` — 全部测试通过
2. `node scripts/build-release-notes.mjs v1.3.4` — 本地手动跑一次,人工检查 `RELEASE_BODY.md` 包含 macOS 首次启动段且锚点链接拼对
3. GitHub 渲染 README:确认英文 H3 `### First launch on macOS` 生成的锚点为 `#first-launch-on-macos` (GitHub markdown slug 规则)
4. 中英文 README 段落字数差不超过 30%,信息量对等 (人工目测)

## 6. 风险与缓解

| 风险 | 缓解 |
|---|---|
| README 英文 H3 锚点从 `#macos-install-note` 变为 `#first-launch-on-macos`,可能破坏外链 | 项目内 grep 无引用;外部用户引用未知,可接受 |
| macOS 14 及更早用户仍可走 *右键 → 打开*,文档不写他们会更绕 | 当前用户群体以 macOS 14+ 为主;若日后收到 macOS 13 反馈再加回备注段 |
| `xattr` 命令对 `/Applications/` 下非当前用户 owner 的 app 可能需要 `sudo` | 文档暂不加 `sudo`;若收到失败反馈再精化 |
| Release-notes 模板里硬编码英文锚点,中文段落引用同一锚点 | GitHub 渲染中只有一个英文锚点;两段链接指向同一锚点是预期行为 |

## 7. 不在本次范围

- 任何 Apple 签名 / 公证流程接入 (留待未来若决定上 Developer ID 时另起 spec)
- 应用内自动放行 (Tauri 不能从 app 内部解除自己的 quarantine 属性)
- Windows SmartScreen 等价问题 (与 macOS 无关,已有单独的便携 exe 路径)
