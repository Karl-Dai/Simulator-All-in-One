import { startNativeWindowTheme } from './nativeWindowTheme'
import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

document.documentElement.dataset.simlab = "launcher";

createApp(App).mount("#app");

void startNativeWindowTheme().catch(() => console.warn('Native window theme unavailable'))
