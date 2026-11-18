# Updater CN Proxy Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让国内访问不到 github.com 的用户也能成功使用应用内更新检查与下载,而不破坏国外用户的现有升级路径。

**Architecture:** CI 在 release 时为每个 role(slave/master)生成 4 份 manifest(原版 + 3 份 CN 版本,后者把 `platforms[*].url` 加上 ghfast.top / gh-proxy.com / gh.idayer.com 前缀;签名串完全相同因 proxy 透传 byte-equal)。两个 `tauri.conf.json` 的 `updater.endpoints` 改成有序 4 项,proxy 在前 github 兜底,利用 Tauri v2 updater 的 manifest 级 fallback。前端在手动检查失败时弹出引导,提供"打开 ghfast.top 镜像页"按钮。详见 `docs/superpowers/specs/2026-05-20-updater-cn-proxy-design.md`。

**Tech Stack:** Node 20 + vitest(scripts 测试)/ Rust + Tauri 2(updater, opener plugin)/ Vue 3 + TypeScript + vue-i18n(前端)/ GitHub Actions(release.yml)/ minisign(签名)。

---

## File Structure

新增:

- `scripts/test-update-proxies.sh` —— 一次性 proxy 可用性探测,无 CI 依赖
- 在 release assets 中新增:`latest-slave-cn{1,2,3}.json` 与 `latest-master-cn{1,2,3}.json`(运行时生成,无源码文件)

修改:

- `scripts/gen-update-manifest.mjs` —— 提取 `buildManifestForRole(role, grouped, notes, pubDate, urlPrefix)` 工具函数,循环 4 个前缀写出 4 份文件
- `scripts/gen-update-manifest.test.mjs` —— 增加 URL 前缀注入的测试
- `scripts/build-release-notes.mjs` —— 在 `buildBody` 顶部插入 CN 镜像引导横幅,优先级最高(放在下载表格上方)
- `scripts/build-release-notes.test.mjs` —— 增加镜像引导文案测试
- `.github/workflows/release.yml` —— `publish-manifest` 的上传步骤把新文件追加到 `gh release upload` 列表
- `crates/iec104sim-app/tauri.conf.json` —— `updater.endpoints` 改 4 项
- `crates/iec104master-app/tauri.conf.json` —— 同上
- `crates/iec104sim-app/Cargo.toml` + `crates/iec104master-app/Cargo.toml` —— 增加 `tauri-plugin-opener = "2"`
- `crates/iec104sim-app/src/lib.rs` + `crates/iec104master-app/src/lib.rs` —— 注册 `tauri-plugin-opener`
- `crates/iec104sim-app/capabilities/default.json` + master 同名 —— 加上 `opener:allow-open-url` 权限
- `frontend/src/components/Toolbar.vue` + `master-frontend/src/components/Toolbar.vue` —— 在 `manualCheckUpdate` 失败分支用 `showConfirm` + `openUrl` 引导用户到镜像
- 4 个 i18n locale 文件(slave 与 master 各 zh-CN/en-US)—— 增加 `toolbar.updateCheckFailedMirrorPrompt` 与 `toolbar.openMirrorAction` 键
- `README.md` + `README_CN.md` —— 在下载小节增加"国内镜像"段落

每个文件都有单一职责:manifest 生成只在 `gen-update-manifest.mjs`;UI 引导只在两个 Toolbar.vue;URL 前缀常量本身放在 `gen-update-manifest.mjs` 顶部,不外泄。

---

## Task 1: 把 proxy 探测脚本提交到 `scripts/`

设计文档 §5.1 要求脚本入仓,以便将来换 proxy 时先跑一遍。当前 `/tmp/test-update-proxies.sh` 是 brainstorming 阶段写的临时版,这里规范化提交。

**Files:**
- Create: `scripts/test-update-proxies.sh`

- [ ] **Step 1: 创建脚本**

Create `scripts/test-update-proxies.sh`:

