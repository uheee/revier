import { createPinia, setActivePinia } from 'pinia';
import {
  MAX_PENDING_ANALYSIS_TASKS,
  useReviewStore
} from '../../src/renderer/stores/reviewStore';
import { revierClient } from '../../src/renderer/api/revierClient';
import { useNotifications } from '../../src/renderer/composables/useNotifications';
import type {
  AnalysisTaskSnapshot,
  ChangedFile,
  DiffBlock,
  FileOverlay,
  ReviewAuthorOptionsRequest,
  ReviewFilters
} from '../../src/renderer/generated/bindings';

vi.mock('../../src/renderer/api/revierClient', () => ({
  revierClient: {
    projects: {
      list: vi.fn(),
      add: vi.fn(),
      update: vi.fn(),
      remove: vi.fn(),
      validateRepository: vi.fn(),
      listBranches: vi.fn(),
      selectDirectory: vi.fn()
    },
    review: {
      restoreBranchAnalysis: vi.fn(),
      getBranchCacheStatus: vi.fn(),
      setBranchSelectedFile: vi.fn(),
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(),
      onOperationProgress: vi.fn(),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn(),
      attributeBlocks: vi.fn(),
      getCommitOverlay: vi.fn(),
      listAuthors: vi.fn()
    }
  }
}));

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

const runningTask: AnalysisTaskSnapshot = {
  taskId: 'task-1',
  projectId: 'project-1',
  status: 'running',
  stage: 'readRepository',
  message: '读取项目仓库'
};

const file: ChangedFile = {
  path: 'src/main/index.ts',
  status: 'modified',
  additions: 3,
  deletions: 1,
  isBinary: false,
  isPreviewable: true
};

const block: DiffBlock = {
  id: 'block-1',
  oldStart: 1,
  oldEnd: 2,
  newStart: 1,
  newEnd: 3,
  changeType: 'modified',
  authors: [
    {
      name: 'Snowind',
      email: 'jinks.tao@gmail.com',
      commitCount: 1,
      lastCommittedAt: '2026-05-10T00:00:00.000Z'
    }
  ],
  rows: [],
  relatedCommits: []
};

const overlay: FileOverlay = {
  mode: 'range',
  file,
  range: {
    branch: 'develop',
    baseCommit: 'base',
    headCommit: 'head'
  },
  blocks: [block],
  rows: [],
  warnings: [],
  oldContent: 'old',
  newContent: 'new',
  resolvedEncoding: 'utf-8'
};

