import { createPinia, setActivePinia } from 'pinia';
import { useProjectStore } from '../../src/renderer/stores/projectStore';
import { revierClient } from '../../src/renderer/api/revierClient';
import { useNotifications } from '../../src/renderer/composables/useNotifications';
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
    useNotifications().clear();
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

  it('项目列表加载失败时保留页面错误并记录通知', async () => {
    vi.mocked(revierClient.projects.list).mockRejectedValue(new Error('项目列表不可用'));
    const store = useProjectStore();

    await store.loadProjects();

    expect(store.error).toBe('项目列表不可用');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error', title: '项目列表加载失败', message: '项目列表不可用', source: 'Projects'
      })
    ]);
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

  it('筛选偏好保存失败时保留局部错误并记录项目上下文', async () => {
    vi.mocked(revierClient.projects.update).mockRejectedValue(new Error('保存失败'));
    const store = useProjectStore();
    store.projects = [project];

    await store.saveReviewFilters(project.id, { branch: 'main' });

    expect(store.error).toBe('保存失败');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '分析筛选保存失败',
        message: 'Revier：保存失败',
        source: 'Projects'
      })
    ]);
  });

  it('已被新筛选替代的保存失败不得污染错误或通知', async () => {
    let rejectOld!: (reason: unknown) => void;
    vi.mocked(revierClient.projects.update)
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockImplementationOnce(async (updatedProject) => updatedProject);
    const store = useProjectStore();
    store.projects = [project];

    const oldSave = store.saveReviewFilters(project.id, { branch: 'develop' });
    await store.saveReviewFilters(project.id, { branch: 'main' });
    rejectOld(new Error('旧筛选保存失败'));
    await oldSave;

    expect(store.error).toBeUndefined();
    expect(useNotifications().notifications.value).toEqual([]);
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
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '项目添加失败',
        message: 'E:/Projects/revier：请选择一个 Git 仓库目录',
        source: 'Projects'
      })
    ]);
  });

  it('删除项目失败时保留项目与局部错误并记录通知', async () => {
    vi.mocked(revierClient.projects.remove).mockRejectedValue(new Error('删除失败'));
    const store = useProjectStore();
    store.projects = [project];

    await store.removeProject(project.id);

    expect(store.projects).toStrictEqual([project]);
    expect(store.error).toBe('删除失败');
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error', title: '项目删除失败', message: 'Revier：删除失败', source: 'Projects'
      })
    ]);
  });
});
