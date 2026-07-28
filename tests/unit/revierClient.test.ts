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

const editorColors = {
  workspaceBackground: '#F4F6F8',
  panelBackground: '#FFFFFF',
  editorBackground: '#FCFDFE',
  border: '#DFE5EC',
  foreground: '#273448',
  muted: '#768296',
  accent: '#0F766E',
  selection: '#DCEFEB',
  diffRemoved: '#FBE7E5',
  diffRemovedStrong: '#BC3D35',
  diffRemovedWord: '#F1B9B3',
  diffAdded: '#E2F3E8',
  diffAddedStrong: '#26804A',
  diffAddedWord: '#A9DBBB',
  syntax: {
    comment: '#768296',
    keyword: '#893CAD',
    string: '#0B7952',
    number: '#A05B00',
    type: '#0969DA',
    function: '#1C63A5',
    variable: '#273448'
  }
};

const editorSettingsSnapshot = {
  configPath: 'C:/config/editor.toml',
  settings: {
    version: 1,
    theme: 'system',
    defaultEncoding: 'auto',
    editor: {
      fontFamilies: ['monospace'],
      fontSize: 13,
      lineHeight: 22,
      minimap: true
    },
    largeFile: {
      maxBytes: 1_048_576,
      maxLines: 5_000
    },
    themes: {
      light: editorColors,
      dark: editorColors
    }
  }
};

describe('revierClient', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it('invokes editor settings command without a payload', async () => {
    vi.mocked(invoke).mockResolvedValue(editorSettingsSnapshot);

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

    await revierClient.review.startAnalysis(filters, 'operation-1');
    await revierClient.review.listChangedFiles(task.taskId);
    await revierClient.review.getFileOverlay({
      taskId: task.taskId,
      filePath: file.path,
      operationId: 'file-operation-1',
      cacheMode: 'prefer-cache'
    });
    await revierClient.review.getCommitOverlay({
      taskId: task.taskId,
      filePath: file.path,
      commitHash: 'abc123',
      operationId: 'commit-operation-1',
      cacheMode: 'prefer-cache'
    });

    expect(invoke).toHaveBeenNthCalledWith(1, 'review_start_analysis', {
      filters,
      operationId: 'operation-1'
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'review_list_changed_files', {
      taskId: task.taskId
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'review_get_file_overlay', {
      request: {
        taskId: task.taskId,
        filePath: file.path,
        operationId: 'file-operation-1',
        cacheMode: 'prefer-cache'
      }
    });
    expect(invoke).toHaveBeenNthCalledWith(4, 'review_get_commit_overlay', {
      request: {
        taskId: task.taskId,
        filePath: file.path,
        commitHash: 'abc123',
        operationId: 'commit-operation-1',
        cacheMode: 'prefer-cache'
      }
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

  it('调用分支缓存恢复与选中文件持久化命令', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await revierClient.review.restoreBranchAnalysis('project-1', 'develop');
    await revierClient.review.getBranchCacheStatus('project-1', 'develop');
    await revierClient.review.setBranchSelectedFile('project-1', 'develop', 'src/main.ts');

    expect(invoke).toHaveBeenNthCalledWith(1, 'review_restore_branch_analysis', {
      projectId: 'project-1',
      branch: 'develop'
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'review_get_branch_cache_status', {
      projectId: 'project-1',
      branch: 'develop'
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'review_set_branch_selected_file', {
      projectId: 'project-1',
      branch: 'develop',
      filePath: 'src/main.ts'
    });
  });

  it('订阅统一操作进度事件', async () => {
    const unlisten = vi.fn();
    const progress = {
      operationId: 'operation-1',
      kind: 'project-analysis' as const,
      status: 'running' as const,
      projectId: 'project-1',
      branch: 'develop',
      stage: 'index-commits' as const,
      message: '正在更新索引',
      startedAt: '2026-07-22T00:00:00Z',
      elapsedMs: 100,
      cacheState: 'refresh' as const
    };
    vi.mocked(listen).mockImplementation(async (_eventName, handler) => {
      handler({ payload: progress } as Parameters<typeof handler>[0]);
      return unlisten;
    });
    const callback = vi.fn();

    const unsubscribe = await revierClient.review.onOperationProgress(callback);
    unsubscribe();

    expect(listen).toHaveBeenCalledWith('review://operation-progress', expect.any(Function));
    expect(callback).toHaveBeenCalledWith(progress);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
