import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process is a nodejs global
const isCI = process.env.CI === "true";

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],
  build: {
    // Keep CI memory usage lower on macOS runners.
    // Monaco + workers can push V8 close to default heap limits during minification.
    minify: isCI ? false : "esbuild",
    cssMinify: isCI ? false : "esbuild",
    sourcemap: false,
    reportCompressedSize: false,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
