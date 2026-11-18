# Updater CN Proxy Fallback —— 设计文档

- **日期**: 2026-05-20
- **范围**: `crates/iec104sim-app` (slave) 与 `crates/iec104master-app` (master) 两个 Tauri 应用的应用内更新通道
- **背景**: 现状 `updater.endpoints` 只配置了 `github.com/.../releases/latest/download/latest-<role>.json` 一个地址,中国大陆用户长期访问不稳定,导致应用内更新检查/下载失败,实际等同于"通不了 GitHub 的用户就用不了更新功能"

## 1. 目标

让 manifest 拉取与 bundle 下载在国内用户处也能成功完成,同时:

- 不破坏国外用户当前的升级路径
- 不在 Rust 端引入复杂 hook(避免与 `tauri-plugin-updater` 升级绑死)
- 不依赖单一第三方反代(任意一家挂掉时仍有替补)
- minisign 签名验证保持有效(代理只能透传字节,不能伪造内容)

## 2. 选定方案: CI 生成多份 manifest + 多 endpoint fallback

### 2.1 为什么不是其它候选

| 方案 | 否决原因 |
|---|---|
| 自建更新服务器(VPS / OSS) | 维护成本高,且并未脱离签名分发可信链需求 |
| 运行时在 Rust 端重写 bundle URL | `tauri-plugin-updater` 没有干净的 URL 重写 hook;绕过去要自己解析 manifest + 重组 Update,易与签名校验冲突,且每次 plugin 升级都可能破 |
| 仅在 `endpoints` 加 proxy 前缀,manifest 内容不变 | Tauri updater 拉到 manifest 后会**用 manifest 里写的 url** 去下载 bundle,manifest 里的 url 是 `github.com/.../releases/download/...`,bundle 阶段仍然卡在 github.com |

### 2.2 选定方案要点

1. `scripts/gen-update-manifest.mjs` 一次性生成多份 manifest,每份对应一个 endpoint:
   - `latest-<role>.json` —— 原版,`platforms[*].url` 指向 `github.com`(国外用户走这份)
   - `latest-<role>-cn1.json` —— `platforms[*].url` 前缀为 `https://ghfast.top/`
   - `latest-<role>-cn2.json` —— `platforms[*].url` 前缀为 `https://gh-proxy.com/`
   - `latest-<role>-cn3.json` —— `platforms[*].url` 前缀为 `https://gh.idayer.com/`
   - 各份 manifest 的 `signature` 字段完全相同(基于安装包字节的 minisign 签名;代理只透传,字节不变,签名仍然有效)
2. `release.yml` 的 `publish-manifest` job 上传新增的 3 份 CN manifest 到 release assets
3. 两个 `tauri.conf.json` 的 `updater.endpoints` 改成 4 项,顺序为 proxy 在前 / github 兜底:
   ```
   https://ghfast.top/https://github.com/.../latest-<role>-cn1.json
   https://gh-proxy.com/https://github.com/.../latest-<role>-cn2.json
   https://gh.idayer.com/https://github.com/.../latest-<role>-cn3.json
   https://github.com/.../latest-<role>.json
   ```

### 2.3 选定 proxy 的依据(无头测试结果, 2026-05-20)

测试脚本对每个候选 proxy 执行:① 拉取真实 `latest-master.json` 与原版 byte-equal 比对;② Range 0-1023 拉取 release bundle 与原版 byte-equal 比对。

| Proxy | Manifest | Bundle 透传 | 结论 |
|---|---|---|---|
| ghfast.top | ✅ 200, 1.75s | ✅ byte-equal, 1.29s | 采用 |
| gh-proxy.com | ✅ 200, 1.03s | ✅ byte-equal, 1.37s | 采用 |
| gh.idayer.com | ✅ 200, 1.53s | ✅ byte-equal, 0.99s | 采用 |
| ghproxy.net | ❌ 502 on manifest 路径 | ✅ | 淘汰 |
| mirror.ghproxy.com | ❌ 连接错误 | ✅ | 淘汰 |

