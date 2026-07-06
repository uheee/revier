import { createPinia, setActivePinia } from 'pinia';
import { useReviewStore } from '../../src/renderer/stores/reviewStore';
import { revierClient } from '../../src/renderer/api/revierClient';
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
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn(),
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
  authors: [{ name: 'Snowind', email: 'jinks.tao@gmail.com' }],
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
  warnings: []
};

describe('renderer reviewStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(revierClient.review.startAnalysis).mockReset();
    vi.mocked(revierClient.review.listChangedFiles).mockReset();
    vi.mocked(revierClient.review.cancelAnalysis).mockReset();
    vi.mocked(revierClient.review.getFileOverlay).mockReset();
    vi.mocked(revierClient.review.getCommitOverlay).mockReset();
    vi.mocked(revierClient.review.listAuthors).mockReset();
  });

  it('starts analysis and loads changed files after completion event', async () => {
    vi.mocked(revierClient.review.startAnalysis).mockResolvedValue(runningTask);
    vi.mocked(revierClient.review.listChangedFiles).mockResolvedValue([file]);

    const store = useReviewStore();
    await store.start(filters);

    expect(revierClient.review.startAnalysis).toHaveBeenCalledWith(filters);
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

  it('loads overlay and selects a diff block', async () => {
    vi.mocked(revierClient.review.getFileOverlay).mockResolvedValue(overlay);

    const store = useReviewStore();
    store.task = task;
    await store.loadOverlay(file.path);
    store.selectBlock(block);

    expect(revierClient.review.getFileOverlay).toHaveBeenCalledWith({
      taskId: task.taskId,
      filePath: file.path
    });
    expect(store.overlay).toEqual(overlay);
    expect(store.selectedBlock).toEqual(block);
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
    await store.loadCommitOverlay(file.path, 'abc123');

    expect(revierClient.review.getCommitOverlay).toHaveBeenCalledWith({
      taskId: task.taskId,
      filePath: file.path,
      commitHash: 'abc123'
    });
    expect(store.drilldownOverlay).toEqual(commitOverlay);
    expect(store.selectedCommitHash).toBe('abc123');

    store.closeCommitDrilldown();
    expect(store.drilldownOverlay).toBeUndefined();
    expect(store.selectedCommitHash).toBeUndefined();
  });
});
