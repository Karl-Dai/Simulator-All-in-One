import { installConfigurationClose } from 'simlab-ui/configClose'
import 'simlab-ui/tokens.css'
import 'simlab-ui/components.css'
import { invoke } from 'simlab-ui/configInvoke'
import { parseAppearance } from 'simlab-ui/contracts/appearance'
import { createApp } from 'vue'
import '@shared/styles/tokens.css'
import '@shared/styles/base.css'
import '@shared/styles/transitions.css'
import '@shared/styles/toolbar.css'
import { installSuitePreferences } from 'simlab-ui/configPreferences'

async function bootstrap() {
  if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
    const { listen } = await import('@tauri-apps/api/event')
    const { invoke } = await import('simlab-ui/configInvoke')
    await installConfigurationClose(
      requested => listen('saio-before-close', requested),
      () => invoke('simlab_confirm_configuration_close'),
      () => invoke('simlab_enable_configuration_close'),
    )
  }

  if (import.meta.env.VITE_SIMLAB_SUITE === '1') await installSuitePreferences('iec104.master')
  const { createSuiteAppearance, suiteAppearanceKey } = await import('./suiteAppearance')
  await import('@shared/composables/useTheme')
  const { default: App } = await import('./App.vue')
  const appearance = createSuiteAppearance(import.meta.env.VITE_SIMLAB_SUITE === '1', {
    readAppearance: async () => parseAppearance(await invoke('simlab_appearance')),
  })
  createApp(App).provide(suiteAppearanceKey, appearance).mount('#app')
  if (import.meta.hot) import.meta.hot.dispose(() => appearance?.dispose())
}
void bootstrap()
