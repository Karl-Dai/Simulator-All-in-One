# Role icons

The four protocol families use distinct colors and symbols. A round **M** badge
identifies a master (**C** for the OPC UA client); a square **S** badge identifies a slave, substation, or server.
The browser title spells out the complete role.

Each `icon.svg` is the editable source. PNG, ICO and ICNS files were generated
with the repository's Tauri 2 CLI, then limited to the formats used by desktop
bundles. To regenerate a role, run the installed Tauri CLI's `icon` command on
its SVG in a temporary output directory and copy `32x32.png`, `128x128.png`,
`128x128@2x.png`, `icon.png`, `icon.ico`, and `icon.icns` back here.

`scripts/sync_ui.py --write` copies the checked-in assets to all eight patched
frontends and native build directories. Tauri suite configurations reference
`suite-icons`; the Modbus suite build temporarily swaps its Wails build icon
and restores the standalone assets on exit. Modbus Linux also embeds the role
PNG because the window icon is configured at runtime.
