import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

describe('set-package-version', () => {
  it('同步更新 package 与 Tauri 配置版本号', () => {
    const workspace = mkdtempSync(join(tmpdir(), 'revier-version-'));
    const scriptPath = resolve(process.cwd(), 'scripts/set-package-version.mjs');

    try {
      mkdirSync(join(workspace, 'src-tauri'), { recursive: true });
      writeFileSync(
        join(workspace, 'package.json'),
        `${JSON.stringify({ name: 'revier', version: '0.1.0' }, null, 2)}\n`
      );
      writeFileSync(
        join(workspace, 'src-tauri/tauri.conf.json'),
        `${JSON.stringify({ productName: 'Revier', version: '0.1.0' }, null, 2)}\n`
      );

      execFileSync(process.execPath, [scriptPath, 'v1.2.3'], { cwd: workspace });

      const packageJson = JSON.parse(readFileSync(join(workspace, 'package.json'), 'utf8'));
      const tauriConfig = JSON.parse(readFileSync(join(workspace, 'src-tauri/tauri.conf.json'), 'utf8'));

      expect(packageJson.version).toBe('1.2.3');
      expect(tauriConfig.version).toBe('1.2.3');
    } finally {
      rmSync(workspace, { recursive: true, force: true });
    }
  });
});
