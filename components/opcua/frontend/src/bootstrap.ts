import 'simlab-ui/tokens.css'
import 'simlab-ui/components.css'
import { startSuiteTheme } from 'simlab-ui/theme'
import { createApp } from 'vue'
import '@shared/styles/tokens.css'
import '@shared/styles/transitions.css'
import '@shared/styles/toolbar.css'
import App from './App.vue'

if (import.meta.env.VITE_SIMLAB_SUITE === '1') startSuiteTheme()
createApp(App).mount('#app')

if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
  void import('./components/PersistenceStatus.vue').then(({ default: PersistenceStatus }) => {
    const statusRoot = document.createElement('div')
    document.body.append(statusRoot)
    createApp(PersistenceStatus).mount(statusRoot)
  })
}
