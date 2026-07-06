import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

describe('Tauri capabilities', () => {
  it('主窗口具备事件订阅权限', () => {
    const capability = JSON.parse(
      readFileSync(resolve(process.cwd(), 'src-tauri/capabilities/default.json'), 'utf8')
    );

    expect(capability.windows).toContain('main');
    expect(capability.permissions).toContain('core:event:default');
  });
});
