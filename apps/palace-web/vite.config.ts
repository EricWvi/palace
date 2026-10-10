import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { preloadFirstPaintFont } from "./vite/preload-font.ts";

export default defineConfig({
  plugins: [react(), tailwindcss(), preloadFirstPaintFont()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            // Highlighting is most of the Markdown weight and changes only on upgrades; its own
            // chunk keeps one cached copy across every page that shows Markdown, whatever they ship.
            {
              name: "highlight",
              test: /node_modules[\\/](highlight\.js|lowlight|rehype-highlight)[\\/]/,
            },
          ],
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/auth": "http://127.0.0.1:8080",
    },
  },
  test: {
    exclude: ["e2e/**", "node_modules/**"],
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    restoreMocks: true,
  },
});
