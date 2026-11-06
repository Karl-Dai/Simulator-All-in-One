# Role icons

The eight role icons use the soft enamel design: a continuous colored tile,
soft upper-left lighting, a fine inset rim, and a shallow shadow behind the
protocol symbol. Filled geometric shapes replace the previous line drawings.
Protocol labels are integrated into the tile without a separate badge.

Masters and clients use a blue gradient (`#387DA6`, `#225C87`, `#174365`).
Slaves, substations, and servers use a warm copper gradient (`#BC743E`,
`#9A4C22`, `#703617`). Both roles in a family share the same symbol and label:
**104**, **MODBUS**, **PMU**, or **UA**. The browser title names the complete role.

Every SVG uses a 128-unit view box and the same 120-unit tile with 25-unit
corners. Symbols fit inside `(20, 20, 88, 66)`, including their shallow shadow.
Labels have a shared 16-unit cap height, right edge at 108, and bottom at 110.
The DIN Condensed Bold lettering is stored as paths, so installed fonts do not
change its proportions. Gradients, masks, and lighting are vector-only.

Each `icon.svg` is the editable source. PNG, ICO and ICNS files are generated
with the repository's Tauri 2 CLI, then limited to desktop bundle formats.
Run the installed Tauri CLI's `icon` command on an SVG in a temporary output
directory and copy `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`,
`icon.ico`, and `icon.icns` back here. Inspect 32, 64, and 128 pixel previews.

`scripts/sync_ui.py --write` copies the checked-in assets to all eight patched
frontends and native build directories. Tauri suite configurations reference
`suite-icons`; the Modbus suite build temporarily swaps its Wails build icon
and restores standalone assets on exit. Modbus Linux also embeds the role PNG
because the window icon is configured at runtime.
