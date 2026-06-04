import packageJson from '../../package.json';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

describe('project scaffold', () => {
  it('uses the expected package name and Electron entry', () => {
    expect(packageJson.name).toBe('revier');
    expect(packageJson.main).toBe('dist/main/index.js');
  });

  it('uses Naive UI as the renderer component framework', () => {
    const dependencies = packageJson.dependencies;
    const rendererEntry = readFileSync(resolve(import.meta.dirname, '../../src/renderer/main.ts'), 'utf8');

    expect(dependencies).toHaveProperty('naive-ui');
    expect(dependencies).not.toHaveProperty('element-plus');
    expect(rendererEntry).toContain("from 'naive-ui'");
    expect(rendererEntry).not.toContain('element-plus');
  });
});
