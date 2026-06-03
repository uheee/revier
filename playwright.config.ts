import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 30_000,
  webServer: {
    command: 'pnpm exec vite --config vite.renderer.config.ts --host 127.0.0.1 --port 5173',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000
  },
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry'
  }
});
