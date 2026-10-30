# Modbus 子站初始点位

新建连接、同一连接下新建从站时，四类地址区默认均为空。按需要点击“添加范围”，每类可以配置多个不连续范围；创建后继续使用现有的编辑、删除、批量添加或导入功能。

| 地址区 | 共用点表的功能码 | 类型 |
|---|---|---|
| 线圈 | FC01、FC05、FC15 | Bool |
| 离散输入 | FC02 | Bool |
| 保持寄存器 | FC03、FC06、FC16 | UInt16、Int16、UInt32、Int32、Float32 |
| 输入寄存器 | FC04 | UInt16、Int16、UInt32、Int32、Float32 |

地址是从 0 开始的协议地址，起止地址均包含在范围内。UInt32、Int32、Float32 占两个连续寄存器。例如保持寄存器 `100～119 / Float32 / 小端序` 生成 10 个点，占用 20 个寄存器地址。表单分别显示点数和地址数。

同一区域不允许范围重叠；不同区域允许使用相同地址。地址越界、起止颠倒、非整数或不能容纳完整 32 位点的范围会阻止提交，并在对应行显示原因。全部留空可创建空点表。“全零/随机”只初始化选定点位。随机浮点数使用有限数值，不生成 NaN 或 Infinity。

创建失败不留下部分连接或部分子站，且表单内容保留。旧工程按保存的实际点位加载，不补点或删点。旧 IPC 调用省略 `RegisterRanges` 时保留原初始化语义；当前两个创建表单始终提交明确数组，空数组表示空点表。存储格式和协议处理逻辑无需迁移。

## 验证

本功能随 `patches/modbus.patch` 应用到锁定的 Modbus 源码。在准备好的 `.components/ModbusSim` 中运行：

```sh
npm ci --ignore-scripts
node --experimental-strip-types --test scripts/register-ranges.test.mjs
npm run build --workspace frontend
./go/scripts/slavegui.sh test -race ./...
```

CI 的 `modbus-points` 任务从锁定提交重新应用补丁，检查前端校验、构建与 Go appstate 回归；后者包括真实 TCP 请求及工程往返测试。

2026-10-30 在 macOS ARM 上通过浏览器连接真实 Wails 开发服务（真实 Go 绑定，无 mock），以实际表单验证：

- 线圈 0～15，保持寄存器 0～9/UInt16 和 100～119/Float32/小端，共 36 个点、46 个地址；输入寄存器和离散输入留空。
- 该 UI 创建的连接启动真实 TCP 监听后，独立 Python 对端验证 FC01/05/15、FC03/06/16 读写一致，地址空洞和空输入区返回地址异常，小端 Float32 123.5 写入后原样读回。
- 新增空从站为 0 点；新增随机从站包含 8 个离散输入和 2 个 UInt32 输入点；已有 36 点不变。新建空连接同样为 0 点。
- 范围重叠和不完整 Float32 范围阻止提交；中英文文案、900×600 窗口内滚动、后端点数及前端列表一致。

本机截图保存在执行 worktree 的 `output/playwright/modbus-ranges-{dialog,result,english}.png`。浏览器载入 Wails 开发调试脚本时出现一次第三方 `nodes` 空引用，绑定连接和上述全部操作断言均通过。这里的页面证据来自 Chromium 与 Wails 开发桥；不等于 Windows/Linux 原生窗口验收或新的套件安装包验收。
