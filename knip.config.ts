import type { KnipConfig } from 'knip';

const config: KnipConfig = {
  entry: [
    'src/renderer/main.ts',
    'src/renderer/App.vue',
    'vite.config.ts',
    'vitest.config.ts',
    'vitest.perf.config.ts',
    'eslint.config.js'
  ],
  project: [
    'src/renderer/**/*.{ts,vue}',
    'tests/**/*.{ts,vue}',
    'vite.config.ts',
    'vitest.config.ts',
    'vitest.perf.config.ts',
    'eslint.config.js'
  ],
  ignore: ['src/renderer/generated/bindings.ts'],
  ignoreDependencies: [
    // 阶段一只记录候选，不在未完成独立确认和回归前删除。
    '@vueuse/components',
    'diff',
    'minimatch',
    // 阶段四性质测试会使用，阶段一先随 Node 测试依赖加入。
    'fast-check'
  ]
};

export default config;
