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

  it('exposes Rust index debug scripts', () => {
    expect(packageJson.scripts['rust:index:status']).toBe(
      'cargo run -p revier-analysis -- index status'
    );
    expect(packageJson.scripts['rust:index:build']).toBe(
      'cargo run -p revier-analysis -- index build'
    );
    expect(packageJson.scripts['rust:index:query-files']).toBe(
      'cargo run -p revier-analysis -- index query-files'
    );
  });
});
