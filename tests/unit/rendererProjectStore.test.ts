import { createPinia, setActivePinia } from 'pinia';
import { useProjectStore } from '../../src/renderer/stores/projectStore';
import type { RevierApi } from '../../src/shared/ipcTypes';
import type { ReviewProject } from '../../src/shared/projectTypes';

const project: ReviewProject = {
  id: 'project-1',
  name: 'Revier',
  repoPath: 'E:/Projects/revier',
  pinned: true,
  lastOpenedAt: '2026-06-03T00:00:00.000Z',
  preferences: {
    defaultBranch: 'develop',
    defaultDays: 30,
    defaultGlobRules: ['src/**/*.ts']
  }
};

describe('renderer projectStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('loads projects through preload API', async () => {
    const api = mockApi({
      list: vi.fn(async () => [project])
    });
    vi.stubGlobal('window', { revier: api });

    const store = useProjectStore();
    await store.loadProjects();

    expect(api.projects.list).toHaveBeenCalledTimes(1);
    expect(store.projects).toEqual([project]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeUndefined();
  });

  it('adds and removes projects then refreshes the list', async () => {
    const api = mockApi({
      list: vi.fn(async () => []),
      add: vi.fn(async () => project),
      remove: vi.fn(async () => undefined)
    });
    vi.stubGlobal('window', { revier: api });

    const store = useProjectStore();
    await store.addProject(project.repoPath, project.name);
    await store.removeProject(project.id);

    expect(api.projects.add).toHaveBeenCalledWith(project.repoPath, { name: project.name });
    expect(api.projects.remove).toHaveBeenCalledWith(project.id);
    expect(api.projects.list).toHaveBeenCalledTimes(2);
  });

  it('异步保存项目的分析筛选偏好且不进入全局加载状态', async () => {
    const api = mockApi({
      update: vi.fn(async (updatedProject) => updatedProject)
    });
    vi.stubGlobal('window', { revier: api });

    const store = useProjectStore();
    store.projects = [project];
    const savePromise = store.saveReviewFilters(project.id, {
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-06-01T00:00:00.000Z',
      authorKeys: ['alice@example.com'],
      messageQuery: 'feature',
      globRules: ['docs/**/*.md']
    });

    expect(store.loading).toBe(false);
    expect(store.projects[0].preferences).toMatchObject({
      defaultBranch: 'main',
      defaultGlobRules: ['docs/**/*.md'],
      reviewFilters: {
        branch: 'main',
        startAt: '2026-05-01T00:00:00.000Z',
        endAt: '2026-06-01T00:00:00.000Z',
        authorKeys: ['alice@example.com'],
        messageQuery: 'feature',
        globRules: ['docs/**/*.md']
      }
    });

    await savePromise;

    expect(api.projects.update).toHaveBeenCalledWith(
      expect.objectContaining({
        id: project.id,
        preferences: expect.objectContaining({
          reviewFilters: expect.objectContaining({
            branch: 'main'
          })
        })
      })
    );
  });

  it('stores add-project errors from duplicate or invalid repositories', async () => {
    const api = mockApi({
      add: vi.fn(async () => {
        throw new Error('该仓库已在项目列表中');
      })
    });
    vi.stubGlobal('window', { revier: api });

    const store = useProjectStore();
    await store.addProject(project.repoPath, project.name);

    expect(store.error).toBe('该仓库已在项目列表中');
    expect(store.loading).toBe(false);
  });
});

function mockApi(projects: Partial<RevierApi['projects']>): RevierApi {
  return {
    projects: {
      list: vi.fn(async () => []),
      add: vi.fn(),
      update: vi.fn(),
      remove: vi.fn(),
      validateRepository: vi.fn(),
      listBranches: vi.fn(),
      ...projects
    },
    review: {
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn()
    }
  } as unknown as RevierApi;
}
