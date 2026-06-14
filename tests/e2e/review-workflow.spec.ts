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
    args: [join(process.cwd(), 'dist/main/index.js')],
    env
  });

  try {
    const page = await app.firstWindow();
    await expect(page.getByRole('heading', { name: 'Revier' })).toBeVisible();
    await expect(page.getByPlaceholder('选择或输入本地仓库路径')).toHaveCount(0);

    await page.locator('[data-test="open-project-dialog"]').first().click();
    const projectDialog = page.locator('[data-test="project-dialog"]');
    await expect(projectDialog).toBeVisible();

    const repoPathInput = projectDialog.getByPlaceholder('选择或输入本地仓库路径');
    await expect(repoPathInput).toBeVisible();
    await repoPathInput.evaluate((element, value) => {
      const input = element as HTMLInputElement;
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }, process.cwd());
    await projectDialog.getByRole('textbox', { name: '项目名称' }).fill('Revier');
    await projectDialog.locator('[data-test="project-editor-submit"]').click();

    await expect(page.getByRole('heading', { name: '筛选' })).toBeVisible();
    await expect(page.locator('[data-test="branch-select"]')).toBeVisible();
    await expect(page.locator('[data-test="date-range"]')).toBeVisible();

    await page.getByRole('button', { name: /分析/ }).click();
    await expect(page.locator('.diff-table--full')).toBeVisible({ timeout: 20000 });
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
