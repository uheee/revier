import packageJson from '../../package.json';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

describe('project scaffold', () => {
  it('uses the expected package name without an Electron entry', () => {
    expect(packageJson.name).toBe('revier');
    expect(packageJson).not.toHaveProperty('main');
  });

  it('uses Naive UI as the renderer component framework', () => {
    const dependencies = packageJson.dependencies;
    const rendererEntry = readFileSync(resolve(import.meta.dirname, '../../src/renderer/main.ts'), 'utf8');

    expect(dependencies).toHaveProperty('naive-ui');
    expect(dependencies).not.toHaveProperty('element-plus');
    expect(rendererEntry).toContain("from 'naive-ui'");
    expect(rendererEntry).not.toContain('element-plus');
  });

  it('provides platform build scripts for Rust resources', () => {
    expect(existsSync(resolve(import.meta.dirname, '../../scripts/build.ps1'))).toBe(true);
    expect(existsSync(resolve(import.meta.dirname, '../../scripts/build.sh'))).toBe(true);
    expect(existsSync(resolve(import.meta.dirname, '../../scripts/cargo-duckdb-download.mjs'))).toBe(false);
  });

  it('exposes Tauri scripts without legacy Rust debug shortcuts', () => {
    const scripts: Record<string, string> = packageJson.scripts;

    expect(scripts.dev).toBe('pnpm tauri dev');
    expect(scripts.build).toBe('pnpm typecheck && pnpm generate:bindings:check && pnpm tauri build');
    expect(scripts.tauri).toBe('tauri');
    expect(scripts).not.toHaveProperty('rust:test:win');
    expect(scripts).not.toHaveProperty('rust:test:unix');
    expect(packageJson.scripts).not.toHaveProperty('rust:test');
    expect(packageJson.scripts).not.toHaveProperty('rust:spike');
    expect(packageJson.scripts).not.toHaveProperty('rust:index:status');
    expect(packageJson.scripts).not.toHaveProperty('rust:index:build');
    expect(packageJson.scripts).not.toHaveProperty('rust:index:query-files');
    expect(packageJson.scripts).not.toHaveProperty('rust:file-overlay');
    expect(packageJson.scripts).not.toHaveProperty('rust:trace-block');
  });
});
