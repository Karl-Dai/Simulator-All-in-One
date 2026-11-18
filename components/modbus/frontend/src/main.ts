import { installWailsConfigurationClose } from 'simlab-ui/configClose'
import { EventsOn } from '../wailsjs/runtime/runtime'
import { installSuitePreferences } from 'simlab-ui/configPreferences'

async function start() {
  if (import.meta.env.VITE_SIMLAB_SUITE === '1') await installWailsConfigurationClose(EventsOn)
  if (import.meta.env.VITE_SIMLAB_SUITE === '1') await installSuitePreferences('modbus.slave')
  await import('./bootstrap')
}
void start().catch(error => {
  console.error('Application initialization failed', error)
  const message = document.createElement('div')
  message.setAttribute('role', 'alert')
  message.textContent = String(error)
  document.body.append(message)
})
