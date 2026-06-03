import { app, BrowserWindow } from 'electron';
import { createWindow } from './app/createWindow';
import { registerProjectsIpc } from './ipc/projectsIpc';
import { registerReviewIpc } from './ipc/reviewIpc';

app.whenReady().then(() => {
  const projectStore = registerProjectsIpc();
  registerReviewIpc(projectStore);
  createWindow();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
