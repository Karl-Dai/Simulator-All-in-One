# macOS Gatekeeper 首次启动引导文档 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 macOS Gatekeeper 首次启动拦截的用户引导,从过时的"右键 → 打开"路径,更新为 macOS 15+ 的 *系统设置 → 仍要打开* 主路径,并把同样内容自动注入到每次 GitHub Release 描述里。

**Architecture:** 纯文档变更:重写 `README_CN.md` 与 `README.md` 的 macOS 章节;在 `scripts/build-release-notes.mjs` 中以常量形式新增段落并由 `buildBody()` 注入,同步在 vitest 测试里加用例。CI、签名、Tauri 配置不动。

**Tech Stack:** Markdown · Node.js ESM script · vitest 4.x

**Spec:** `docs/superpowers/specs/2026-05-15-macos-gatekeeper-onboarding-design.md`

**Spec 修正:** spec §5 的测试代码示例使用 `node:test` 风格,但仓库实际使用 **vitest** (`scripts/package.json` 与现有 `build-release-notes.test.mjs` 已确认)。本计划以 vitest 为准。

---

## File Structure

| 文件 | 责任 | 操作 |
|---|---|---|
| `scripts/build-release-notes.mjs` | 生成 GitHub Release body 的脚本 | 修改:在 `REPO_URL` 之后新增常量 `MACOS_FIRST_LAUNCH_NOTE`;在 `buildBody()` 的 `lines.push('---')` 前注入该常量 |
| `scripts/build-release-notes.test.mjs` | vitest 单元测试 | 修改:新增一个 `it(...)` 用例,断言 release body 含 macOS 首次启动段 |
| `README_CN.md` | 中文 README | 修改:替换 L144–157 整段 |
| `README.md` | 英文 README | 修改:替换 L146–160 整段 |

**不动:** `.github/workflows/release.yml` · `crates/iec104sim-app/tauri.conf.json` · `crates/iec104master-app/tauri.conf.json` · `CHANGELOG.md` · `scripts/gen-update-manifest.mjs`

---

## Task 1: vitest 测试断言 release body 含 macOS 首次启动段

**Files:**
- Modify: `scripts/build-release-notes.test.mjs` (在 L47 `keeps the footer` 用例后新增 `it` 块)

- [ ] **Step 1: 写失败测试**

在 `scripts/build-release-notes.test.mjs` 第 47 行那个 `it('keeps the footer ...', ...)` 用例之后、`})` 收尾之前,新增:

```javascript
  it('includes the macOS first-launch guidance block', () => {
    const body = buildBody('v1.2.3', md)
    expect(body).toContain('macOS 首次启动 / First launch on macOS')
    expect(body).toContain('xattr -dr com.apple.quarantine')
    expect(body).toContain('System Settings → Privacy & Security')
    expect(body).toContain('系统设置 → 隐私与安全性')
    expect(body).toContain('#first-launch-on-macos')
  })
```

- [ ] **Step 2: 跑测试确认它失败**

```bash
cd scripts && npm test -- --run build-release-notes
```

期望:**FAIL**,失败的就是新增的这个 `includes the macOS first-launch guidance block` 用例,其它 5 个用例保持 PASS。

- [ ] **Step 3: 提交失败测试**

```bash
git add scripts/build-release-notes.test.mjs
git commit -m "test(release-notes): assert macOS first-launch block is rendered"
```

---

## Task 2: 在 build-release-notes.mjs 中注入 macOS 首次启动段

**Files:**
- Modify: `scripts/build-release-notes.mjs:19` (在 `REPO_URL` 之后追加常量)
- Modify: `scripts/build-release-notes.mjs:65` (`lines.push('---')` 之前注入)

- [ ] **Step 1: 新增 `MACOS_FIRST_LAUNCH_NOTE` 常量**

打开 `scripts/build-release-notes.mjs`,在 L19 `const REPO_URL = ...` 之后、L21 `const APPS = [...]` 之前插入:

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

- [ ] **Step 2: 在 `buildBody()` 中注入**

在 `buildBody()` 函数里、当前 `lines.push('---')` 那一行**之前**插入两行:

