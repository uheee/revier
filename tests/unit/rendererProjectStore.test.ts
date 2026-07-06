import { createPinia, setActivePinia } from 'pinia';
import { useProjectStore } from '../../src/renderer/stores/projectStore';
import { revierClient } from '../../src/renderer/api/revierClient';
import type { ReviewProject } from '../../src/renderer/generated/bindings';

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
    vi.mocked(revierClient.projects.list).mockReset();
    vi.mocked(revierClient.projects.add).mockReset();
    vi.mocked(revierClient.projects.update).mockReset();
    vi.mocked(revierClient.projects.remove).mockReset();
  });

  it('loads projects through Tauri client', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);

    const store = useProjectStore();
    await store.loadProjects();

    expect(revierClient.projects.list).toHaveBeenCalledTimes(1);
    expect(store.projects).toEqual([project]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeUndefined();
  });

  it('adds and removes projects then refreshes the list', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([]);
    vi.mocked(revierClient.projects.add).mockResolvedValue(project);
    vi.mocked(revierClient.projects.remove).mockResolvedValue(undefined);

    const store = useProjectStore();
    await store.addProject(project.repoPath, project.name);
    await store.removeProject(project.id);

    expect(revierClient.projects.add).toHaveBeenCalledWith(project.repoPath, {
      name: project.name
    });
    expect(revierClient.projects.remove).toHaveBeenCalledWith(project.id);
    expect(revierClient.projects.list).toHaveBeenCalledTimes(2);
  });

  it('异步保存项目的分析筛选偏好且不进入全局加载状态', async () => {
    vi.mocked(revierClient.projects.update).mockImplementation(async (updatedProject) => updatedProject);

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

    expect(revierClient.projects.update).toHaveBeenCalledWith(
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
    vi.mocked(revierClient.projects.add).mockRejectedValue(
      { code: 'REPOSITORY_INVALID', message: '请选择一个 Git 仓库目录', detail: project.repoPath }
    );
    vi.mocked(revierClient.projects.list).mockResolvedValue([]);

    const store = useProjectStore();
    await store.addProject(project.repoPath, project.name);

    expect(store.error).toBe('请选择一个 Git 仓库目录');
    expect(store.loading).toBe(false);
  });
});
