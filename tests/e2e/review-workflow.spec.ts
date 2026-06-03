import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { _electron as electron, expect, test } from '@playwright/test';

test('project entry UI is interactive', async () => {
  const userDataDir = await mkdtemp(join(tmpdir(), 'revier-e2e-'));
  const env: Record<string, string> = {
    ...stringEnv(),
    ELECTRON_DISABLE_SECURITY_WARNINGS: 'true',
    REVIER_USER_DATA_DIR: userDataDir
  };
  delete env.ELECTRON_RUN_AS_NODE;

  const app = await electron.launch({
    args: [join(process.cwd(), 'out/main/index.js')],
    env
  });

  try {
    const page = await app.firstWindow();
    await expect(page.getByRole('heading', { name: 'Revier' })).toBeVisible();
    await expect(page.getByPlaceholder('E:/Projects/revier')).toBeVisible();

    await page.getByPlaceholder('E:/Projects/revier').fill(process.cwd());
    await page.getByRole('textbox', { name: '项目名称' }).fill('Revier');
    await page.getByRole('button', { name: /添加项目/ }).click();

    await expect(page.getByText(process.cwd())).toBeVisible();
    await page.getByRole('button', { name: '打开' }).first().click();
    await expect(page.getByRole('heading', { name: '筛选' })).toBeVisible();
  } finally {
    await app.close();
    await rm(userDataDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  }
});

function stringEnv(): Record<string, string> {
  return Object.fromEntries(
    Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === 'string')
  );
}
