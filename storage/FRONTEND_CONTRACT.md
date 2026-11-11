# Suite browser preference bridge

Canonical `ui/configPreferences.ts` exports `installSuitePreferences(role): Promise<{flush(): Promise<void>}>`. Call only for suite builds, before dynamically importing any App/i18n/theme module that accesses localStorage at module initialization. It awaits restoration and blocks mounting on failure, with visible retry. Standalone keeps original browser persistence.

Native methods:
- Go `App.LoadPreferences(legacy map[string]any) (map[string]any,error)` and `App.SavePreferences(values map[string]any) error`.
- Tauri `simlab_load_preferences(legacy: serde_json::Value) -> Result<Value,String>` and `simlab_save_preferences(values: Value) -> Result<(),String>`.

Both read/write role document `ui` in the same session Store used by other role documents. First load imports the supplied legacy map with absent-only migration, source `webview-localStorage-v1`. Existing SQLite document wins. Each save uses the revision captured by successful load/save, never reloads before writing. Restore errors forbid defaults writes. Objects must be stored as typed trees.

Wire dictionary values are tagged `{kind:'text',value:string}` or `{kind:'json',value:<typed JSON value>}`. The helper parses valid raw localStorage strings beginning with `{` or `[` into `json` values. Other strings are `text`. Native returns the same wire dictionary. JSON.stringify only reconstructs the browser's in-memory string interface; it is never persisted as a serialized object/array leaf. Original browser storage remains unchanged.

Writes are debounced and serialized, followed with the latest draft when another change occurs during a write. Failures retain dirty drafts, show an accessible error and manual retry. clear/remove save a valid smaller/empty map. The returned flush rejects on save error. Page lifecycle handlers request a flush; beforeunload blocks while dirty, because an asynchronous last-moment write alone is not durable.

## Native close handshake

`ui/configClose.ts` owns the per-window flusher registry: `registerConfigurationFlusher(flush)` returns an unregister callback; concurrent `flushAllConfiguration()` calls share one serialized flush. Preferences register only after valid restore; PMU registers document flush only after all documents restore. Failed/unmounted read-only stores are not registered.

Every suite entry point subscribes to `saio-before-close`, then awaits native enable, before App imports/mount. Wails binds `App.EnableConfigurationClose` / `App.ConfirmConfigurationClose`; Tauri binds `simlab_enable_configuration_close` / `simlab_confirm_configuration_close`. Guards start disarmed so an unmounted application remains closable. Once armed, native close requests are prevented and coalesced until an acknowledgement for an active request. The frontend freezes editing (`body.inert`), blurs the edited control and drains queued watchers, flushes all registered stores and in-flight native configuration requests, then confirms close. A write/confirm error unfreezes the UI and displays a retry action that repeats the whole close flow. Success has no confirmation prompt. Lifecycle/beforeunload handlers are supplemental to this native handshake.

Tauri app/shared imports use canonical `ui/configInvoke.ts`, which re-exports the SDK core and delegates invoke with the original argument count/options while registering configuration promises. Native SDK internals are never patched (the SDK invoke property is read-only). Runtime reads/connections, event-plugin calls and close enable/confirm commands do not delay the configuration barrier. Wails binds are writable; their configuration methods plus launcher Launch (which commits Recent after startup) are tracked before mounting. Explicit registry/SDK tests cover slow writes, a second flusher, failed flush and retry, repeated close requests, blur watchers, pending replace_connection and launcher Launch.
