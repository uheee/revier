import { randomUUID } from 'node:crypto';
import type { AnalysisStage, AnalysisTaskSnapshot } from '../../shared/reviewTypes';

type Listener = (snapshot: AnalysisTaskSnapshot) => void;

export class AnalysisTaskManager {
  private readonly tasks = new Map<string, AnalysisTaskSnapshot>();
  private readonly activeByProject = new Map<string, string>();
  private readonly listeners = new Set<Listener>();

  createTask(projectId: string): AnalysisTaskSnapshot {
    const activeTaskId = this.activeByProject.get(projectId);
    if (activeTaskId) {
      this.markCancelled(activeTaskId);
    }

    const snapshot: AnalysisTaskSnapshot = {
      taskId: randomUUID(),
      projectId,
      status: 'pending',
      stage: 'readRepository',
      progress: 0
    };
    this.tasks.set(snapshot.taskId, snapshot);
    this.activeByProject.set(projectId, snapshot.taskId);
    return snapshot;
  }

  getTask(taskId: string): AnalysisTaskSnapshot | undefined {
    return this.tasks.get(taskId);
  }

  onUpdate(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  markRunning(taskId: string, stage: AnalysisStage, message: string): void {
    this.update(taskId, { status: 'running', stage, message });
  }

  markCompleted(taskId: string): void {
    this.update(taskId, { status: 'completed', stage: 'ready', progress: 1 });
  }

  markCancelled(taskId: string): void {
    this.update(taskId, { status: 'cancelled' });
  }

  private update(taskId: string, patch: Partial<AnalysisTaskSnapshot>): void {
    const current = this.tasks.get(taskId);
    if (!current) {
      return;
    }

    const next = { ...current, ...patch };
    this.tasks.set(taskId, next);
    for (const listener of this.listeners) {
      listener(next);
    }
  }
}
