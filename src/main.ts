import { createApp } from "vue";
import App from "./App.vue";

import "./assets/styles/index.scss";
import "@xterm/xterm/css/xterm.css";

import { FontAwesomeIcon } from "./icons/fontawesome";

// Keep native context menu in dev mode for easier debugging.
if (!import.meta.env.DEV) {
  window.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });
}

createApp(App)
  .component("FontAwesomeIcon", FontAwesomeIcon)
  .mount("#app");

// Remove startup overlay as soon as Vue has mounted.
const bootSplash = document.getElementById("boot-splash");
if (bootSplash) {
  bootSplash.remove();
}
