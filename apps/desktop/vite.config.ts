import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Config afinada para Tauri: porta fixa, sem limpar a tela (Tauri usa o stdout),
// e ignorando o diretório src-tauri no watcher.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  // Tauri usa Chromium (Win)/WebKit (mac/linux) modernos — target enxuto.
  build: {
    target: "esnext",
    sourcemap: false,
  },
});
