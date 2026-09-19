import { defineConfig } from "@playwright/test";
const port = Number(process.env.PALACE_BROWSER_TEST_PORT ?? 5173);
const baseURL = `http://127.0.0.1:${port}`;
export default defineConfig({
  testDir: "./e2e",
  use: { baseURL, browserName: "chromium" },
  webServer: {
    command: `npm run dev -- --port ${port} --strictPort`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
