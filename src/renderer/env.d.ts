import type { RevierApi } from '../shared/ipcTypes';

declare global {
  interface Window {
    revier: RevierApi;
  }
}

export {};
