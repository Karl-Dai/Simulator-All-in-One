import { installConfigurationClose } from 'simlab-ui/configClose'
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

  if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
    const { installSuitePreferences } = await import('simlab-ui/configPreferences')
    await installSuitePreferences('opcua.master')
  }
  await import('./bootstrap')
}
void bootstrap()
