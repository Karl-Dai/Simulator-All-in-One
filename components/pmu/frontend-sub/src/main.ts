import { installConfigurationClose } from './simlab-ui/configClose'
import { installSuitePreferences } from './simlab-ui/configPreferences'

async function start() {
  if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
    const { listen } = await import('@tauri-apps/api/event')
    const { invoke } = await import('./simlab-ui/configInvoke')
    await installConfigurationClose(
      requested => listen('saio-before-close', requested),
      () => invoke('simlab_confirm_configuration_close'),
      () => invoke('simlab_enable_configuration_close'),
    )
  }

  if (import.meta.env.VITE_SIMLAB_SUITE === '1') await installSuitePreferences('pmu.sub')
  if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
    const { restoreDocuments } = await import('./suiteWorkspace')
    await restoreDocuments('pmu.sub')
  }
  await import('./bootstrap')
}
void start().catch(error => {
  console.error('Application initialization failed', error)
  const message = document.createElement('div')
  message.setAttribute('role', 'alert')
  message.textContent = String(error)
  const retry = document.createElement('button')
  retry.textContent = '重试 / Retry'
  retry.onclick = () => window.location.reload()
  message.append(retry)
  document.body.append(message)
})
