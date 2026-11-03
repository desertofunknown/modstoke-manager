import Icons from 'unplugin-icons/vite';
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// This project's folder with forward slashes, e.g. C:/.../RedManager/.
const projectRoot = decodeURIComponent(new URL(".", import.meta.url).pathname).replace(/^\/([A-Za-z]:)/, "$1");

// Cargo's target folder (tens of thousands of files) and local tool folders are not frontend
// sources; watching them kept Vite busy for seconds at a time. Only this project's own are skipped,
// so a checkout that sits inside one of them still watches its sources.
function outsideFrontend(file: string) {
  const path = file.replaceAll("\\", "/");
  if (!path.toLowerCase().startsWith(projectRoot.toLowerCase())) return false;
  return /^(src-tauri|\.[^/]+)(\/|$)/.test(path.slice(projectRoot.length));
}

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [svelte(), Icons({
    compiler: 'svelte',
  })],

  // No PostCSS plugins are used. Without an inline config Vite looks for one when it serves the first
  // stylesheet, which held up the first dev load for about 30 seconds.
  css: { postcss: {} },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: outsideFrontend,
    },
    // Compile the startup modules while cargo builds, so the window doesn't wait on Vite.
    warmup: {
      clientFiles: ["./src/main.ts", "./src/App.svelte", "./src/pages/MainPage.svelte"],
    },
  },
  // 3. to make use of `TAURI_DEBUG` and other env variables
  // https://tauri.studio/v1/api/config#buildconfig.beforedevcommand
  envPrefix: ["VITE_", "TAURI_"],
}));
