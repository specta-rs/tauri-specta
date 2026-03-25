import { defineConfig } from "vite";
import rescript from "@jihchi/vite-plugin-rescript";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [rescript({ loader: { output: "./lib/bs", suffix: ".js" } })],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome100", "safari13"],
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
