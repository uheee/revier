// @vitest-environment jsdom
vi.mock('vue-router', () => ({
  useRoute: () => ({ params: { projectId: 'project-1' } }),
  useRouter: () => ({ push: vi.fn() })
}));

import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import ReviewWorkspace from '../../src/renderer/pages/ReviewWorkspace.vue';
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

describe('ReviewWorkspace', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-12T12:00:00.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('筛选条件变化后防抖异步保存到项目配置', async () => {
    const api = mockApi({
      list: vi.fn(async () => [project]),
      update: vi.fn(async (updatedProject) => updatedProject),
      listBranches: vi.fn(async () => [
        { name: 'develop', current: true },
        { name: 'main', current: false }
      ])
    });
    const testWindow = Object.create(window) as Window & { revier: RevierApi };
    Object.assign(testWindow, {
      innerWidth: 1200,
      revier: api
    });
    vi.stubGlobal('window', testWindow);

    const wrapper = mount(ReviewWorkspace, {
      global: {
        stubs: {
          FilterPanel: {
            props: ['projectId'],
            emits: ['change', 'submit'],
            template:
              '<button type="button" data-test="change-filter" @click="$emit(\'change\', { projectId, branch: \'main\', startAt: \'2026-05-01T00:00:00.000Z\', endAt: \'2026-06-01T00:00:00.000Z\', authorKeys: [\'alice@example.com\'], messageQuery: \'feature\', globRules: [\'docs/**/*.md\'] })">change</button>'
          },
          TaskProgress: true,
          ChangedFileList: true,
          DiffViewer: true,
          DiffDrilldownOverlay: true,
          BlockDetailPanel: true,
          ReviewLayoutResizer: true,
          'n-button': {
            template: '<button type="button"><slot /></button>'
          }
        }
      }
    });

    await flushPromises();
    await wrapper.get('[data-test="change-filter"]').trigger('click');

    expect(api.projects.update).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    expect(api.projects.update).toHaveBeenCalledWith(
      expect.objectContaining({
        id: project.id,
        preferences: expect.objectContaining({
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
        })
      })
    );
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
      selectDirectory: vi.fn(),
      ...projects
    },
    review: {
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(() => vi.fn()),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn(),
      listAuthors: vi.fn(async () => []),
      getCommitOverlay: vi.fn()
    }
  } as unknown as RevierApi;
}
