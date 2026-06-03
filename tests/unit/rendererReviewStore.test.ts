import { createPinia, setActivePinia } from 'pinia';
import { useReviewStore } from '../../src/renderer/stores/reviewStore';
import type { RevierApi } from '../../src/shared/ipcTypes';
import type {
  AnalysisTaskSnapshot,
  ChangedFile,
  DiffBlock,
  FileOverlay,
  ReviewAuthorOptionsRequest,
  ReviewFilters
} from '../../src/shared/reviewTypes';

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
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('starts analysis and loads changed files', async () => {
    const api = mockApi({
      startAnalysis: vi.fn(async () => task),
      listChangedFiles: vi.fn(async () => [file])
    });
    vi.stubGlobal('window', { revier: api });

    const store = useReviewStore();
    await store.start(filters);

    expect(api.review.startAnalysis).toHaveBeenCalledWith(filters);
    expect(api.review.listChangedFiles).toHaveBeenCalledWith(task.taskId);
    expect(store.task).toEqual(task);
    expect(store.files).toEqual([file]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeUndefined();
  });

  it('loads overlay and selects a diff block', async () => {
    const api = mockApi({
      getFileOverlay: vi.fn(async () => overlay)
    });
    vi.stubGlobal('window', { revier: api });

    const store = useReviewStore();
    store.task = task;
    await store.loadOverlay(file.path);
    store.selectBlock(block);

    expect(api.review.getFileOverlay).toHaveBeenCalledWith({
      taskId: task.taskId,
      filePath: file.path
    });
    expect(store.overlay).toEqual(overlay);
    expect(store.selectedBlock).toEqual(block);
  });

  it('loads author filter options', async () => {
    const request: ReviewAuthorOptionsRequest = {
      projectId: 'project-1',
      branch: 'develop'
    };
    const authors = [
      { key: 'alice@example.com', name: 'Alice', email: 'alice@example.com', commitCount: 2 }
    ];
    const api = mockApi({
      listAuthors: vi.fn(async () => authors)
    });
    vi.stubGlobal('window', { revier: api });

    const store = useReviewStore();
    await store.loadAuthors(request);

    expect(api.review.listAuthors).toHaveBeenCalledWith(request);
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
    const api = mockApi({
      getCommitOverlay: vi.fn(async () => commitOverlay)
    });
    vi.stubGlobal('window', { revier: api });

    const store = useReviewStore();
    store.task = task;
    await store.loadCommitOverlay(file.path, 'abc123');

    expect(api.review.getCommitOverlay).toHaveBeenCalledWith({
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

function mockApi(review: Partial<RevierApi['review']>): RevierApi {
  return {
    projects: {
      list: vi.fn(),
      add: vi.fn(),
      update: vi.fn(),
      remove: vi.fn(),
      validateRepository: vi.fn(),
      listBranches: vi.fn()
    },
    review: {
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn(),
      listAuthors: vi.fn(),
      getCommitOverlay: vi.fn(),
      ...review
    }
  } as unknown as RevierApi;
}
