/// <reference types="vite/client" />

declare module '*?worker' {
  const WorkerConstructor: {
    new (): Worker;
  };
  export default WorkerConstructor;
}

declare var MonacoEnvironment: {
  getWorker(moduleId: string, label: string): Worker;
};
