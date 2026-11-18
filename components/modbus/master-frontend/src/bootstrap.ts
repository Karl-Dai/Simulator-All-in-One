import { startNativeWindowTheme } from 'shared-frontend/src/composables/nativeWindowTheme'
import './simlab-ui/tokens.css'
import './simlab-ui/components.css'
import { createSuiteAppearance, suiteAppearanceKey } from './suiteAppearance'
import { ReadAppearance } from '../wailsjs/go/suitehost/Appearance'
import { parseAppearance } from './simlab-ui/contracts/appearance'
import { GetVersion } from '../wailsjs/go/main/App'
import { BrowserOpenURL } from '../wailsjs/runtime/runtime'
import 'shared-frontend/src/styles/tokens.css'
import 'shared-frontend/src/styles/toolbar.css'
import { desktopServicesKey, initTheme } from 'shared-frontend'
import { createApp } from 'vue'
import App from './App.vue'

initTheme()
const appearance = createSuiteAppearance(import.meta.env.VITE_SIMLAB_SUITE === '1', {
  readAppearance: async () => parseAppearance(await ReadAppearance()),
})
createApp(App)
  .provide(desktopServicesKey, { getVersion: GetVersion, openURL: BrowserOpenURL })
  .provide(suiteAppearanceKey, appearance)
  .mount('#app')
if (import.meta.hot) import.meta.hot.dispose(() => appearance?.dispose())

void startNativeWindowTheme().catch(() => console.warn('Native window theme unavailable'))
