import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { BrowserWindow } from 'electron';

const currentDir = dirname(fileURLToPath(import.meta.url));

export function createWindow(): BrowserWindow {
  const window = new BrowserWindow({
    width: 1280,
    height: 820,
    minWidth: 1024,
    minHeight: 700,
    show: false,
    webPreferences: {
      preload: join(currentDir, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });

  window.once('ready-to-show', () => window.show());

  if (process.env.ELECTRON_RENDERER_URL) {
    void window.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    void window.loadFile(join(currentDir, '../renderer/index.html'));
  }

  return window;
}