注:测试在境外节点执行;直连 github.com 的 manifest 拉取耗时 2.35s,proxy 反而更快(三者都是 Cloudflare,github bundle 是 S3)—— 这说明"proxy 在前"的排序对国外用户也没有惩罚。

## 3. 实现细节

### 3.1 `scripts/gen-update-manifest.mjs`

现有逻辑:遍历每个 role,组装 `{ version, notes, pub_date, platforms }` 写入 `latest-<role>.json`。

改造:抽出一个 `buildManifestWithUrlPrefix(basePlatforms, prefix)` 工具函数,在写盘前按下表循环生成 4 份文件:

| 后缀 | URL 前缀 | 对应 endpoint |
|---|---|---|
| `''`(无后缀) | 无 | `github.com` 兜底 |
| `-cn1` | `https://ghfast.top/` | proxy #1 |
| `-cn2` | `https://gh-proxy.com/` | proxy #2 |
| `-cn3` | `https://gh.idayer.com/` | proxy #3 |

`signature` 字段不依赖 url,所有 4 份直接复用同一签名串。

### 3.2 `.github/workflows/release.yml`

`publish-manifest` 的"Upload manifests to release"步骤把文件名列表扩展为:
```
latest-slave.json latest-slave-cn1.json latest-slave-cn2.json latest-slave-cn3.json \
latest-master.json latest-master-cn1.json latest-master-cn2.json latest-master-cn3.json
```
保留现有的 5-attempt 重试循环不动。

### 3.3 `crates/iec104sim-app/tauri.conf.json` 与 `crates/iec104master-app/tauri.conf.json`

`updater.endpoints` 改为 4 个 URL 的数组,顺序见 2.2。`pubkey` 字段不变。

### 3.4 失败提示(前端,master 与 slave 共用)

现状(`crates/*/src/update.rs`):`check_for_update` 已经把错误字符串 `Err(String)` 返回给前端;Rust 端不需要改。

前端改动(`shared-frontend` 或各自 `frontend/`、`master-frontend/` 的 toolbar 更新按钮处):

- 现状下手动触发检查失败时,UI 已有 toast,但内容是裸的 Rust 错误字符串,对用户不友好
- 改为:当 `force=true` 路径返回 Err 时,弹一个对话框/toast 包含:
  - 文案:「检查更新失败,可能是网络无法访问 GitHub。你可以直接打开发布页手动下载安装包。」
  - 两个按钮:「打开 ghfast.top 镜像」→ `https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest`;「打开 GitHub 发布页」→ 原始 URL
- 启动自动检查(`force=false`)失败仍然静默,与现状一致

### 3.5 Tauri updater fallback 行为注意点

`tauri-plugin-updater` v2 的 fallback **只在拉取 manifest 这一步生效**:它会按顺序尝试 `endpoints`,第一个能拿到合法 manifest 的就停止,然后用 **那份 manifest 里写死的 url** 去下载 bundle —— bundle 下载失败不会再回退到下一个 endpoint 的 manifest。

也就是说,我们做的是"manifest 级别的多链路 fallback",不是"bundle 级别的多链路 fallback"。

接受这个折衷的理由:无头测试表明同一个 proxy 上 manifest 和 bundle 的可用性高度一致,不存在"manifest 通但 bundle 不通"的常见组合。如果将来确实出现这种情况,可以在前端 install 失败时再次给出"打开镜像页"的引导,无需重新设计。

## 4. 向后兼容与首次过渡

这是设计的固有限制,必须明确:

`updater.endpoints` 数组在编译期固化进二进制,**这次改造只影响这次发版及之后版本的二进制**。已经装在用户机器上的旧版(≤ v1.3.13)仍然只会去 `github.com/.../latest-<role>.json` 这一个地址。

三类用户的实际升级表现:

