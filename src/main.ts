import { createApp } from "vue";
import App from "./App.vue";

import "./assets/styles/index.scss";
import "@xterm/xterm/css/xterm.css";

import { FontAwesomeIcon } from "./icons/fontawesome";

createApp(App)
  .component("FontAwesomeIcon", FontAwesomeIcon)
  .mount("#app");

// Remove startup overlay as soon as Vue has mounted.
const bootSplash = document.getElementById("boot-splash");
if (bootSplash) {
  bootSplash.remove();
}
