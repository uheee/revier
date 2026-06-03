import { app, ipcMain } from 'electron';
import { join } from 'node:path';
import { ipcChannels } from '../../shared/ipcTypes';
import { GitService } from '../git/gitService';
import { JsonProjectStore } from '../projects/projectStore';

export function registerProjectsIpc(): JsonProjectStore {
  const store = new JsonProjectStore(join(app.getPath('userData'), 'projects.json'));
  const git = new GitService();

  ipcMain.handle(ipcChannels.projectsList, () => store.list());
  ipcMain.handle(ipcChannels.projectsAdd, (_event, repoPath: string, options) =>
    store.add(repoPath, options)
  );
  ipcMain.handle(ipcChannels.projectsUpdate, (_event, project) => store.update(project));
  ipcMain.handle(ipcChannels.projectsRemove, (_event, projectId: string) => store.remove(projectId));
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
