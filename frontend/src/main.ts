import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

document.documentElement.dataset.simlab = "launcher";

createApp(App).mount("#app");
