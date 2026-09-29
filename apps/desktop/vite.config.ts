import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The UI lives in ui/; Tauri loads the dev server in development and the
// built files from dist/ in a bundle (tauri.conf.json).
export default defineConfig({
  root: "ui",
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: "127.0.0.1" },
  build: { outDir: "../dist", emptyOutDir: true, target: "safari16" },
});
