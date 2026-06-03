import { app, dialog, ipcMain } from 'electron';
import { basename, join } from 'node:path';
import { ipcChannels } from '../../shared/ipcTypes';
import { GitService } from '../git/gitService';
import { JsonProjectStore } from '../projects/projectStore';

export function registerProjectsIpc(): JsonProjectStore {
  const store = new JsonProjectStore(join(app.getPath('userData'), 'projects.json'));
  const git = new GitService();

  ipcMain.handle(ipcChannels.projectsList, () => store.list());
  ipcMain.handle(ipcChannels.projectsAdd, async (_event, repoPath: string, options) => {
    const validation = await git.validateRepository(repoPath);
    if (!validation.valid) {
      throw new Error(validation.error ?? '请选择一个 Git 仓库目录');
    }

    return store.add(validation.repoPath, {
      ...options,
      preferences: {
        ...options?.preferences,
        defaultBranch: options?.preferences?.defaultBranch ?? validation.currentBranch
      }
    });
  });
  ipcMain.handle(ipcChannels.projectsUpdate, (_event, project) => store.update(project));
  ipcMain.handle(ipcChannels.projectsRemove, (_event, projectId: string) => store.remove(projectId));
  ipcMain.handle(ipcChannels.projectsSelectDirectory, async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory'],
      title: '选择 Git 仓库目录'
    });
    const selectedPath = result.filePaths[0];
    if (result.canceled || !selectedPath) {
      return undefined;
    }

    return {
      path: selectedPath,
      name: basename(selectedPath)
    };
  });
  ipcMain.handle(ipcChannels.projectsValidateRepository, (_event, repoPath: string) =>
    git.validateRepository(repoPath)
  );
  ipcMain.handle(ipcChannels.projectsListBranches, async (_event, projectId: string) => {
    const project = (await store.list()).find((item) => item.id === projectId);
    if (!project) {
      return [];
    }
    return git.listBranches(project.repoPath);
  });

  return store;
}
