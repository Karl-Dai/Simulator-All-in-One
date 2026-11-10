# Changelog

Notable changes are documented in English and Chinese, following Keep a Changelog.

## [Unreleased]

- 内置模拟器统一使用 SAIO 版本，保留上游源码与补丁摘要用于诊断。 / Bundled simulators share the SAIO version; source and patch identities remain available for diagnostics.
- Windows 改为单个便携 EXE，自动校验、解压及修复程序缓存，保留用户工程。 / Windows builds use one portable EXE with verified extraction and cache repair, preserving user projects.
- WebView2 不随包携带；缺少或过旧时提示打开 SimLab 下载页面。 / WebView2 is a separate prerequisite; missing or outdated runtimes trigger a native download prompt.

## [0.1.4] - 2026-11-05

### Highlights / 亮点

- Reliable Modbus discovery finds responding slave IDs and valid registers inside sparse address ranges. / 修复 Modbus 在线从站漏扫和不连续寄存器地址漏扫。
- Scan a chosen slave, review results, and add verified ranges as polling groups without duplicates. / 可指定从站扫描、查看结果，再将有效范围添加为轮询组，重复添加会跳过已有组。
- Protocol icons now carry `104`, `UA`, and `P` badges; Windows title bars follow the selected appearance. / 协议图标增加 `104`、`UA`、`P` 标注，Windows 标题栏跟随所选主题。

### Fixed / 修复

- Slave discovery sends numeric ID arrays to the WebView and recognizes device exception replies while excluding gateway delivery failures. / 从站扫描向界面传递数字 ID 数组，识别设备异常响应，并排除网关转发失败。
- Register discovery uses the chosen slave ID and splits rejected blocks to recover valid addresses; connection failures are shown explicitly. / 寄存器扫描使用指定从站 ID，将读取失败的地址块拆分以找出有效地址，并明确显示连接错误。
- Cancellation interrupts pending network reads; progress uses completed counts, and large result sets are paginated. / 中止扫描会打断等待中的网络读取，进度按已扫描数量计算，大量结果分页显示。
- Connection and polling settings can be edited from the context menu; menus close outside the tree or on Escape, and project reload preserves default-slave inheritance. / 右键可编辑连接和轮询设置，在树外点击或按 Escape 会关闭菜单，重新打开项目仍保留默认从站继承关系。
- Modbus dialogs retain edits while dragging and support explicit TLS 1.2/1.3 selection alongside certificate and server-name verification settings. / 修复 Modbus 弹窗拖动时意外关闭的问题，支持明确选择 TLS 1.2/1.3，并保留证书与服务器名称验证设置。

### Tests / 测试

- Add discovery, cancellation, task-event ordering, sparse-address, target-ID, pagination and duplicate-group regressions to the Modbus checks. / 新增扫描、中止、任务事件顺序、不连续地址、目标从站、分页和重复组回归测试，并接入发布前检查。
- Local verification passed 137 shared frontend tests, Go race tests, and real TCP discovery-to-polling browser flows. / 本地通过 137 项共享前端测试、Go 竞态测试，以及真实 TCP 从发现、扫描到轮询的浏览器流程。

### Validation limits / 验收边界

- Local cross-builds do not replace native Windows or physical serial-device acceptance. The release workflow builds and validates all four native targets before publishing. / 本地交叉构建不代替 Windows 原生界面和真实串口设备验收；发布流程在公开产物前构建并检查四个原生目标。

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

- Use Python 3.12.10 across native CI targets and a short Windows dependency workspace to avoid OpenSSL compiler path limits. / 原生 CI 统一使用 Python 3.12.10，Windows 依赖使用短路径目录，避免 OpenSSL 编译路径超限。

- Windows native menu integration and Modbus initial slave point address areas. / 修复 Windows 原生菜单集成和 Modbus 从站初始点位地址区配置。
- Appearance bindings remain available in both standalone and suite Wails builds; standalone methods reject suite configuration access. / Wails 独立版和套件构建均生成稳定的外观绑定，独立版拒绝访问套件配置。

### Validation limits / 验收边界

- macOS packages use ad-hoc signatures and are not notarized. Full GUI/protocol acceptance on every platform, macOS 13 hardware and offline first installation without WebView2 remain pending. / macOS 安装包使用 ad-hoc 签名，尚未公证；全平台完整窗口及协议验收、macOS 13 实机、无 WebView2 的断网首次安装仍待验证。
- Automated checks and the detailed historical evidence are described in `docs/implementation-status.md`. / 自动检查和历史验收证据见 `docs/implementation-status.md`。