```bash
#!/usr/bin/env bash
# 无头测试 GitHub 反代是否能正确透传 Tauri updater 所需的 manifest 与 bundle。
# 用途:更换 endpoint 名单前先跑一遍验证。手动执行,不接入 CI。
# 退出码:0 = 至少一个 proxy 可用;1 = 基线获取失败。
set -u

REPO="Karl-Dai/IEC60870-5-104-Simulator"
MANIFEST_PATH="releases/latest/download/latest-master.json"
ORIGIN="https://github.com/$REPO/$MANIFEST_PATH"

PROXIES=(
  "https://ghfast.top"
  "https://gh-proxy.com"
  "https://gh.idayer.com"
)

now_ms() { python3 -c 'import time;print(int(time.time()*1000))'; }

TMP=$(mktemp -d)
echo "=== 1) 基线: 直连 github.com 拉 manifest ==="
T0=$(now_ms)
HTTP_CODE=$(curl -sSL -o "$TMP/origin.json" -w "%{http_code}" --max-time 30 "$ORIGIN" || echo "ERR")
T1=$(now_ms)
ORIGIN_MS=$((T1-T0))
ORIGIN_SIZE=$(wc -c < "$TMP/origin.json" | tr -d ' ')
ORIGIN_SHA=$(shasum -a 256 "$TMP/origin.json" | awk '{print $1}')
echo "  $ORIGIN -> HTTP $HTTP_CODE  ${ORIGIN_MS}ms  ${ORIGIN_SIZE}B  sha256=${ORIGIN_SHA:0:16}…"

if [ "$HTTP_CODE" != "200" ] || [ "$ORIGIN_SIZE" = "0" ]; then
  echo "::error:: 基线失败"; head -5 "$TMP/origin.json"; exit 1
fi

BUNDLE_URL=$(node -e "const j=JSON.parse(require('fs').readFileSync('$TMP/origin.json','utf8'));const p=j.platforms||{};const k=p['windows-x86_64']||p['darwin-aarch64']||p['linux-x86_64']||Object.values(p)[0];process.stdout.write(k.url||'')")
echo "  bundle URL: $BUNDLE_URL"

T0=$(now_ms)
curl -sSL -o "$TMP/origin.bin" -H "Range: bytes=0-1023" --max-time 30 "$BUNDLE_URL"
T1=$(now_ms)
ORIGIN_BIN_MS=$((T1-T0))
ORIGIN_BIN_SHA=$(shasum -a 256 "$TMP/origin.bin" | awk '{print $1}')
ORIGIN_BIN_SIZE=$(wc -c < "$TMP/origin.bin" | tr -d ' ')
echo "  bundle[0..1023] ${ORIGIN_BIN_SIZE}B  ${ORIGIN_BIN_MS}ms  sha256=${ORIGIN_BIN_SHA:0:16}…"
echo

echo "=== 2) 各 proxy 透传测试 ==="
printf "%-28s %-7s %-8s %-9s %-11s %-7s %-11s\n" "PROXY" "MFT_HC" "MFT_MS" "MFT_SZ" "MFT_OK" "BIN_MS" "BIN_OK"
OK_COUNT=0
for P in "${PROXIES[@]}"; do
  MURL="$P/$ORIGIN"
  T0=$(now_ms)
  HC=$(curl -sSL -o "$TMP/proxy.json" -w "%{http_code}" --max-time 20 "$MURL" 2>/dev/null || echo "ERR")
  T1=$(now_ms); MMS=$((T1-T0))
  SZ=$(wc -c < "$TMP/proxy.json" 2>/dev/null | tr -d ' ')
  SHA=$(shasum -a 256 "$TMP/proxy.json" 2>/dev/null | awk '{print $1}')
  [ "$SHA" = "$ORIGIN_SHA" ] && MOK="OK" || MOK="FAIL"

  BURL="$P/$BUNDLE_URL"
  T0=$(now_ms)
  curl -sSL -o "$TMP/proxy.bin" -H "Range: bytes=0-1023" --max-time 30 "$BURL" 2>/dev/null
  T1=$(now_ms); BMS=$((T1-T0))
  BSHA=$(shasum -a 256 "$TMP/proxy.bin" 2>/dev/null | awk '{print $1}')
  BSZ=$(wc -c < "$TMP/proxy.bin" 2>/dev/null | tr -d ' ')
  if [ "$BSHA" = "$ORIGIN_BIN_SHA" ]; then BOK="OK"; else BOK="FAIL(${BSZ}B)"; fi

  printf "%-28s %-7s %-8s %-9s %-11s %-7s %-11s\n" "$P" "$HC" "${MMS}ms" "${SZ}B" "$MOK" "${BMS}ms" "$BOK"
  if [ "$MOK" = "OK" ] && [ "$BOK" = "OK" ]; then OK_COUNT=$((OK_COUNT+1)); fi
done

echo
if [ $OK_COUNT -eq 0 ]; then
  echo "::warning:: 没有任何 proxy 同时通过 manifest+bundle 透传测试"
fi
exit 0
```

- [ ] **Step 2: 添加可执行权限**

Run:
```bash
chmod +x scripts/test-update-proxies.sh
```

- [ ] **Step 3: 跑一次确认逻辑无误**

Run:
```bash
bash scripts/test-update-proxies.sh
```
Expected: 至少有一个 proxy 同时 manifest+bundle 列显示 `OK`(基于设计阶段实测,ghfast.top / gh-proxy.com / gh.idayer.com 均应通过)。

- [ ] **Step 4: 提交**

```bash
git add scripts/test-update-proxies.sh
git commit -m "chore(scripts): 增加 GitHub proxy 探测脚本"
```

---

## Task 2: `gen-update-manifest.mjs` 生成 4 份 manifest(TDD)

