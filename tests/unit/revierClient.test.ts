import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { revierClient } from '../../src/renderer/api/revierClient';
import type {
  AnalysisTaskSnapshot,
  ChangedFile,
  FileOverlay,
  ReviewFilters,
  ReviewProject
} from '../../src/renderer/generated/bindings';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn()
}));

const project: ReviewProject = {
  id: 'project-1',
  name: 'Revier',
  repoPath: 'E:/Projects/revier',
  pinned: false,
  preferences: {
    defaultBranch: 'develop',
    defaultDays: 30,
    defaultGlobRules: []
  }
};

const filters: ReviewFilters = {
  projectId: 'project-1',
  branch: 'develop',
  globRules: ['src/**/*.ts']
};

const task: AnalysisTaskSnapshot = {
  taskId: 'task-1',
  projectId: 'project-1',
  status: 'completed',
  stage: 'ready',
  progress: 1
};

const file: ChangedFile = {
  path: 'src/main.ts',
  status: 'modified',
  additions: 1,
  deletions: 1,
  isBinary: false,
  isPreviewable: true
};

const overlay: FileOverlay = {
  mode: 'range',
  file,
  range: {
    branch: 'develop',
    baseCommit: 'base',
    headCommit: 'head'
  },
  rows: [],
  blocks: [],
  warnings: [],
  oldContent: '',
  newContent: '',
  resolvedEncoding: 'utf-8'
};

describe('revierClient', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it('invokes editor settings command without a payload', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await revierClient.settings.getEditorSettings();

    expect(invoke).toHaveBeenCalledWith('editor_settings_get');
  });

  it('invokes project commands with Tauri payload names', async () => {
    vi.mocked(invoke).mockResolvedValue(project);

    await revierClient.projects.add(project.repoPath, { name: project.name });
    await revierClient.projects.update(project);
    await revierClient.projects.remove(project.id);

    expect(invoke).toHaveBeenNthCalledWith(1, 'projects_add', {
      repoPath: project.repoPath,
      options: { name: project.name }
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'projects_update', { project });
    expect(invoke).toHaveBeenNthCalledWith(3, 'projects_remove', { projectId: project.id });
  });

  it('normalizes empty directory selection to undefined', async () => {
    vi.mocked(invoke).mockResolvedValue(null);

    await expect(revierClient.projects.selectDirectory()).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('projects_select_directory');
  });

  it('invokes review commands with Tauri payload names', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(task)
      .mockResolvedValueOnce([file])
      .mockResolvedValueOnce(overlay)
      .mockResolvedValueOnce(overlay);

    await revierClient.review.startAnalysis(filters);
    await revierClient.review.listChangedFiles(task.taskId);
    await revierClient.review.getFileOverlay({
      taskId: task.taskId,
      filePath: file.path
    });
    await revierClient.review.getCommitOverlay({
      taskId: task.taskId,
      filePath: file.path,
      commitHash: 'abc123'
    });

    expect(invoke).toHaveBeenNthCalledWith(1, 'review_start_analysis', { filters });
    expect(invoke).toHaveBeenNthCalledWith(2, 'review_list_changed_files', {
      taskId: task.taskId
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'review_get_file_overlay', {
      request: { taskId: task.taskId, filePath: file.path }
    });
    expect(invoke).toHaveBeenNthCalledWith(4, 'review_get_commit_overlay', {
      request: { taskId: task.taskId, filePath: file.path, commitHash: 'abc123' }
    });
  });

  it('subscribes to review task update events', async () => {
    const unlisten = vi.fn();
    vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
      handler({ payload: task } as Parameters<typeof handler>[0]);
      return unlisten;
    });
    const callback = vi.fn();

    const unsubscribe = await revierClient.review.onTaskUpdate(callback);
    unsubscribe();

    expect(listen).toHaveBeenCalledWith('review://task-updated', expect.any(Function));
    expect(callback).toHaveBeenCalledWith(task);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
