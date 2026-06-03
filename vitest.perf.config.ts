import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['tests/perf/**/*.test.ts'],
    testTimeout: 120_000,
    hookTimeout: 30_000
  }
});
