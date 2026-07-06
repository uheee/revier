import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

it('生成的核心类型不包含 recoverable 字段', () => {
  const bindings = readFileSync(
    resolve(process.cwd(), 'src/renderer/generated/bindings.ts'),
    'utf8'
  );
  expect(bindings).not.toContain('recoverable');
});
