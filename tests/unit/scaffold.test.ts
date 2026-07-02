import packageJson from '../../package.json';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

describe('project scaffold', () => {
  const cargoWithDuckDbDownload = 'node scripts/cargo-duckdb-download.mjs';

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

  it('runs Rust cargo scripts through the DuckDB download wrapper', () => {
    expect(packageJson.scripts['rust:test']).toBe(`${cargoWithDuckDbDownload} test --workspace`);
    expect(packageJson.scripts['rust:spike']).toBe(
      `${cargoWithDuckDbDownload} run -p revier-analysis -- spike run`
    );
  });

  it('exposes Rust index debug scripts', () => {
    expect(packageJson.scripts['rust:index:status']).toBe(
      `${cargoWithDuckDbDownload} run -p revier-analysis -- index status`
    );
    expect(packageJson.scripts['rust:index:build']).toBe(
      `${cargoWithDuckDbDownload} run -p revier-analysis -- index build`
    );
    expect(packageJson.scripts['rust:index:query-files']).toBe(
      `${cargoWithDuckDbDownload} run -p revier-analysis -- index query-files`
    );
  });

  it('exposes Rust overlay debug scripts', () => {
    expect(packageJson.scripts['rust:file-overlay']).toBe(
      `${cargoWithDuckDbDownload} run -p revier-analysis -- file-overlay`
    );
    expect(packageJson.scripts['rust:trace-block']).toBe(
      `${cargoWithDuckDbDownload} run -p revier-analysis -- trace-block`
    );
  });
});