| 用户 | 旧版 → 新版的升级路径 | 是否需要手动介入 |
|---|---|---|
| 国外 / 能通 github | 旧版 updater → `github.com/.../latest-<role>.json`(文件名保留)→ 取到新 manifest → 下载新 bundle → 装上后自动获得 4 endpoint fallback | 否 |
| 国内但偶发能通 github | 同上,可能要重试 | 否 |
| 国内长期通不了 github | 旧版 updater 卡死在 github.com → 本次改造不生效 → 必须从镜像手动下载一次安装包 | **是,仅这一次** |

### 4.1 缓解措施

1. **保留原文件名**:`latest-<role>.json` 仍照常生成并上传(就是 §2.2 里那份"github.com 兜底"manifest)→ 国外用户升级路径零破坏
2. **release notes 顶部加引导**:`scripts/build-release-notes.mjs` 渲染时在最上方插入一行高亮文案:
   ```
   🇨🇳 中国大陆用户首次升级请从镜像下载:
   https://ghfast.top/https://github.com/Karl-Dai/IEC60870-5-104-Simulator/releases/latest
   ```
   从本次发版起每个 release 都带,首次升级后用户即获得 4 endpoint fallback,后续自更新无需镜像
3. **README 镜像入口**:在 README.md / README_CN.md 的"下载"小节增加镜像 URL

## 5. 测试与验收

### 5.1 自动化(无头)

复用本设计阶段写过的脚本(`/tmp/test-update-proxies.sh`),作为一次性验证脚本提交到 `scripts/test-update-proxies.sh`,在 README 中说明"如需更换 proxy 名单,先跑这个脚本验证可用性"。不进入 CI(GitHub Actions runner 与最终用户网络不可比)。

### 5.2 端到端

发版后用 v1.3.13 作为对照,手动验证:

1. **国外节点**:从 v1.3.13 启动应用,toolbar 检查更新 → 应能取到新版本并升级成功(走 github.com 兜底 endpoint)
2. **新版本启动后**:再次触发检查 → 看 devtools / 日志,确认首个 endpoint(ghfast.top)返回 200 且升级链路通畅
3. **故意断网模拟**:在新版本里通过 hosts 屏蔽 ghfast.top → 应能回退到 gh-proxy.com → 再屏蔽 gh-proxy.com → 应能回退到 gh.idayer.com → 全屏蔽 → UI 弹镜像引导 toast

### 5.3 签名验证

`minisign` 签名基于 bundle 字节,proxy 透传 byte-equal 已在 §2.3 验证。新版本首次升级时,Tauri 内部会进行 minisign 验签,任何 proxy 篡改字节都会导致升级失败而非静默装上 —— 这是设计本身保留的安全属性。

## 6. 风险与未尽事项

- **proxy 服务方稳定性**:三家公益反代任何一家停服都会拉长 fallback 等待时间。每次发版前(或定期 monthly)跑一次 `scripts/test-update-proxies.sh`,如发现某家失效,下次发版替换名单。
- **proxy 限流**:ghfast.top 等会对单 IP 的下载并发限流;Tauri updater 是单流下载,通常不会触发,但若有大量企业用户同 NAT 出口,有可能。届时考虑自建 Cloudflare Workers 反代作为第四 endpoint。
- **endpoint 数组长度上限**:Tauri v2 updater 未文档化此限制,但 4 个属于常规范围,无风险。
- **manifest 文件名后缀约定 (`-cn1`/`-cn2`/`-cn3`)**:语义不直观,但能让 release assets 列表里一眼看出顺序;不打算改成 `-ghfast` 这种带 proxy 名的命名,避免将来换 proxy 时 endpoint URL 必须改。如果将来想做"按 proxy 命名",此处会有一次小迁移。

## 7. 不在本次范围

- 应用安装包的初次分发(README 已经在前述 §4.1 加引导即可)
- 自建更新服务器(未来如果 proxy 全部不可用再考虑)
- 增加"用户手动切换更新通道"的 UI 设置项(YAGNI,fallback 已经自动处理)
- 修改 Tauri updater plugin 本身的源码