设计 §3.1 要求 manifest 生成函数接受 URL 前缀参数,并按固定后缀映射输出 4 份文件。

**Files:**
- Modify: `scripts/gen-update-manifest.mjs`
- Modify: `scripts/gen-update-manifest.test.mjs`

- [ ] **Step 1: 先写失败的单元测试**

在 `scripts/gen-update-manifest.test.mjs` 末尾追加(import 一并补上):

```javascript
import { buildManifest, MANIFEST_VARIANTS } from './gen-update-manifest.mjs'

describe('MANIFEST_VARIANTS', () => {
  it('declares 4 variants in proxy-first / github-last order', () => {
    expect(MANIFEST_VARIANTS).toEqual([
      { suffix: '-cn1', prefix: 'https://ghfast.top/' },
      { suffix: '-cn2', prefix: 'https://gh-proxy.com/' },
      { suffix: '-cn3', prefix: 'https://gh.idayer.com/' },
      { suffix: '',     prefix: null },
    ])
  })
})

describe('buildManifest', () => {
  const platforms = {
    'windows-x86_64': { signature: 'SIG', url: 'https://github.com/u/r/releases/download/v1/a.exe' },
    'darwin-aarch64': { signature: 'SIG2', url: 'https://github.com/u/r/releases/download/v1/b.tar.gz' },
  }
  const base = { version: '1.0.0', notes: 'n', pub_date: '2026-01-01T00:00:00Z', platforms }

  it('returns the original manifest unchanged when prefix is null', () => {
    expect(buildManifest(base, null)).toEqual(base)
  })

  it('prepends the prefix to every platform url, leaving signature untouched', () => {
    const got = buildManifest(base, 'https://ghfast.top/')
    expect(got.platforms['windows-x86_64'].url).toBe('https://ghfast.top/https://github.com/u/r/releases/download/v1/a.exe')
    expect(got.platforms['darwin-aarch64'].url).toBe('https://ghfast.top/https://github.com/u/r/releases/download/v1/b.tar.gz')
    expect(got.platforms['windows-x86_64'].signature).toBe('SIG')
    expect(got.platforms['darwin-aarch64'].signature).toBe('SIG2')
  })

  it('does not mutate the input manifest', () => {
    const snapshot = JSON.parse(JSON.stringify(base))
    buildManifest(base, 'https://ghfast.top/')
    expect(base).toEqual(snapshot)
  })
})
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd scripts && npm test -- --run gen-update-manifest
```
Expected: 3 个新增 test 全部失败,提示 `buildManifest`/`MANIFEST_VARIANTS` is not exported。

- [ ] **Step 3: 在 `scripts/gen-update-manifest.mjs` 实现并改造 `main`**

修改 `scripts/gen-update-manifest.mjs`:

(a) 在文件顶部 `const PLATFORM_PATTERNS = [...]` 之后,新增导出:

