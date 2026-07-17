// @vitest-environment jsdom
vi.mock('vue-router', () => ({
  useRoute: () => ({ params: { projectId: 'project-1' } }),
  useRouter: () => ({ push: vi.fn() })
}));

vi.mock('../../src/renderer/components/review/DiffViewer.vue', () => ({
  default: {
    name: 'DiffViewer',
    props: ['overlay', 'loading', 'selectedBlockId'],
    emits: ['selected'],
    template: '<div data-testid="diff-viewer-stub" />'
  }
}));

import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import ReviewWorkspace from '../../src/renderer/pages/ReviewWorkspace.vue';
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

describe('ReviewWorkspace', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-12T12:00:00.000Z'));
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1200 });
    vi.mocked(revierClient.projects.list).mockReset();
    vi.mocked(revierClient.projects.update).mockReset();
    vi.mocked(revierClient.projects.listBranches).mockReset();
    vi.mocked(revierClient.review.listAuthors).mockReset();
    vi.mocked(revierClient.review.onTaskUpdate).mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('筛选条件变化后防抖异步保存到项目配置', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.update).mockImplementation(async (updatedProject) => updatedProject);
    vi.mocked(revierClient.projects.listBranches).mockResolvedValue([
      { name: 'develop', current: true },
      { name: 'main', current: false }
    ]);
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockResolvedValue(vi.fn());

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

    expect(revierClient.projects.update).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    expect(revierClient.projects.update).toHaveBeenCalledWith(
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
