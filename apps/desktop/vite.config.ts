import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Config afinada para Tauri: porta fixa, sem limpar a tela (Tauri usa o stdout),
// e ignorando o diretório src-tauri no watcher.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    // Porta distinta da default (5173) pra não colidir com outros projetos
    // Tauri rodando em paralelo.
    port: 5273,
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