```javascript
// 与 `crates/*/tauri.conf.json` 的 `updater.endpoints` 顺序保持一致(proxy 在前,
// github 兜底)。修改顺序请同步两个 tauri.conf.json。
export const MANIFEST_VARIANTS = [
  { suffix: '-cn1', prefix: 'https://ghfast.top/' },
  { suffix: '-cn2', prefix: 'https://gh-proxy.com/' },
  { suffix: '-cn3', prefix: 'https://gh.idayer.com/' },
  { suffix: '',     prefix: null },
]

export function buildManifest(manifest, urlPrefix) {
  if (!urlPrefix) return manifest
  const platforms = {}
  for (const [k, v] of Object.entries(manifest.platforms)) {
    platforms[k] = { signature: v.signature, url: `${urlPrefix}${v.url}` }
  }
  return { ...manifest, platforms }
}
```

(b) 把 `main()` 末尾写盘那段(目前是 `writeFileSync(out, JSON.stringify(manifest, null, 2))`)改为按变体循环:

```javascript
    const manifest = { version, notes, pub_date: pubDate, platforms }
    for (const { suffix, prefix } of MANIFEST_VARIANTS) {
      const variant = buildManifest(manifest, prefix)
      const out = resolve(process.cwd(), `latest-${role}${suffix}.json`)
      writeFileSync(out, JSON.stringify(variant, null, 2))
      console.log(`wrote ${out}`)
    }
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd scripts && npm test -- --run gen-update-manifest
```
Expected: 所有测试 PASS(原有 `groupAssetsByRole` / `extractChangelogSection` + 新增 3 个)。

- [ ] **Step 5: 提交**

```bash
git add scripts/gen-update-manifest.mjs scripts/gen-update-manifest.test.mjs
git commit -m "feat(scripts): manifest 生成支持 CN proxy 多变体"
```

---

## Task 3: `release.yml` 上传 8 份 manifest

`publish-manifest` job 现在只上传 2 份,需要扩展为 8 份(每 role 4 份)。

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 定位上传步骤**

打开 `.github/workflows/release.yml`,找到第 254-263 行的 `Upload manifests to release` 步骤。

- [ ] **Step 2: 替换上传文件列表**

将该步骤里的:
```yaml
            if gh release upload ${{ github.ref_name }} latest-slave.json latest-master.json --clobber; then exit 0; fi
```

替换为:
```yaml
            if gh release upload ${{ github.ref_name }} \
              latest-slave.json latest-slave-cn1.json latest-slave-cn2.json latest-slave-cn3.json \
              latest-master.json latest-master-cn1.json latest-master-cn2.json latest-master-cn3.json \
              --clobber; then exit 0; fi
```

- [ ] **Step 3: 本地干跑生成 manifest 验证文件名匹配**

为避免在 CI 跑完才发现 manifest 文件名拼错,先在本地干跑 `gen-update-manifest.mjs` 看输出文件名:

```bash
# 找最近一次发布的 tag(只用于读 release 资产生成 manifest)
LAST_TAG=$(git tag --sort=-v:refname | head -1)
cd scripts && node gen-update-manifest.mjs "$LAST_TAG"
cd .. && ls latest-*.json
```
Expected: 列出恰好 8 个文件 `latest-{slave,master}{,-cn1,-cn2,-cn3}.json`,文件名与 release.yml 中的列表完全一致。然后:
```bash
rm latest-*.json
```

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): 上传 CN proxy manifest 变体"
```

---

## Task 4: 修改两个 `tauri.conf.json` 的 `updater.endpoints`

设计 §2.2 给出的有序 4 项 endpoint。

**Files:**
- Modify: `crates/iec104sim-app/tauri.conf.json`
- Modify: `crates/iec104master-app/tauri.conf.json`

- [ ] **Step 1: 改 slave 的 endpoints**

把 `crates/iec104sim-app/tauri.conf.json` 里:
```json
      "endpoints": [
        "https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-slave.json"
      ],
```
替换为:
```json
      "endpoints": [
        "https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-slave-cn1.json",
        "https://gh-proxy.com/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-slave-cn2.json",
        "https://gh.idayer.com/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-slave-cn3.json",
        "https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-slave.json"
      ],
```

- [ ] **Step 2: 改 master 的 endpoints**

把 `crates/iec104master-app/tauri.conf.json` 里:
```json
      "endpoints": [
        "https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-master.json"
      ],
```
替换为:
```json
      "endpoints": [
        "https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-master-cn1.json",
        "https://gh-proxy.com/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-master-cn2.json",
        "https://gh.idayer.com/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-master-cn3.json",
        "https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest/download/latest-master.json"
      ],
```

- [ ] **Step 3: JSON 语法校验**

```bash
node -e "JSON.parse(require('fs').readFileSync('crates/iec104sim-app/tauri.conf.json','utf8'))"
node -e "JSON.parse(require('fs').readFileSync('crates/iec104master-app/tauri.conf.json','utf8'))"
```
Expected: 都无输出(JSON 合法)。

- [ ] **Step 4: 提交**

```bash
git add crates/iec104sim-app/tauri.conf.json crates/iec104master-app/tauri.conf.json
git commit -m "feat(updater): endpoints 增加 CN proxy fallback"
```

---

## Task 5: `build-release-notes.mjs` 顶部加 CN 镜像引导(TDD)

设计 §4.1 第 2 条要求从本次发版起每个 release 都在 notes 最上方带镜像引导。

**Files:**
- Modify: `scripts/build-release-notes.mjs`
- Modify: `scripts/build-release-notes.test.mjs`

- [ ] **Step 1: 先写失败的测试**

在 `scripts/build-release-notes.test.mjs` 里的 `describe('buildBody', () => { ... })` 内追加用例:

```javascript
  it('places the CN mirror banner above the download table', () => {
    const body = buildBody('v1.2.3', md)
    const bannerIdx = body.indexOf('ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest')
    const tableIdx = body.indexOf('## 下载 / Downloads')
    expect(bannerIdx).toBeGreaterThanOrEqual(0)
    expect(tableIdx).toBeGreaterThanOrEqual(0)
    expect(bannerIdx).toBeLessThan(tableIdx)
  })
  it('mentions the one-time manual download caveat', () => {
    const body = buildBody('v1.2.3', md)
    expect(body).toMatch(/首次升级|first.*upgrade/i)
  })
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd scripts && npm test -- --run build-release-notes
```
Expected: 两个新 test 失败(找不到 `ghfast.top` 字符串)。

- [ ] **Step 3: 在 `buildBody` 中插入横幅**

在 `scripts/build-release-notes.mjs` 的 `buildBody` 函数内,`lines.push('## 下载 / Downloads')` 这行**之前**(即版本标题之后)插入:

```javascript
  // 国内用户首次从旧版升级时,updater 仍指向 github.com,大概率拉不到;
  // 引导他们直接走镜像下载安装包。新版本起 updater 已带 proxy fallback。
  lines.push('> 🇨🇳 **中国大陆用户**:首次从旧版升级若失败,请直接从镜像下载安装包(新版本起更新检查会自动走 proxy):')
  lines.push('>')
  lines.push(`> <https://ghfast.top/${REPO_URL}/releases/latest>`)
  lines.push('>')
  lines.push('> 🌍 **Users in mainland China**: if the in-app updater fails on first upgrade from an older version, download installers from the mirror above (later versions will auto-fallback through proxies).')
  lines.push('')
```

注:`REPO_URL` 已存在(`const REPO_URL = 'https://github.com/${REPO}'`),直接复用。

- [ ] **Step 4: 跑测试确认通过**

```bash
cd scripts && npm test -- --run build-release-notes
```
Expected: 所有 test PASS(含原有 4 个 + 新增 2 个)。

- [ ] **Step 5: 提交**

```bash
git add scripts/build-release-notes.mjs scripts/build-release-notes.test.mjs
git commit -m "feat(release-notes): 顶部加 CN 镜像首次升级引导"
```

---

## Task 6: 添加 `tauri-plugin-opener` 并注册(两个 app)

前端要在更新检查失败时打开外部 URL,Tauri 2 默认不允许 `window.open`,需要 `tauri-plugin-opener`。

**Files:**
- Modify: `crates/iec104sim-app/Cargo.toml`
- Modify: `crates/iec104master-app/Cargo.toml`
- Modify: `crates/iec104sim-app/src/lib.rs`
- Modify: `crates/iec104master-app/src/lib.rs`
- Modify: `crates/iec104sim-app/capabilities/default.json`
- Modify: `crates/iec104master-app/capabilities/default.json`
- Modify: `frontend/package.json`
- Modify: `master-frontend/package.json`

- [ ] **Step 1: 加 Rust 依赖**

在 `crates/iec104sim-app/Cargo.toml` 的 `[dependencies]` 段、`tauri-plugin-dialog = "2"` 之后追加一行:

```toml
tauri-plugin-opener = "2"
```

在 `crates/iec104master-app/Cargo.toml` 同位置追加同一行。

- [ ] **Step 2: 注册插件**

在 `crates/iec104sim-app/src/lib.rs` 的 `tauri::Builder::default()` 链式调用上,把:
```rust
        .plugin(tauri_plugin_dialog::init())
```
之后追加:
```rust
        .plugin(tauri_plugin_opener::init())
```

`crates/iec104master-app/src/lib.rs` 同样追加。

- [ ] **Step 3: 加 capability 权限**

修改 `crates/iec104sim-app/capabilities/default.json`,把 `permissions` 数组改为:
```json
  "permissions": [
    "core:default",
    "dialog:allow-open",
    "dialog:allow-save",
    "opener:allow-open-url"
  ]
```

`crates/iec104master-app/capabilities/default.json` 做同样改动。

- [ ] **Step 4: 加前端依赖**

在 `frontend/package.json` 的 `dependencies` 段加入:
```json
    "@tauri-apps/plugin-opener": "^2"
```

`master-frontend/package.json` 同样追加。

- [ ] **Step 5: 安装并编译验证**

```bash
cd frontend && npm install && cd ..
cd master-frontend && npm install && cd ..
cargo check -p iec104sim-app
cargo check -p iec104master-app
```
Expected: 两个 `cargo check` 都 PASS(可能要等 `tauri-plugin-opener` crate 下载)。

- [ ] **Step 6: 提交**

```bash
git add crates/iec104sim-app/Cargo.toml crates/iec104master-app/Cargo.toml \
        crates/iec104sim-app/src/lib.rs crates/iec104master-app/src/lib.rs \
        crates/iec104sim-app/capabilities/default.json crates/iec104master-app/capabilities/default.json \
        frontend/package.json frontend/package-lock.json \
        master-frontend/package.json master-frontend/package-lock.json \
        Cargo.lock
git commit -m "chore: 接入 tauri-plugin-opener (用于打开外部链接)"
```

---

## Task 7: 前端更新失败镜像引导(slave + master)

设计 §3.4 要求 toolbar 手动检查失败时弹引导。当前实现(`Toolbar.vue:40-42`)只是把 Rust 错误字符串包进 alert。改为 confirm:用户点确认即打开镜像。

**Files:**
- Modify: `frontend/src/components/Toolbar.vue`
- Modify: `master-frontend/src/components/Toolbar.vue`
- Modify: `frontend/src/i18n/locales/zh-CN.ts`
- Modify: `frontend/src/i18n/locales/en-US.ts`
- Modify: `master-frontend/src/i18n/locales/zh-CN.ts`
- Modify: `master-frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1: 在 slave i18n zh-CN 增加 key 声明与值**

打开 `frontend/src/i18n/locales/zh-CN.ts`,在第 34-37 行(`toolbar` 类型块内 `updateCheckFailed: string` 附近)插入一个新字段声明:
```typescript
    updateCheckFailedMirrorPrompt: string
```

然后到 264-267 行附近(`toolbar` 值块的 `updateCheckFailed: '更新检查失败',`)后追加:
```typescript
    updateCheckFailedMirrorPrompt: '更新检查失败,可能无法访问 GitHub。是否打开国内镜像下载页面?',
```

只加 `updateCheckFailedMirrorPrompt` 一个键。`showConfirm` 的按钮文案由 dialog 内置的"确定/取消"提供,不能定制,因此**不**新增"打开镜像"按钮文案 key。

- [ ] **Step 2: 在 slave i18n en-US 加同名键**

打开 `frontend/src/i18n/locales/en-US.ts`,定位到 `updateCheckFailed: 'Update check failed',`(约第 39 行),其后追加:
```typescript
    updateCheckFailedMirrorPrompt: 'Update check failed, possibly because GitHub is unreachable. Open the mainland China mirror download page?',
```

(只追加一行,en-US 文件没有独立的类型块,所以无需修改其它位置。)

- [ ] **Step 3: 在 master 两个 i18n 文件做同样改动**

在 `master-frontend/src/i18n/locales/zh-CN.ts` 的 `toolbar` 类型块内(约 24-27 行,`updateCheckFailed: string` 附近)插入:
```typescript
    updateCheckFailedMirrorPrompt: string
```
在同文件 `toolbar` 值块(约 255-258 行,`updateCheckFailed: '更新检查失败',` 之后)追加:
```typescript
    updateCheckFailedMirrorPrompt: '更新检查失败,可能无法访问 GitHub。是否打开国内镜像下载页面?',
```

在 `master-frontend/src/i18n/locales/en-US.ts`(约 26-29 行,`updateCheckFailed: 'Update check failed',` 之后)追加:
```typescript
    updateCheckFailedMirrorPrompt: 'Update check failed, possibly because GitHub is unreachable. Open the mainland China mirror download page?',
```

- [ ] **Step 4: 改 slave Toolbar.vue 的失败分支**

打开 `frontend/src/components/Toolbar.vue`,在文件顶部 import 区(约第 1-10 行附近,已经存在的 `import { invoke } from '@tauri-apps/api/core'` 一类)追加:

```typescript
import { openUrl } from '@tauri-apps/plugin-opener'
```

注入区里把:
```typescript
const { showAlert, showPrompt } = inject<{
  showAlert: typeof ShowAlert
  showPrompt: typeof ShowPrompt
}>(dialogKey)!
```
改为(增加 `showConfirm`):
```typescript
import type { showAlert as ShowAlert, showPrompt as ShowPrompt, showConfirm as ShowConfirm } from '@shared/composables/useDialog'
const { showAlert, showPrompt, showConfirm } = inject<{
  showAlert: typeof ShowAlert
  showPrompt: typeof ShowPrompt
  showConfirm: typeof ShowConfirm
}>(dialogKey)!
```

注:原有 `import type ... from '@shared/composables/useDialog'` 那一行需相应改为上面这一行(在原行基础上补 `showConfirm`)。

把 `manualCheckUpdate` 函数:
```typescript
async function manualCheckUpdate() {
  if (updateChecking.value) return
  updateChecking.value = true
  try {
    const meta = await checkUpdate(true)
    if (!meta) await showAlert(t('toolbar.alreadyLatest'))
  } catch (e) {
    await showAlert(`${t('toolbar.updateCheckFailed')}: ${e}`)
  } finally {
    updateChecking.value = false
  }
}
```
改为:
```typescript
const MIRROR_RELEASE_URL = 'https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest'

async function manualCheckUpdate() {
  if (updateChecking.value) return
  updateChecking.value = true
  try {
    const meta = await checkUpdate(true)
    if (!meta) await showAlert(t('toolbar.alreadyLatest'))
  } catch (e) {
    console.warn('update check failed', e)
    const wantMirror = await showConfirm(t('toolbar.updateCheckFailedMirrorPrompt'))
    if (wantMirror) {
      try {
        await openUrl(MIRROR_RELEASE_URL)
      } catch (err) {
        await showAlert(`${t('toolbar.updateCheckFailed')}: ${err}`)
      }
    }
  } finally {
    updateChecking.value = false
  }
}
```

- [ ] **Step 5: 确认 `showConfirm` 已在 App.vue 中 provide(应无改动)**

`frontend/src/App.vue:14` 已经 `import { showAlert, showConfirm, showPrompt, dialogKey }`,`:78` 已经 `provide(dialogKey, { showAlert, showConfirm, showPrompt })`。本步只需确认现状,无代码改动:

```bash
grep -n "provide(dialogKey" frontend/src/App.vue
```
Expected: 输出包含 `showConfirm`。若不包含,把 provide 对象补上 `showConfirm`(同时确保 import 也含 `showConfirm`)。

- [ ] **Step 6: 对 master Toolbar.vue 做完整改造**

在 `master-frontend/src/components/Toolbar.vue` 的 import 区(已有 `import { invoke } from ...` 一类的位置)追加一行:
```typescript
import { openUrl } from '@tauri-apps/plugin-opener'
```

把当前 dialog 注入(约第 22-25 行附近):
```typescript
const checkUpdate = inject<(force?: boolean) => Promise<UpdateMeta | null>>('checkUpdate')!
```
其上方/附近若已有 `showAlert/showPrompt` 注入则补上 `showConfirm`;若 master 当前未注入 dialog,则新增:
```typescript
import type { showAlert as ShowAlert, showConfirm as ShowConfirm } from '@shared/composables/useDialog'
import { dialogKey } from '@shared/composables/useDialog'
const { showAlert, showConfirm } = inject<{
  showAlert: typeof ShowAlert
  showConfirm: typeof ShowConfirm
}>(dialogKey)!
```
(具体取舍由现有代码已有什么决定;最终结果是文件里能直接调用 `showAlert` 和 `showConfirm`。)

把 master 的 `manualCheckUpdate`(约 25-32 行):
```typescript
async function manualCheckUpdate() {
  if (updateChecking.value) return
  updateChecking.value = true
  try {
    const meta = await checkUpdate(true)
    if (!meta) await showAlert(t('toolbar.alreadyLatest'))
  } catch (e) {
    await showAlert(`${t('toolbar.updateCheckFailed')}: ${e}`)
  } finally {
    updateChecking.value = false
  }
}
```
替换为(与 slave 完全一致,只是文件不同):
```typescript
const MIRROR_RELEASE_URL = 'https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest'

async function manualCheckUpdate() {
  if (updateChecking.value) return
  updateChecking.value = true
  try {
    const meta = await checkUpdate(true)
    if (!meta) await showAlert(t('toolbar.alreadyLatest'))
  } catch (e) {
    console.warn('update check failed', e)
    const wantMirror = await showConfirm(t('toolbar.updateCheckFailedMirrorPrompt'))
    if (wantMirror) {
      try {
        await openUrl(MIRROR_RELEASE_URL)
      } catch (err) {
        await showAlert(`${t('toolbar.updateCheckFailed')}: ${err}`)
      }
    }
  } finally {
    updateChecking.value = false
  }
}
```

确认 `master-frontend/src/App.vue:13,107` 已经 import 并 provide 了 `showConfirm`(现状已满足):
```bash
grep -n "provide(dialogKey" master-frontend/src/App.vue
```
Expected: 输出包含 `showConfirm`。

- [ ] **Step 7: 编译两端前端**

```bash
cd frontend && npm run build && cd ..
cd master-frontend && npm run build && cd ..
```
Expected: TypeScript 编译 + Vite 构建均 PASS。

- [ ] **Step 8: 提交**

```bash
git add frontend/src/components/Toolbar.vue master-frontend/src/components/Toolbar.vue \
        frontend/src/i18n/locales/zh-CN.ts frontend/src/i18n/locales/en-US.ts \
        master-frontend/src/i18n/locales/zh-CN.ts master-frontend/src/i18n/locales/en-US.ts \
        frontend/src/App.vue master-frontend/src/App.vue
git commit -m "feat(ui): 更新检查失败引导用户打开镜像下载页"
```

---

## Task 8: README 加镜像入口

设计 §4.1 第 3 条。

**Files:**
- Modify: `README.md`
- Modify: `README_CN.md`

- [ ] **Step 1: 找下载小节**

```bash
grep -n "download\|Download\|下载" README.md README_CN.md | head -20
```
记录两个文件里"Download / 下载"小节标题所在行号。

- [ ] **Step 2: 在 README_CN.md 的下载小节后插入镜像段落**

在 `README_CN.md` 的下载小节的第一个段落之后,插入:

```markdown
### 国内镜像 (China mirror)

中国大陆用户访问 GitHub Releases 可能不稳定,推荐通过镜像直接下载安装包:

- <https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest>

应用内更新功能从 v1.x.x(即包含本次改动的发布版本)起会自动通过多个反代回退,无需手动处理。但**首次从旧版升级**时,旧版二进制中编译进的 endpoint 仍是 github.com,如果检查更新失败,请按上面镜像链接手动下载新版安装一次,后续更新即可自动通过 proxy。
```

(其中 `v1.x.x` 在 release 时由 release 流程的实际版本号体现;此处保留占位是 README 静态文档的常见做法,无需在每次 release 时改。)

- [ ] **Step 3: 在 README.md 的下载小节后插入同样段落的英文版**

```markdown
### China mirror

Users in mainland China may have unstable access to GitHub Releases. Recommended mirror for direct installer downloads:

- <https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest>

Starting from the version that includes this change, the in-app updater automatically falls back through multiple proxies — no manual action needed. However, **the very first upgrade from an older version** uses the endpoint compiled into the old binary (github.com only); if the in-app update check fails, please download and install the new version once via the mirror above, after which the updater will route through proxies automatically.
```

- [ ] **Step 4: 提交**

```bash
git add README.md README_CN.md
git commit -m "docs: README 增加国内镜像入口与首次升级说明"
```

---

## Task 9: 端到端自查与最终提交

集中跑一遍所有自动化检查,确保前 8 个任务的产物没有相互冲突。

- [ ] **Step 1: 全量跑 scripts 测试**

```bash
cd scripts && npm test && cd ..
```
Expected: 所有 vitest 用例 PASS。

- [ ] **Step 2: cargo 全量 check**

```bash
cargo check --workspace
```
Expected: 无 error,只有可能的 warnings。

- [ ] **Step 3: 两端前端构建**

```bash
cd frontend && npm run build && cd ..
cd master-frontend && npm run build && cd ..
```
Expected: 均成功。

- [ ] **Step 4: JSON 配置语法二次校验**

```bash
for f in crates/iec104sim-app/tauri.conf.json crates/iec104master-app/tauri.conf.json \
         crates/iec104sim-app/capabilities/default.json crates/iec104master-app/capabilities/default.json; do
  node -e "JSON.parse(require('fs').readFileSync('$f','utf8'))" && echo "OK $f"
done
```
Expected: 4 行 OK 输出。

- [ ] **Step 5: Endpoints 顺序与 MANIFEST_VARIANTS 顺序一致性自查**

```bash
node -e '
const c1 = require("./crates/iec104sim-app/tauri.conf.json").plugins.updater.endpoints;
const c2 = require("./crates/iec104master-app/tauri.conf.json").plugins.updater.endpoints;
const m = require("./scripts/gen-update-manifest.mjs");
console.log("slave endpoint count:", c1.length);
console.log("master endpoint count:", c2.length);
console.log("variants:", m.MANIFEST_VARIANTS.length);
// 顺序一致性:每个 endpoint 的 prefix 应该匹配对应 variant.prefix
for (let i = 0; i < m.MANIFEST_VARIANTS.length; i++) {
  const v = m.MANIFEST_VARIANTS[i];
  const expectSlavePrefix = v.prefix || "https://github.com/";
  const expectMasterPrefix = v.prefix || "https://github.com/";
  const okSlave = c1[i].startsWith(expectSlavePrefix);
  const okMaster = c2[i].startsWith(expectMasterPrefix);
  console.log(i, v.suffix, "slave:", okSlave, "master:", okMaster);
  if (!okSlave || !okMaster) process.exit(1);
}
console.log("OK: endpoints 顺序与 MANIFEST_VARIANTS 一致");
'
```
Expected: 输出 `OK: endpoints 顺序与 MANIFEST_VARIANTS 一致`,且 endpoint count = 4。

- [ ] **Step 6: 提交分支(如果尚未到 PR 阶段就跳过)**

工作分支当前为 `feat/point-config-import-export`。所有 8 个任务的提交已经按任务边界分别 commit,无需额外 squash。本任务无新文件改动,不需要 commit。

- [ ] **Step 7: 在 CHANGELOG.md 顶部记录变更**

打开 `CHANGELOG.md`,在最顶端的"未发布"或最新版本段落之上,加入:

```markdown
## [Unreleased]

### Changed

- **Updater**: 应用内更新增加中国大陆 proxy fallback(ghfast.top / gh-proxy.com / gh.idayer.com),并在检查失败时引导用户打开镜像下载页。首次从旧版升级仍需走 github.com,失败时请用 README 中的镜像 URL 手动下载一次。
```

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 记录 updater CN proxy fallback"
```

---

## 验证清单(给 reviewer / QA)

实际上线后:

- [ ] 在能访问 github.com 的机器上跑 `bash scripts/test-update-proxies.sh`,确认 3 个 proxy 都通过
- [ ] 用旧版 v1.3.13 启动,toolbar 检查更新,确认能取到新版 manifest(走 github.com 兜底)并升级成功
- [ ] 升级到新版后再次检查更新,通过应用日志或 devtools 确认首个 endpoint(ghfast.top)返回 200
- [ ] 在新版本里 `sudo` 把 ghfast.top / gh-proxy.com / gh.idayer.com 都加进 `/etc/hosts` 指向 127.0.0.1,再触发检查 → 应弹镜像引导对话框,确认点"打开镜像"能正确打开 ghfast.top releases 页
- [ ] 检查 release notes 渲染结果包含镜像引导横幅且位于下载表格上方
- [ ] 检查 release assets 列表里 8 个 `latest-*.json` 文件全部到位

如有任一未通过,回到对应 Task 修正。
