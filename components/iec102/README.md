# IEC 60870-5-102 Simulator

IEC 60870-5-102 主站（计量中心）与子站仿真器，作为 Simulator All-in-One 套件的组件运行。

- `crates/iec102-core`：FT1.2 链路层、102 累计量应用层、主站与子站会话、TCP 与串口传输。
- `crates/iec102master-app`：主站 Tauri 应用。
- `crates/iec102sim-app`：子站 Tauri 应用。
- `frontend`：两端共用的前端。

## 开发

```bash
cargo test -p iec102-core
```
