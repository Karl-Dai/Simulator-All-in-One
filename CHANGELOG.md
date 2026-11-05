# Changelog

Notable changes are documented in English and Chinese, following Keep a Changelog.

## [0.1.3] - 2026-11-05

### Highlights / 亮点

- Install eight industrial protocol simulators together and open each in its own window. / 一次安装八个工业协议模拟器，每个程序使用独立窗口。
- The launcher adapts to narrow and wide windows, with visible favorites and launch feedback. / 入口适配宽窄窗口，提供收藏和明确的启动反馈。
- Shared appearance controls reach the launcher, `ModbusMaster` and `IEC104Master`; the other six applications retain their existing controls. / 入口、`ModbusMaster` 和 `IEC104Master` 试点共享外观控件，其余六端保留原设置入口。
- Public source and a manual release pipeline provide four native build targets with package checksums. / 源码公开，手动发版流程提供四个原生构建目标及安装包校验值。

### Added / 新增

- Shared appearance contracts, status labels and error messages, with temporary suite language overrides that preserve standalone preferences. / 新增公共外观接口、状态标签和错误提示；套件语言使用临时覆盖，保留独立版偏好。
- Central toolchain lock and build receipts; collection and packaging reject stale or mismatched records. / 集中锁定工具链并记录构建信息，收集和打包拒绝过期或不匹配的记录。
- Manual release workflow validates source, builds macOS ARM/Intel, Windows x64 and Linux x64, then verifies all six archives before publication. / 手动发版流程检查源码，构建四个原生目标，并在发布前核验六份安装归档。

### Changed / 改进

- English-first Simulator All-in-One branding, bilingual launcher and responsive protocol cards with persistent favorites. / 统一 Simulator All-in-One 品牌，默认英文，支持双语入口、响应式协议卡片和持久收藏。
- Recursive shared-resource synchronization records content digests while retaining source, patch, payload-tree and signature checks. / 公共资源递归同步并记录摘要，保留源码、补丁、资源树和签名检查。

### Fixed / 修复

- Windows native menu integration and Modbus initial slave point address areas. / 修复 Windows 原生菜单集成和 Modbus 从站初始点位地址区配置。
- Appearance bindings remain available in both standalone and suite Wails builds; standalone methods reject suite configuration access. / Wails 独立版和套件构建均生成稳定的外观绑定，独立版拒绝访问套件配置。

### Validation limits / 验收边界

- macOS packages use ad-hoc signatures and are not notarized. Full GUI/protocol acceptance on every platform, macOS 13 hardware and offline first installation without WebView2 remain pending. / macOS 安装包使用 ad-hoc 签名，尚未公证；全平台完整窗口及协议验收、macOS 13 实机、无 WebView2 的断网首次安装仍待验证。
- Automated checks and the detailed historical evidence are described in `docs/implementation-status.md`. / 自动检查和历史验收证据见 `docs/implementation-status.md`。
