import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

// The popup, the extension page and the service worker. The service worker is an ES module
// (see public/manifest.json), so it shares this build; the content script cannot be one and
// is built alone by vite.content.config.ts.
export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rolldownOptions: {
      input: {
        popup: here("popup.html"),
        pages: here("pages.html"),
        background: here("src/background.ts"),
      },
      output: { entryFileNames: "[name].js" },
    },
  },
  test: {
    exclude: ["e2e/**", "node_modules/**"],
    environment: "jsdom",
    // Vitest empties CSS imports by default; the highlights read their colours from the tokens.
    css: { include: /tokens\.css/ },
    restoreMocks: true,
  },
});