定位上下文(原文 L64–66):

```javascript
    lines.push('')
  }
  lines.push('---')
```

改为:

```javascript
    lines.push('')
  }
  lines.push(MACOS_FIRST_LAUNCH_NOTE)
  lines.push('')
  lines.push('---')
```

- [ ] **Step 3: 跑测试确认全绿**

```bash
cd scripts && npm test -- --run build-release-notes
```

期望:**6 个用例全部 PASS**,包括 Task 1 新增的那个。

- [ ] **Step 4: 手动渲染一次确认输出**

```bash
node scripts/build-release-notes.mjs v1.3.4
grep -A 12 "macOS 首次启动" RELEASE_BODY.md
```

期望输出包含完整的中英文双语段落、`xattr` 命令、README 链接 `#first-launch-on-macos`。

- [ ] **Step 5: 清理临时产物**

```bash
rm RELEASE_BODY.md
```

`RELEASE_BODY.md` 不进版本库 (CI 才生成),手动渲染后清掉。

- [ ] **Step 6: 提交**

```bash
git add scripts/build-release-notes.mjs
git commit -m "feat(release-notes): inject macOS first-launch guidance into Release body"
```

---

## Task 3: 重写 README_CN.md 的 macOS 章节

**Files:**
- Modify: `README_CN.md:144-157`

- [ ] **Step 1: 用 Edit 工具整段替换**

定位旧段落 (`README_CN.md` L144–157):

```markdown
### macOS 安装提示

应用未做 Apple 公证（Notarization）。从 v1.1.2 起 dmg 内的 .app 带 ad-hoc 签名，
首次打开时 macOS 会提示"无法验证开发者"，**右键 → 打开** 即可绕过。

如果你下载的是 v1.1.1 或更早的 dmg，看到 **"已损坏，无法打开"** 提示，是因为
旧版完全没签名，被新 macOS 直接判定为损坏。终端跑一行解决：

```bash
xattr -dr com.apple.quarantine "/Applications/IEC104Master.app"
xattr -dr com.apple.quarantine "/Applications/IEC104Slave.app"
```

或直接升级到 v1.1.2 及以后的版本（应用内"检查更新"也会推过来）。
```

替换为:

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

注意:旧标题里的全角括号「（）」和全角逗号「，」要替换为半角的「()」和「,」,这是 spec 措辞决定的(与新章节其它符号风格一致)。

- [ ] **Step 2: 验证替换后行号与下文衔接正常**

```bash
grep -n "## 许可证\|### macOS" README_CN.md
```

期望:`### macOS 首次启动` 出现一次;`## 许可证` 紧随该章节(中间空一行)。

- [ ] **Step 3: 提交**

```bash
git add README_CN.md
git commit -m "docs(README_CN): rewrite macOS first-launch guidance for Sequoia"
```

---

## Task 4: 重写 README.md 的 macOS 章节

**Files:**
- Modify: `README.md:146-160`

- [ ] **Step 1: 用 Edit 工具整段替换**

定位旧段落 (`README.md` L146–160):

```markdown
### macOS install note

The bundles are **not Apple-notarized** (no paid Developer Program). From v1.1.2 the `.app`
inside the dmg is ad-hoc signed, so on first launch macOS shows the standard "unidentified
developer" warning — right-click → **Open** to bypass.

If you downloaded a v1.1.1 or earlier dmg and see **"is damaged, can't be opened, move to
Trash"**, that's the unsigned-app behaviour newer macOS enforces. Run:

```bash
xattr -dr com.apple.quarantine "/Applications/IEC104Master.app"
xattr -dr com.apple.quarantine "/Applications/IEC104Slave.app"
```

…or upgrade to v1.1.2+ (the in-app updater will push it).
```

替换为:

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

- [ ] **Step 2: 验证锚点串能命中(与 release-notes 链接对齐)**

```bash
grep -n "^### First launch on macOS\|^## License" README.md
```

期望:`### First launch on macOS` 出现一次;`## License` 紧随其后(中间空一行)。

GitHub 把 `### First launch on macOS` 渲染为锚点 `#first-launch-on-macos`(slug 规则:lowercase + 空格转 `-` + 标点剔除),与 Task 2 注入的 release-notes 链接一致。

