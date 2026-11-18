# IEC 60870-5-101 Simulator

IEC 60870-5-101 主站与子站仿真器，作为 Simulator All-in-One 套件的组件运行。

- `crates/iec101-core`：FT1.2 链路层、101 应用层、主站与子站会话、TCP 与串口传输。
- `crates/iec101master-app`：主站 Tauri 应用。
- `crates/iec101sim-app`：子站 Tauri 应用。
- `frontend`：两端共用的前端。

## 开发

```bash
cargo test -p iec101-core
```
