import { AnalysisTaskManager } from '../../src/main/analysis/analysisTaskManager';

describe('AnalysisTaskManager', () => {
  it('creates tasks and cancels the previous active project task', () => {
    const manager = new AnalysisTaskManager();

    const first = manager.createTask('project-1');
    const second = manager.createTask('project-1');

    expect(manager.getTask(first.taskId)?.status).toBe('cancelled');
    expect(manager.getTask(second.taskId)?.status).toBe('pending');
  });

  it('emits task snapshots when status changes', () => {
    const manager = new AnalysisTaskManager();
    const task = manager.createTask('project-1');
    const updates: string[] = [];

    const unsubscribe = manager.onUpdate((snapshot) => updates.push(snapshot.status));
    manager.markRunning(task.taskId, 'resolveRange', 'Resolving range');
    manager.markCompleted(task.taskId);
    unsubscribe();

    expect(updates).toEqual(['running', 'completed']);
  });
});
