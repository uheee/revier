import { contextBridge } from 'electron';

contextBridge.exposeInMainWorld('revier', {
  version: '0.1.0'
});
