// @vitest-environment jsdom
vi.mock('vue-router', () => ({
  useRoute: () => ({ params: { projectId: 'project-1' } }),
  useRouter: () => ({ push: vi.fn() })
}));

const editorSettings = {
  version: 1,
  theme: 'dark' as const,
  defaultEncoding: 'utf-8' as const,
  editor: { fontFamilies: ['monospace'], fontSize: 13, lineHeight: 22, minimap: true },
  largeFile: { maxBytes: 1_048_576, maxLines: 5_000 },
  themes: {}
};

vi.mock('../../src/renderer/composables/useEditorSettings', () => ({
  useEditorSettings: () => ({
    snapshot: { value: { settings: editorSettings } },
    effectiveTheme: { value: 'dark' }
  })
}));

vi.mock('../../src/renderer/components/review/DiffViewer.vue', () => ({
  default: {
    name: 'DiffViewer',
    props: ['overlay', 'loading', 'selectedBlockId', 'settings', 'themeName', 'requestedEncoding', 'contextKey'],
    emits: ['selected', 'draftChange', 'encodingChange'],
    template: '<div data-testid="diff-viewer-stub"><button data-test="select-block" @click="$emit(\'selected\', overlay.blocks[0])"/><button data-test="draft" @click="$emit(\'draftChange\', true)"/><button data-test="encoding" @click="$emit(\'encodingChange\', \'gb18030\')"/></div>'
  }
}));

import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import ReviewWorkspace from '../../src/renderer/pages/ReviewWorkspace.vue';
import { revierClient } from '../../src/renderer/api/revierClient';
import { useReviewStore } from '../../src/renderer/stores/reviewStore';
import type { DiffBlock, FileOverlay, ReviewProject } from '../../src/renderer/generated/bindings';

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

const block: DiffBlock = {
  id: 'block-1', oldStart: 1, oldEnd: 1, newStart: 1, newEnd: 1,
  changeType: 'modified', authors: [], rows: [], relatedCommits: []
};
const overlay: FileOverlay = {
  mode: 'range',
  file: { path: 'src/app.ts', status: 'modified', additions: 1, deletions: 1, isBinary: false, isPreviewable: true },
  range: { branch: 'develop', baseCommit: 'base', headCommit: 'head' },
  blocks: [block], rows: [], warnings: [], oldContent: 'old', newContent: 'new', resolvedEncoding: 'utf-8'
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

  it('接入设置、草稿销毁、编码原子更新并保留完整左栏', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.listBranches).mockResolvedValue([{ name: 'develop', current: true }]);
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockResolvedValue(vi.fn());
    vi.mocked(revierClient.review.getFileOverlay)
      .mockResolvedValueOnce(overlay)
      .mockResolvedValueOnce({ ...overlay, resolvedEncoding: 'gb18030', newContent: '新内容' })
      .mockRejectedValueOnce(new Error('解码失败'))
      .mockResolvedValueOnce({ ...overlay, file: { ...overlay.file, path: 'src/other.ts' } });

    const store = useReviewStore();
    store.task = { taskId: 'task-1', projectId: project.id, status: 'completed', stage: 'ready' };
    store.files = [overlay.file, { ...overlay.file, path: 'src/other.ts' }];
    const wrapper = mount(ReviewWorkspace, {
      global: { stubs: {
        FilterPanel: { template: '<section data-test="filters"><i data-filter="branch"/><i data-filter="author"/><i data-filter="time"/><i data-filter="message"/><i data-filter="glob"/></section>' },
        TaskProgress: true,
        ChangedFileList: { emits: ['selected'], template: '<div><button data-test="app-file" @click="$emit(\'selected\', \'src/app.ts\')">主文件</button><button data-test="other-file" @click="$emit(\'selected\', \'src/other.ts\')">其他文件</button></div>' },
        DiffDrilldownOverlay: true,
        BlockDetailPanel: { name: 'BlockDetailPanel', props: ['block', 'draft'], template: '<aside data-test="detail" />' },
        ReviewLayoutResizer: true,
        'n-button': { template: '<button><slot /></button>' }
      } }
    });
    await flushPromises();

    await wrapper.get('[data-test="app-file"]').trigger('click');
    await flushPromises();
    expect(revierClient.review.getFileOverlay).toHaveBeenNthCalledWith(1, {
      taskId: 'task-1', filePath: 'src/app.ts', encoding: 'utf-8'
    });
    let viewer = wrapper.getComponent({ name: 'DiffViewer' });
    expect(viewer.props()).toMatchObject({ settings: editorSettings, themeName: 'revier-dark', requestedEncoding: 'utf-8' });
    await wrapper.get('[data-test="select-block"]').trigger('click');
    expect(wrapper.getComponent({ name: 'BlockDetailPanel' }).props('block')).toEqual(block);

    await wrapper.get('[data-test="draft"]').trigger('click');
    expect(store.selectedBlock).toBeUndefined();
    expect(wrapper.getComponent({ name: 'BlockDetailPanel' }).props()).toMatchObject({ draft: true, block: undefined });

    await wrapper.get('[data-test="encoding"]').trigger('click');
    await flushPromises();
    viewer = wrapper.getComponent({ name: 'DiffViewer' });
    expect(viewer.props('requestedEncoding')).toBe('gb18030');
    expect(viewer.props('overlay').newContent).toBe('新内容');

    await wrapper.get('[data-test="encoding"]').trigger('click');
    await flushPromises();
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('overlay').newContent).toBe('新内容');
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('requestedEncoding')).toBe('gb18030');

    await wrapper.get('[data-test="other-file"]').trigger('click');
    await flushPromises();
    expect(revierClient.review.getFileOverlay).toHaveBeenLastCalledWith(expect.objectContaining({ filePath: 'src/other.ts', encoding: 'utf-8' }));
    expect(wrapper.getComponent({ name: 'BlockDetailPanel' }).props('draft')).toBe(false);
    expect(wrapper.findAll('[data-test="filters"] [data-filter]')).toHaveLength(5);
    expect(wrapper.find('[data-test="other-file"]').exists()).toBe(true);
  });
});
