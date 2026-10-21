import { defineConfig } from "@playwright/test";
// Each test launches its own Chromium with the built extension and serves its own pages.
export default defineConfig({
  testDir: "./e2e",
  workers: 1,
});
