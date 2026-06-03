import { createGlobMatcher } from '../../src/main/analysis/globRules';

describe('globRules', () => {
  it('supports include and exclude rules', () => {
    const matcher = createGlobMatcher(['src/**/*.ts', '!**/*.test.ts']);

    expect(matcher('src/main/index.ts')).toBe(true);
    expect(matcher('src/main/index.test.ts')).toBe(false);
    expect(matcher('README.md')).toBe(false);
  });

  it('matches every normalized path when no rules are provided', () => {
    const matcher = createGlobMatcher([]);

    expect(matcher('src/App.vue')).toBe(true);
  });
});
