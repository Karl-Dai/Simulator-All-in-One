import { createApp } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { installConfigurationClose } from './simlab-ui/configClose'
import { installSuitePreferences } from './simlab-ui/configPreferences'
import { setPreferencesFlusher } from './suite-persist'
import { startSuiteTheme } from './simlab-ui/theme'
import App from './App.vue'
import './simlab-ui/tokens.css'
import './simlab-ui/components.css'
import './style.css'
import './suite-theme.css'

async function bootstrap(): Promise<void> {
  // 套件模式先把 localStorage 换成 SQLite 支撑的实现，再挂载应用。
  if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
    startSuiteTheme()
    const preferences = await installSuitePreferences('iec101.slave')
    setPreferencesFlusher(preferences.flush)
    await installConfigurationClose(
      (requested) => listen('saio-before-close', requested),
      () => invoke('simlab_confirm_configuration_close'),
      () => invoke('simlab_enable_configuration_close'),
    )
  }
  createApp(App).mount('#app')
}

void bootstrap().catch((error) => {
  // 初始化失败时把原因显示在页面上，便于现场排查。
  document.body.textContent = '初始化失败 / Init failed: ' + String(error)
})