- [ ] **Step 3: 提交**

```bash
git add README.md
git commit -m "docs(README): rewrite macOS first-launch guidance for Sequoia"
```

---

## Task 5: 最终整合验证

**Files:** 无修改,仅运行验证命令

- [ ] **Step 1: 重跑 vitest 全套**

```bash
cd scripts && npm test
```

期望:**所有用例 PASS**,包括 Task 1 新增的 `includes the macOS first-launch guidance block`。

- [ ] **Step 2: 端到端渲染一次最新 tag 的 release body**

```bash
LATEST_TAG=$(git describe --tags --abbrev=0)
node scripts/build-release-notes.mjs "$LATEST_TAG"
cat RELEASE_BODY.md
```

人工核对:
- 顶部下载表完整
- CHANGELOG 该 tag 的段落被嵌入
- macOS 首次启动 / First launch on macOS 段紧跟在 CHANGELOG 与 `---` 分隔线之间
- README 链接锚点 `#first-launch-on-macos` 正确

清理:

```bash
rm RELEASE_BODY.md
```

- [ ] **Step 3: README 双语字数对照**

```bash
awk '/^### macOS 首次启动$/,/^## 许可证/' README_CN.md | wc -w
awk '/^### First launch on macOS$/,/^## License/' README.md | wc -w
```

期望:两数差距不超过 30%(中文 wc -w 受空格切分限制会偏小,目测两段都涵盖了:警告原因、两条放行路径、`xattr` 命令、旧版"损坏"段)。

- [ ] **Step 4: 全局自检 grep**

```bash
git grep -n "右键 → 打开\|right-click → Open\|右键打开" -- README.md README_CN.md
```

期望:**0 命中** (旧路径已彻底移除)。

```bash
git grep -n "macos-install-note\|macos-安装提示" -- '*.md' '*.mjs' '*.json'
```

期望:**0 命中** (旧锚点已无引用)。

- [ ] **Step 5: 完整提交日志检查**

```bash
git log --oneline -n 5
```

期望看到 4 个新提交,顺序为:

```
docs(README): rewrite macOS first-launch guidance for Sequoia
docs(README_CN): rewrite macOS first-launch guidance for Sequoia
feat(release-notes): inject macOS first-launch guidance into Release body
test(release-notes): assert macOS first-launch block is rendered
```

无需额外 commit。

---

## Self-Review

**Spec 覆盖检查 (对照 `docs/superpowers/specs/2026-05-15-macos-gatekeeper-onboarding-design.md`):**

- spec §3 `README_CN.md` L144–157 替换 → Task 3 ✓
- spec §3 `README.md` L146–160 替换 → Task 4 ✓
- spec §3 `build-release-notes.mjs` 增加 `MACOS_FIRST_LAUNCH_NOTE` 常量 + buildBody 注入 → Task 2 ✓
- spec §3 `build-release-notes.test.mjs` 新增测试 → Task 1 ✓ (修正为 vitest 风格)
- spec §3 "不动" 范围 → 计划全程未触碰,Task 5 Step 4 grep 兜底验证 ✓
- spec §4.1 / §4.2 / §4.3 文档措辞逐字落到 Task 2 / 3 / 4 ✓
- spec §5 验证清单 (vitest 跑、手动渲染、锚点核对、字数对照) → Task 2 Step 3-4 + Task 5 全覆盖 ✓
- spec §6 风险锚点变更 → Task 5 Step 4 grep `macos-install-note` 验证无外链 ✓

**占位符扫描:** 全文无 TBD / TODO / "之后实现" / "类似 Task N";所有代码块完整可拷;所有命令都给出期望输出。

**类型与命名一致性:** 常量名 `MACOS_FIRST_LAUNCH_NOTE` 在 Task 2 Step 1 (定义) 与 Step 2 (调用) 一致;锚点 `#first-launch-on-macos` 在 Task 2 模板字符串 / Task 1 测试 / Task 4 README 标题三处一致;`xattr -dr com.apple.quarantine` 命令在 README × 2、release-notes 模板、测试用例四处完全一致。
