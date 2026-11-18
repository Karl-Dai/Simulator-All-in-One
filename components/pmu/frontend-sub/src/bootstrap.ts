import './simlab-ui/tokens.css'
import './simlab-ui/components.css'
import { startSuiteTheme } from './simlab-ui/theme'
import { createApp } from "vue";
import App from "./App.vue";
import { useSubEvents } from "./composables/useSubEvents";

if (import.meta.env.VITE_SIMLAB_SUITE === '1') startSuiteTheme()
createApp(App).mount("#app");
useSubEvents().startListening();