describe('renderer reviewStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(revierClient.review.startAnalysis).mockReset();
    vi.mocked(revierClient.review.restoreBranchAnalysis).mockReset();
    vi.mocked(revierClient.review.listChangedFiles).mockReset();
    vi.mocked(revierClient.review.cancelAnalysis).mockReset();
    vi.mocked(revierClient.review.getFileOverlay).mockReset();
    vi.mocked(revierClient.review.attributeBlocks).mockReset();
    vi.mocked(revierClient.review.getCommitOverlay).mockReset();
    vi.mocked(revierClient.review.listAuthors).mockReset();
    useNotifications().clear();
  });

  it('直接恢复分支缓存而不启动分析', async () => {
    vi.mocked(revierClient.review.restoreBranchAnalysis).mockResolvedValue({
      projectId: 'project-1',
      branch: 'develop',
      cacheState: 'hit',
      cacheHit: true,
      stale: false,
      task,
      filters,
      files: [file],
      lastSelectedPath: file.path,
      currentHead: 'head',
      cachedHead: 'head',
      cacheReadElapsedMs: 12,
      analysisElapsedMs: 1000
    });

    const store = useReviewStore();
    const restored = await store.restoreBranch('project-1', 'develop');

    expect(restored?.cacheHit).toBe(true);
    expect(store.task).toEqual(task);
    expect(store.files).toEqual([file]);
    expect(store.branchCacheState).toBe('hit');
    expect(revierClient.review.startAnalysis).not.toHaveBeenCalled();
    expect(store.operation).toEqual(expect.objectContaining({
      kind: 'project-metadata',
      status: 'completed',
      message: '已从缓存加载',
      elapsedMs: 12
    }));
  });

  it('旧操作的终态不能覆盖当前前台操作', () => {
    const store = useReviewStore();
    store.operation = {
      operationId: 'operation-new',
      kind: 'project-analysis',
      status: 'running',
      projectId: 'project-1',
      branch: 'develop',
      stage: 'index-commits',
      message: '正在更新新操作',
      startedAt: '2026-07-22T00:00:01Z',
      elapsedMs: 10,
      cacheState: 'refresh'
    };

    store.handleOperationProgress({
      operationId: 'operation-old',
      kind: 'project-analysis',
      status: 'completed',
      projectId: 'project-1',
      branch: 'main',
      stage: 'ready',
      message: '旧操作完成',
      startedAt: '2026-07-22T00:00:00Z',
      elapsedMs: 1000,
      cacheState: 'refresh'
    });

    expect(store.operation.operationId).toBe('operation-new');
    expect(store.operation.status).toBe('running');
  });

  it('starts analysis and loads changed files after completion event', async () => {
    vi.mocked(revierClient.review.startAnalysis).mockResolvedValue(runningTask);
    vi.mocked(revierClient.review.listChangedFiles).mockResolvedValue([file]);

    const store = useReviewStore();
    await store.start(filters);

    expect(revierClient.review.startAnalysis).toHaveBeenCalledWith(filters, expect.any(String));
    expect(revierClient.review.listChangedFiles).not.toHaveBeenCalled();
    expect(store.task).toEqual(runningTask);
    expect(store.files).toEqual([]);
    expect(store.loading).toBe(true);

    await store.handleTaskUpdate(task);

    expect(revierClient.review.listChangedFiles).toHaveBeenCalledWith(task.taskId);
    expect(store.task).toEqual(task);
    expect(store.files).toEqual([file]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeUndefined();
  });

  it('分析启动失败时保留局部错误并记录一次带筛选上下文的通知', async () => {
    vi.mocked(revierClient.review.startAnalysis).mockRejectedValue(new Error('无法启动分析'));

    const store = useReviewStore();
    await store.start(filters);

    expect(store.error).toBe('无法启动分析');
    expect(store.loading).toBe(false);
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '分析启动失败',
        message: 'project-1 / develop：无法启动分析',
        source: 'Review'
      })
    ]);
  });

  it('已被新分析替代的启动失败不得产生陈旧通知', async () => {
    let rejectOld!: (reason: unknown) => void;
    vi.mocked(revierClient.review.startAnalysis)
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockResolvedValueOnce(runningTask);

    const store = useReviewStore();
    const oldRequest = store.start(filters);
    await store.start({ ...filters, branch: 'main' });
    rejectOld(new Error('旧分析失败'));
    await oldRequest;

    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('完成后文件列表加载失败时记录通知，过期结果不通知', async () => {
    vi.mocked(revierClient.review.listChangedFiles).mockRejectedValue(new Error('文件列表不可用'));
    const store = useReviewStore();
    store.task = runningTask;

    await store.handleTaskUpdate(task);

    expect(store.error).toBe('文件列表不可用');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '变更文件加载失败',
        message: 'task-1：文件列表不可用',
        source: 'Review'
      })
    ]);
  });

  it('分析取消后到达的文件列表失败不得产生陈旧通知', async () => {
    let rejectFiles!: (reason: unknown) => void;
    vi.mocked(revierClient.review.listChangedFiles).mockReturnValue(
      new Promise((_, reject) => { rejectFiles = reject; })
    );
    vi.mocked(revierClient.review.cancelAnalysis).mockResolvedValue(undefined);
    const store = useReviewStore();
    store.task = runningTask;

    const completion = store.handleTaskUpdate(task);
    await store.cancelAnalysis();
    rejectFiles(new Error('取消后的文件列表失败'));
    await completion;

    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('后端任务失败只记录一次通知并保留任务错误', async () => {
    const failedTask: AnalysisTaskSnapshot = {
      ...runningTask,
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '仓库分析失败' }
    };
    const store = useReviewStore();
    store.task = runningTask;

    await store.handleTaskUpdate(failedTask);
    await store.handleTaskUpdate(failedTask);

    expect(store.error).toBe('仓库分析失败');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '分析任务失败',
        message: 'task-1：仓库分析失败',
        source: 'Review'
      })
    ]);
  });

  it('新分析启动期间重放旧任务失败事件不得改变状态或重复通知', async () => {
    const failedTask: AnalysisTaskSnapshot = {
      ...runningTask,
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '旧任务失败' }
    };
    const nextTask: AnalysisTaskSnapshot = {
      ...runningTask,
      taskId: 'task-2'
    };
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    const store = useReviewStore();
    store.task = runningTask;
    await store.handleTaskUpdate(failedTask);
    expect(useNotifications().notifications.value).toHaveLength(1);

    const start = store.start({ ...filters, branch: 'main' });
    expect(store.loading).toBe(true);
    await store.handleTaskUpdate(failedTask);

    expect(useNotifications().notifications.value).toHaveLength(1);
    expect(store.loading).toBe(true);
    expect(store.error).toBeUndefined();
    expect(store.task).toStrictEqual(failedTask);

    resolveStart(nextTask);
    await start;
    expect(store.task).toStrictEqual(nextTask);
    expect(store.loading).toBe(true);
  });

  it('后发启动使前一代未知 taskId 的早到失败事件失效', async () => {
    let resolveFirst!: (value: AnalysisTaskSnapshot) => void;
    let resolveSecond!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis)
      .mockReturnValueOnce(new Promise((resolve) => { resolveFirst = resolve; }))
      .mockReturnValueOnce(new Promise((resolve) => { resolveSecond = resolve; }));
    const store = useReviewStore();

    const first = store.start(filters);
    const second = store.start({ ...filters, branch: 'main' });
    await store.handleTaskUpdate({
      ...runningTask,
      taskId: 'task-from-first',
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '前一代早到失败' }
    });

    expect(store.task).toBeUndefined();
    expect(store.error).toBeUndefined();
    expect(store.loading).toBe(true);
    expect(useNotifications().notifications.value).toEqual([]);

    const secondTask = { ...runningTask, taskId: 'task-from-second' };
    resolveSecond(secondTask);
    await second;
    resolveFirst({ ...runningTask, taskId: 'task-from-first' });
    await first;
    expect(store.task).toStrictEqual(secondTask);
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('待确认窗口内同一 taskId 的高频事件只保留最新快照', async () => {
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    const store = useReviewStore();
    const start = store.start(filters);

    for (let index = 0; index < 100; index += 1) {
      await store.handleTaskUpdate({
        ...runningTask,
        taskId: 'high-frequency-task',
        message: `进度 ${index}`
      });
    }

    expect(store.pendingAnalysis?.snapshots).toHaveLength(1);
    expect(store.pendingAnalysis?.snapshots[0]?.message).toBe('进度 99');
    resolveStart({ ...runningTask, taskId: 'confirmed-other-task' });
    await start;
  });

  it('待确认窗口内不同 taskId 的候选快照限制为固定容量并淘汰最旧项', async () => {
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    const store = useReviewStore();
    const start = store.start(filters);

    for (let index = 0; index < MAX_PENDING_ANALYSIS_TASKS + 5; index += 1) {
      await store.handleTaskUpdate({ ...runningTask, taskId: `candidate-${index}` });
    }

    expect(store.pendingAnalysis?.snapshots).toHaveLength(MAX_PENDING_ANALYSIS_TASKS);
    expect(store.pendingAnalysis?.snapshots[0]?.taskId).toBe('candidate-5');
    expect(store.pendingAnalysis?.snapshots.at(-1)?.taskId).toBe(
      `candidate-${MAX_PENDING_ANALYSIS_TASKS + 4}`
    );
    resolveStart({ ...runningTask, taskId: 'confirmed-other-task' });
    await start;
  });

  it('首次启动等待期间取消会使缓存代次和后续早到终态失效', async () => {
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    vi.mocked(revierClient.review.listChangedFiles).mockResolvedValue([]);
    const store = useReviewStore();

    const start = store.start(filters);
    await store.cancelAnalysis();
    await store.handleTaskUpdate({
      ...runningTask,
      taskId: 'cancelled-generation-task',
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '取消后早到失败' }
    });
    await store.handleTaskUpdate({ ...task, taskId: 'cancelled-generation-task' });

    expect(store.task).toBeUndefined();
    expect(store.error).toBeUndefined();
    expect(store.loading).toBe(false);
    expect(useNotifications().notifications.value).toEqual([]);

    resolveStart({ ...runningTask, taskId: 'cancelled-generation-task' });
    await start;
    expect(store.task).toBeUndefined();
  });

  it('旧终态之后的新任务早到运行事件在启动响应确认 taskId 后回放', async () => {
    const oldFailedTask: AnalysisTaskSnapshot = {
      ...runningTask,
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '旧任务失败' }
    };
    const responseTask: AnalysisTaskSnapshot = {
      ...runningTask,
      taskId: 'task-2',
      status: 'pending',
      stage: 'ready',
      message: '等待运行'
    };
    const earlyRunningTask: AnalysisTaskSnapshot = {
      ...runningTask,
      taskId: 'task-2',
      message: '已经运行'
    };
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    const store = useReviewStore();
    store.task = oldFailedTask;

    const start = store.start({ ...filters, branch: 'main' });
    await store.handleTaskUpdate(earlyRunningTask);
    expect(store.task).toStrictEqual(oldFailedTask);
    expect(store.loading).toBe(true);

    resolveStart(responseTask);
    await start;
    expect(store.task).toStrictEqual(earlyRunningTask);
    expect(store.loading).toBe(true);
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('completed 响应与缓存 completed 合并后只加载一次文件列表', async () => {
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    vi.mocked(revierClient.review.listChangedFiles)
      .mockResolvedValueOnce([file])
      .mockRejectedValueOnce(new Error('重复加载'));
    const store = useReviewStore();

    const start = store.start(filters);
    await store.handleTaskUpdate({ ...task, message: '最终完成事件' });
    resolveStart({ ...task, message: '响应中的完成状态' });
    await start;

    expect(revierClient.review.listChangedFiles).toHaveBeenCalledTimes(1);
    expect(store.files).toStrictEqual([file]);
    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('completed 响应处理后订阅重复同终态不会再次加载文件列表', async () => {
    vi.mocked(revierClient.review.startAnalysis).mockResolvedValue({
      ...task,
      message: '响应完成'
    });
    vi.mocked(revierClient.review.listChangedFiles)
      .mockResolvedValueOnce([file])
      .mockRejectedValueOnce(new Error('不应重复加载'));
    const store = useReviewStore();

    await store.start(filters);
    const settledTask = store.task;
    const settledFiles = store.files;
    const settledError = store.error;
    const settledLoading = store.loading;
    const settledNotifications = [...useNotifications().notifications.value];

    await store.handleTaskUpdate({ ...task, message: '订阅重复完成' });

    expect(revierClient.review.listChangedFiles).toHaveBeenCalledTimes(1);
    expect(store.task).toStrictEqual(settledTask);
    expect(store.files).toStrictEqual(settledFiles);
    expect(store.error).toBe(settledError);
    expect(store.loading).toBe(settledLoading);
    expect(useNotifications().notifications.value).toStrictEqual(settledNotifications);
  });

  it('failed 响应与更新后的缓存 failed 只按最终快照通知一次', async () => {
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => { resolveStart = resolve; })
    );
    const store = useReviewStore();
    const responseFailure: AnalysisTaskSnapshot = {
      ...runningTask,
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '响应中的过时失败' }
    };
    const finalFailure: AnalysisTaskSnapshot = {
      ...responseFailure,
      error: { code: 'ANALYSIS_FAILED', message: '缓存中的最终失败' }
    };

    const start = store.start(filters);
    await store.handleTaskUpdate(finalFailure);
    resolveStart(responseFailure);
    await start;

    expect(store.error).toBe('缓存中的最终失败');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '分析任务失败',
        message: 'task-1：缓存中的最终失败'
      })
    ]);
  });

  it('用户已取消后迟到的失败事件不视为新的运行时错误', async () => {
    const failedTask: AnalysisTaskSnapshot = {
      ...runningTask,
      status: 'failed',
      stage: 'ready',
      error: { code: 'ANALYSIS_FAILED', message: '迟到失败' }
    };
    const store = useReviewStore();
    store.task = { ...runningTask, status: 'cancelled', stage: 'ready' };

    await store.handleTaskUpdate(failedTask);

    expect(store.task?.status).toBe('cancelled');
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('keeps completed task when completion event arrives before start returns', async () => {
    let resolveStart!: (value: AnalysisTaskSnapshot) => void;
    vi.mocked(revierClient.review.startAnalysis).mockReturnValue(
      new Promise((resolve) => {
        resolveStart = resolve;
      })
    );
    vi.mocked(revierClient.review.listChangedFiles).mockResolvedValue([file]);

    const store = useReviewStore();
    const start = store.start(filters);
    await store.handleTaskUpdate(task);
    resolveStart(runningTask);
    await start;

    expect(store.task).toEqual(task);
    expect(store.files).toEqual([file]);
    expect(store.loading).toBe(false);
  });

  it('accepts completed update after optimistic cancel when backend already finished', async () => {
    vi.mocked(revierClient.review.cancelAnalysis).mockResolvedValue();
    vi.mocked(revierClient.review.listChangedFiles).mockResolvedValue([file]);

    const store = useReviewStore();
    store.task = runningTask;
    store.loading = true;
    await store.cancelAnalysis();

    expect(store.task?.status).toBe('cancelled');

    await store.handleTaskUpdate(task);

    expect(store.task).toEqual(task);
    expect(store.files).toEqual([file]);
    expect(store.loading).toBe(false);
  });

  it('取消 IPC 失败时保留可理解状态并通知，成功取消不通知', async () => {
    vi.mocked(revierClient.review.cancelAnalysis)
      .mockRejectedValueOnce(new Error('无法联系后端'))
      .mockResolvedValueOnce(undefined);
    const store = useReviewStore();
    store.task = runningTask;
    store.loading = true;

    await store.cancelAnalysis();

    expect(store.task?.status).toBe('cancelled');
    expect(store.error).toBe('无法联系后端');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '取消分析失败',
        message: 'task-1：无法联系后端',
        source: 'Review'
      })
    ]);

    useNotifications().clear();
    store.task = runningTask;
    await store.cancelAnalysis();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('被新分析替代的取消请求失败不得产生陈旧通知', async () => {
    let rejectCancel!: (reason: unknown) => void;
    vi.mocked(revierClient.review.cancelAnalysis).mockReturnValue(
      new Promise((_, reject) => { rejectCancel = reject; })
    );
    vi.mocked(revierClient.review.startAnalysis).mockResolvedValue(runningTask);
    const store = useReviewStore();
    store.task = runningTask;

    const cancellation = store.cancelAnalysis();
    await store.start({ ...filters, branch: 'main' });
    rejectCancel(new Error('旧取消失败'));
    await cancellation;

    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('loads overlay and selects a diff block', async () => {
    vi.mocked(revierClient.review.getFileOverlay).mockResolvedValue(overlay);

    const store = useReviewStore();
    store.task = task;
    expect(await store.loadOverlay(file.path, 'gb18030')).toBe(true);
    store.selectBlock(block);

    expect(revierClient.review.getFileOverlay).toHaveBeenCalledWith(expect.objectContaining({
      taskId: task.taskId,
      filePath: file.path,
      cacheMode: 'prefer-cache',
      encoding: 'gb18030'
    }));
    expect(store.overlay).toEqual(overlay);
    expect(store.selectedBlock).toEqual(block);
    expect(store.operation).toEqual(expect.objectContaining({
      kind: 'monaco-diff',
      status: 'running',
      stage: 'compute-diff'
    }));
  });

  it('切换文件时立即清除旧 overlay，且旧请求不得覆盖新请求', async () => {
    let resolveOld!: (value: FileOverlay) => void;
    const newOverlay = { ...overlay, file: { ...file, path: 'src/new.ts' } };
    vi.mocked(revierClient.review.getFileOverlay)
      .mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve; }))
      .mockResolvedValueOnce(newOverlay);

    const store = useReviewStore();
    store.task = task;
    store.overlay = overlay;
    const oldRequest = store.loadOverlay(file.path, 'utf-8');
    expect(store.overlay).toBeUndefined();

    expect(await store.loadOverlay('src/new.ts', 'utf-16le')).toBe(true);
    resolveOld(overlay);
    expect(await oldRequest).toBe(false);
    expect(store.overlay).toEqual(newOverlay);
  });

  it('旧文件请求后到达的失败不得污染错误状态或通知中心', async () => {
    let rejectOld!: (reason: unknown) => void;
    const newOverlay = { ...overlay, file: { ...file, path: 'src/new.ts' } };
    vi.mocked(revierClient.review.getFileOverlay)
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockResolvedValueOnce(newOverlay);

    const store = useReviewStore();
    store.task = task;
    const oldRequest = store.loadOverlay(file.path, 'utf-8');
    expect(await store.loadOverlay('src/new.ts', 'utf-8')).toBe(true);
    rejectOld(new Error('旧文件请求失败'));

    expect(await oldRequest).toBe(false);
    expect(store.overlay).toStrictEqual(newOverlay);
    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('编码重载在成功前保留旧 overlay，成功后原子替换', async () => {
    let resolveReload!: (value: FileOverlay) => void;
    const reloaded = { ...overlay, resolvedEncoding: 'gb18030' as const, newContent: '新内容' };
    vi.mocked(revierClient.review.getFileOverlay).mockReturnValue(
      new Promise((resolve) => { resolveReload = resolve; })
    );

    const store = useReviewStore();
    store.task = task;
    store.overlay = overlay;
    const request = store.reloadOverlayEncoding(file.path, 'gb18030');
    expect(store.overlay).toStrictEqual(overlay);
    expect(store.operation).toEqual(expect.objectContaining({
      kind: 'encoding-reload',
      status: 'running'
    }));
    resolveReload(reloaded);

    expect(await request).toBe(true);
    expect(store.overlay).toEqual(reloaded);
    expect(revierClient.review.getFileOverlay).toHaveBeenCalledWith(expect.objectContaining({
      taskId: task.taskId,
      filePath: file.path,
      cacheMode: 'prefer-cache',
      encoding: 'gb18030'
    }));
  });

  it('刷新读取失败时保留旧文件、作者轨和提交下钻', async () => {
    vi.mocked(revierClient.review.getFileOverlay).mockRejectedValue(new Error('刷新读取失败'));
    const drilldown = { ...overlay, mode: 'commit' as const, parentHash: 'parent' };
    const store = useReviewStore();
    store.task = task;
    store.overlay = overlay;
    store.selectedBlock = block;
    store.drilldownOverlay = drilldown;
    store.selectedCommitHash = 'abc123';

    expect(await store.loadOverlay(file.path, 'utf-8', 'refresh')).toBe(false);

    expect(revierClient.review.getFileOverlay).toHaveBeenCalledWith(expect.objectContaining({
      filePath: file.path,
      cacheMode: 'refresh'
    }));
    expect(store.overlay).toStrictEqual(overlay);
    expect(store.selectedBlock).toStrictEqual(block);
    expect(store.drilldownOverlay).toStrictEqual(drilldown);
    expect(store.selectedCommitHash).toBe('abc123');
  });

  it('刷新归因失败时回滚旧状态，成功时再关闭旧下钻', async () => {
    const refreshed = { ...overlay, newContent: '刷新后的内容' };
    const drilldown = { ...overlay, mode: 'commit' as const, parentHash: 'parent' };
    vi.mocked(revierClient.review.getFileOverlay).mockResolvedValue(refreshed);
    vi.mocked(revierClient.review.attributeBlocks).mockRejectedValueOnce(new Error('刷新归因失败'));
    const store = useReviewStore();
    store.task = task;
    store.overlay = overlay;
    store.selectedBlock = block;
    store.drilldownOverlay = drilldown;
    store.selectedCommitHash = 'abc123';
    const canonicalBlock = { ...block, order: 0 };

    expect(await store.loadOverlay(file.path, 'utf-8', 'refresh')).toBe(true);
    store.acceptDiffBlocks({ contextKey: 'refresh-1', generation: 1, signature: 'signature-1', blocks: [canonicalBlock] }, 'refresh-1');
    await Promise.resolve();

    expect(store.overlay).toStrictEqual(overlay);
    expect(store.drilldownOverlay).toStrictEqual(drilldown);
    expect(store.selectedCommitHash).toBe('abc123');

    vi.mocked(revierClient.review.attributeBlocks).mockResolvedValue({
      resolvedEncoding: 'utf-8',
      attributions: [{
        id: block.id,
        authors: block.authors,
        relatedCommits: block.relatedCommits
      }],
      warnings: [],
      cacheState: 'refresh'
    });
    expect(await store.loadOverlay(file.path, 'utf-8', 'refresh')).toBe(true);
    store.acceptDiffBlocks({ contextKey: 'refresh-2', generation: 2, signature: 'signature-2', blocks: [canonicalBlock] }, 'refresh-2');
    expect(store.operation).toEqual(expect.objectContaining({
      kind: 'file-attribution',
      status: 'running'
    }));
    await vi.waitFor(() => {
      expect(store.diffComputationState).toBe('ready');
    });

    expect(revierClient.review.attributeBlocks).toHaveBeenLastCalledWith(expect.objectContaining({
      cacheMode: 'refresh',
      blockSignature: 'signature-2'
    }));
    expect(store.overlay?.newContent).toBe('刷新后的内容');
    expect(store.drilldownOverlay).toBeUndefined();
    expect(store.selectedCommitHash).toBeUndefined();
    expect(store.lastTerminalOperation).toEqual(expect.objectContaining({
      kind: 'file-attribution',
      status: 'completed'
    }));
  });

  it('编码重载失败时保留旧 overlay、选中块并记录错误', async () => {
    vi.mocked(revierClient.review.getFileOverlay).mockRejectedValue(new Error('解码失败'));
    const store = useReviewStore();
    store.task = task;
    store.overlay = overlay;
    store.selectedBlock = block;

    expect(await store.reloadOverlayEncoding(file.path, 'utf-16be')).toBe(false);
    expect(store.overlay).toStrictEqual(overlay);
    expect(store.selectedBlock).toStrictEqual(block);
    expect(store.error).toBe('解码失败');
    expect(store.operation).toEqual(expect.objectContaining({
      status: 'failed'
    }));
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '文件编码重载失败',
        message: `${file.path}（utf-16be）：解码失败`,
        source: 'Review'
      })
    ]);
  });

  it('stores structured Tauri errors as readable messages', async () => {
    vi.mocked(revierClient.review.getFileOverlay).mockRejectedValue({
      code: 'TASK_FILE_NOT_FOUND',
      message: '任务文件缓存中不存在请求的文件',
      detail: file.path
    });

    const store = useReviewStore();
    store.task = task;
    await store.loadOverlay(file.path);

    expect(store.error).toBe('任务文件缓存中不存在请求的文件');
    expect(store.loading).toBe(false);
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '文件差异加载失败',
        message: `${file.path}：任务文件缓存中不存在请求的文件`,
        source: 'Review'
      })
    ]);
  });

  it('loads author filter options', async () => {
    const request: ReviewAuthorOptionsRequest = {
      projectId: 'project-1',
      branch: 'develop'
    };
    const authors = [
      { key: 'alice@example.com', name: 'Alice', email: 'alice@example.com', commitCount: 2 }
    ];
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue(authors);

    const store = useReviewStore();
    await store.loadAuthors(request);

    expect(revierClient.review.listAuthors).toHaveBeenCalledWith(request);
    expect(store.authors).toEqual(authors);
    expect(store.authorsLoading).toBe(false);
  });

  it('作者筛选加载失败只影响当前请求并记录上下文通知', async () => {
    const request: ReviewAuthorOptionsRequest = {
      projectId: 'project-1',
      branch: 'develop'
    };
    vi.mocked(revierClient.review.listAuthors).mockRejectedValue(new Error('作者列表失败'));

    const store = useReviewStore();
    await store.loadAuthors(request);

    expect(store.error).toBe('作者列表失败');
    expect(store.authorsLoading).toBe(false);
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '作者筛选加载失败',
        message: 'project-1 / develop：作者列表失败',
        source: 'Review'
      })
    ]);
  });

  it('旧作者筛选请求失败不得覆盖新结果或产生陈旧通知', async () => {
    let rejectOld!: (reason: unknown) => void;
    const authors = [
      { key: 'alice@example.com', name: 'Alice', email: 'alice@example.com', commitCount: 2 }
    ];
    vi.mocked(revierClient.review.listAuthors)
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockResolvedValueOnce(authors);
    const store = useReviewStore();

    const oldRequest = store.loadAuthors({ projectId: 'project-1', branch: 'develop' });
    await store.loadAuthors({ projectId: 'project-1', branch: 'main' });
    rejectOld(new Error('旧作者请求失败'));
    await oldRequest;

    expect(store.authors).toStrictEqual(authors);
    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('loads and clears commit drilldown overlay', async () => {
    const commitOverlay: FileOverlay = {
      ...overlay,
      mode: 'commit',
      commit: {
        hash: 'abc123',
        shortHash: 'abc123',
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        committedAt: '2026-05-10T00:00:00.000Z',
        subject: 'feature: update app',
        matchedByFilter: true,
        touchedRanges: []
      },
      parentHash: 'parent'
    };
    vi.mocked(revierClient.review.getCommitOverlay).mockResolvedValue(commitOverlay);

    const store = useReviewStore();
    store.task = task;
    expect(await store.loadCommitOverlay(file.path, 'abc123', 'utf-8')).toBe(true);

    expect(revierClient.review.getCommitOverlay).toHaveBeenCalledWith(expect.objectContaining({
      taskId: task.taskId,
      filePath: file.path,
      commitHash: 'abc123',
      cacheMode: 'prefer-cache',
      encoding: 'utf-8'
    }));
    const request = vi.mocked(revierClient.review.getCommitOverlay).mock.calls[0][0];
    store.handleOperationProgress({
      operationId: request.operationId,
      kind: 'commit-overlay',
      status: 'completed',
      projectId: task.projectId,
      filePath: file.path,
      commitHash: 'abc123',
      stage: 'ready',
      message: '已从缓存加载提交 abc123',
      startedAt: new Date().toISOString(),
      elapsedMs: 12,
      cacheState: 'hit'
    });
    expect(store.operation).toEqual(expect.objectContaining({
      kind: 'commit-overlay',
      status: 'completed',
      cacheState: 'hit'
    }));
    expect(store.drilldownOverlay).toEqual(commitOverlay);
    expect(store.selectedCommitHash).toBe('abc123');

    store.closeCommitDrilldown();
    expect(store.drilldownOverlay).toBeUndefined();
    expect(store.selectedCommitHash).toBeUndefined();

    expect(await store.loadCommitOverlay(file.path, 'abc123', 'utf-8')).toBe(true);
    expect(revierClient.review.getCommitOverlay).toHaveBeenCalledTimes(2);
    expect(store.drilldownOverlay).toEqual(commitOverlay);
    expect(store.operation).toEqual(expect.objectContaining({
      kind: 'commit-overlay',
      status: 'completed'
    }));
  });

  it('提交初载失败时保留当前选择和专用错误状态，关闭后统一清理', async () => {
    vi.mocked(revierClient.review.getCommitOverlay).mockRejectedValue(new Error('提交加载失败'));
    const store = useReviewStore();
    store.task = task;

    expect(await store.loadCommitOverlay(file.path, 'broken', 'utf-8')).toBe(false);
    expect(store.selectedCommitHash).toBe('broken');
    expect(store.drilldownOverlay).toBeUndefined();
    expect(store.drilldownLoading).toBe(false);
    expect(store.drilldownError).toBe('提交加载失败');
    expect(store.error).toBe('提交加载失败');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '提交差异加载失败',
        message: `${file.path} @ broken：提交加载失败`,
        source: 'Review'
      })
    ]);

    store.closeCommitDrilldown();
    expect(store.selectedCommitHash).toBeUndefined();
    expect(store.drilldownError).toBeUndefined();
  });

  it('切换提交时立即清除旧 overlay，旧成功不得覆盖新提交', async () => {
    const oldOverlay = { ...overlay, mode: 'commit' as const, parentHash: 'old-parent' };
    const newOverlay = { ...overlay, mode: 'commit' as const, parentHash: 'new-parent' };
    let resolveOld!: (value: FileOverlay) => void;
    vi.mocked(revierClient.review.getCommitOverlay)
      .mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve; }))
      .mockResolvedValueOnce(newOverlay);
    const store = useReviewStore();
    store.task = task;
    store.drilldownOverlay = oldOverlay;

    const oldRequest = store.loadCommitOverlay(file.path, 'old', 'utf-8');
    expect(store.drilldownOverlay).toBeUndefined();
    expect(store.selectedCommitHash).toBe('old');
    expect(await store.loadCommitOverlay(file.path, 'new', 'gb18030')).toBe(true);
    resolveOld(oldOverlay);

    expect(await oldRequest).toBe(false);
    expect(store.selectedCommitHash).toBe('new');
    expect(store.drilldownOverlay).toStrictEqual(newOverlay);
  });

  it('旧提交请求后到达的失败不得清除新提交选择', async () => {
    let rejectOld!: (reason: unknown) => void;
    const newOverlay = { ...overlay, mode: 'commit' as const, parentHash: 'new-parent' };
    vi.mocked(revierClient.review.getCommitOverlay)
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockResolvedValueOnce(newOverlay);
    const store = useReviewStore();
    store.task = task;

    const oldRequest = store.loadCommitOverlay(file.path, 'old', 'utf-8');
    expect(await store.loadCommitOverlay(file.path, 'new', 'utf-8')).toBe(true);
    rejectOld(new Error('旧请求失败'));

    expect(await oldRequest).toBe(false);
    expect(store.selectedCommitHash).toBe('new');
    expect(store.drilldownOverlay).toStrictEqual(newOverlay);
    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
  });

  it('提交编码重载保留旧 overlay，后到达的旧请求不能覆盖新请求', async () => {
    const commitOverlay = { ...overlay, mode: 'commit' as const, parentHash: 'parent' };
    const latestOverlay = { ...commitOverlay, resolvedEncoding: 'utf-16le' as const };
    let resolveOld!: (value: FileOverlay) => void;
    vi.mocked(revierClient.review.getCommitOverlay)
      .mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve; }))
      .mockResolvedValueOnce(latestOverlay);

    const store = useReviewStore();
    store.task = task;
    store.drilldownOverlay = commitOverlay;
    store.selectedCommitHash = 'abc123';
    const oldRequest = store.reloadCommitOverlayEncoding(file.path, 'abc123', 'gb18030');
    expect(store.drilldownOverlay).toStrictEqual(commitOverlay);

    expect(await store.reloadCommitOverlayEncoding(file.path, 'abc123', 'utf-16le')).toBe(true);
    resolveOld(commitOverlay);
    expect(await oldRequest).toBe(false);
    expect(store.drilldownOverlay).toEqual(latestOverlay);
  });

  it('提交编码重载失败时保留旧 overlay 并记录错误', async () => {
    const commitOverlay = { ...overlay, mode: 'commit' as const, parentHash: 'parent' };
    vi.mocked(revierClient.review.getCommitOverlay).mockRejectedValue(new Error('提交解码失败'));
    const store = useReviewStore();
    store.task = task;
    store.drilldownOverlay = commitOverlay;
    store.selectedCommitHash = 'abc123';

    expect(await store.reloadCommitOverlayEncoding(file.path, 'abc123', 'gb18030')).toBe(false);
    expect(store.drilldownOverlay).toStrictEqual(commitOverlay);
    expect(store.error).toBe('提交解码失败');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '提交编码重载失败',
        message: `${file.path} @ abc123（gb18030）：提交解码失败`,
        source: 'Review'
      })
    ]);
  });
});
