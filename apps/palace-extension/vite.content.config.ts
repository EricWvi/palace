import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

// Content scripts run as classic scripts, so this one is a single self-contained IIFE written
// next to the rest of the build.
export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: false,
    lib: {
      entry: fileURLToPath(new URL("src/content.ts", import.meta.url)),
      formats: ["iife"],
      name: "palaceContent",
      fileName: () => "content.js",
    },
  },
});
