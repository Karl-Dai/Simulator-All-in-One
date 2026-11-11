import { EventsOn } from '../wailsjs/runtime/runtime'
import { installWailsConfigurationClose } from '../../ui/configClose'

async function start() {
  if (window.go?.main?.App) await installWailsConfigurationClose(EventsOn)
  await import('./bootstrap')
}
void start().catch(error => {
  console.error('Application initialization failed', error)
  const message = document.createElement('div')
  message.setAttribute('role', 'alert')
  message.textContent = String(error)
  document.body.append(message)
})
