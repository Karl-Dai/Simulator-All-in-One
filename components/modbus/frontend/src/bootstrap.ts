import { startNativeWindowTheme } from 'shared-frontend/src/composables/nativeWindowTheme'
import './simlab-ui/tokens.css'
import './simlab-ui/components.css'
import { startSuiteTheme } from './simlab-ui/theme'
import 'shared-frontend/src/styles/tokens.css'
import { createApp } from 'vue'
import './style.css'
import 'shared-frontend/src/styles/transitions.css'
import 'shared-frontend/src/styles/toolbar.css'
import { desktopServicesKey, initTheme } from 'shared-frontend'
import App from './App.vue'

import { GetVersion } from '../wailsjs/go/main/App'
import { BrowserOpenURL } from '../wailsjs/runtime/runtime'

initTheme()
if (import.meta.env.VITE_SIMLAB_SUITE === '1') startSuiteTheme()
createApp(App).provide(desktopServicesKey, { getVersion: GetVersion, openURL: BrowserOpenURL }).mount('#app')

void startNativeWindowTheme().catch(() => console.warn('Native window theme unavailable'))
