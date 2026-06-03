import { contextBridge, ipcRenderer } from 'electron';
import { ipcChannels, type RevierApi } from '../shared/ipcTypes';
import type { AnalysisTaskSnapshot } from '../shared/reviewTypes';

const api: RevierApi = {
  projects: {
    list: () => ipcRenderer.invoke(ipcChannels.projectsList),
    add: (repoPath, options) => ipcRenderer.invoke(ipcChannels.projectsAdd, repoPath, options),
    update: (project) => ipcRenderer.invoke(ipcChannels.projectsUpdate, project),
    remove: (projectId) => ipcRenderer.invoke(ipcChannels.projectsRemove, projectId),
    validateRepository: (repoPath) =>
      ipcRenderer.invoke(ipcChannels.projectsValidateRepository, repoPath),
    listBranches: (projectId) => ipcRenderer.invoke(ipcChannels.projectsListBranches, projectId)
  },
  review: {
    startAnalysis: (filters) => ipcRenderer.invoke(ipcChannels.reviewStartAnalysis, filters),
    cancelAnalysis: (taskId) => ipcRenderer.invoke(ipcChannels.reviewCancelAnalysis, taskId),
    getTask: (taskId) => ipcRenderer.invoke(ipcChannels.reviewGetTask, taskId),
    onTaskUpdate: (callback) => {
      const listener = (_event: Electron.IpcRendererEvent, task: unknown) => {
        callback(task as AnalysisTaskSnapshot);
      };
      ipcRenderer.on(ipcChannels.reviewTaskUpdated, listener);
      return () => ipcRenderer.removeListener(ipcChannels.reviewTaskUpdated, listener);
    },
    listChangedFiles: (taskId) => ipcRenderer.invoke(ipcChannels.reviewListChangedFiles, taskId),
    getFileOverlay: (request) => ipcRenderer.invoke(ipcChannels.reviewGetFileOverlay, request)
  }
};

contextBridge.exposeInMainWorld('revier', api);
