import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri expects a fixed dev port (matches tauri.conf.json devUrl).
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
  },
});
