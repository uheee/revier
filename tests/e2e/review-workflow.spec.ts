import { expect, test } from '@playwright/test';

test('project entry UI is interactive', async ({ page }) => {
  await page.addInitScript(() => {
    const projects: Array<{
      id: string;
      name: string;
      repoPath: string;
      pinned: boolean;
      lastOpenedAt: string;
      preferences: { defaultBranch: string; defaultDays: number; defaultGlobRules: string[] };
    }> = [];

    window.revier = {
      projects: {
        list: async () => [...projects],
        add: async (repoPath, options) => {
          const project = {
            id: `project-${projects.length + 1}`,
            name: options?.name ?? 'Revier',
            repoPath,
            pinned: false,
            lastOpenedAt: '2026-06-03T00:00:00.000Z',
            preferences: {
              defaultBranch: 'develop',
              defaultDays: 30,
              defaultGlobRules: []
            }
          };
          projects.push(project);
          return project;
        },
        update: async (project) => project,
        remove: async (projectId) => {
          const index = projects.findIndex((project) => project.id === projectId);
          if (index >= 0) {
            projects.splice(index, 1);
          }
        },
        validateRepository: async (repoPath) => ({ valid: true, repoPath, currentBranch: 'develop' }),
        listBranches: async () => [{ name: 'develop', current: true }]
      },
      review: {
        startAnalysis: async (filters) => ({
          taskId: 'task-1',
          projectId: filters.projectId,
          status: 'completed',
          stage: 'ready',
          progress: 1
        }),
        cancelAnalysis: async () => undefined,
        getTask: async () => ({
          taskId: 'task-1',
          projectId: 'project-1',
          status: 'completed',
          stage: 'ready',
          progress: 1
        }),
        onTaskUpdate: () => () => undefined,
        listChangedFiles: async () => [],
        getFileOverlay: async () => {
          throw new Error('Overlay is not used by this E2E smoke test');
        }
      }
    };
  });

  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Revier' })).toBeVisible();
  await expect(page.getByPlaceholder('E:/Projects/revier')).toBeVisible();

  await page.getByPlaceholder('E:/Projects/revier').fill('E:/Projects/revier');
  await page.getByRole('textbox', { name: '项目名称' }).fill('Revier');
  await page.getByRole('button', { name: /添加项目/ }).click();

  await expect(page.getByText('E:/Projects/revier')).toBeVisible();
  await page.getByRole('button', { name: '打开' }).first().click();
  await expect(page.getByRole('heading', { name: '筛选' })).toBeVisible();
});
