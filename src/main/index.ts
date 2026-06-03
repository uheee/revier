import { app, BrowserWindow } from 'electron';
import { createWindow } from './app/createWindow';
import { registerProjectsIpc } from './ipc/projectsIpc';
import { registerReviewIpc } from './ipc/reviewIpc';

if (process.env.REVIER_USER_DATA_DIR) {
  app.setPath('userData', process.env.REVIER_USER_DATA_DIR);
}

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
