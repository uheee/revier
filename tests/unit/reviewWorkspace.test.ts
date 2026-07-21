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
    template: '<div data-testid="diff-viewer-stub" :data-context-key="contextKey"><span v-if="overlay" :key="contextKey" data-test="editor-session"/><button v-if="overlay" data-test="select-block" @click="$emit(\'selected\', overlay.blocks[0])"/><button v-if="overlay" data-test="draft" @click="$emit(\'draftChange\', true)"/><button v-if="overlay" data-test="encoding" @click="$emit(\'encodingChange\', \'gb18030\')"/></div>'
  }
}));

import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import ReviewWorkspace from '../../src/renderer/pages/ReviewWorkspace.vue';
import { revierClient } from '../../src/renderer/api/revierClient';
import { useReviewStore } from '../../src/renderer/stores/reviewStore';
import { useNotifications } from '../../src/renderer/composables/useNotifications';
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
    vi.mocked(revierClient.review.restoreBranchAnalysis).mockReset();
    vi.mocked(revierClient.review.restoreBranchAnalysis).mockResolvedValue({
      projectId: project.id,
      branch: 'develop',
      cacheState: 'miss',
      cacheHit: false,
      stale: false,
      files: [],
      currentHead: 'head',
      cacheReadElapsedMs: 1
    });
    vi.mocked(revierClient.review.setBranchSelectedFile).mockReset();
    vi.mocked(revierClient.review.setBranchSelectedFile).mockResolvedValue();
    vi.mocked(revierClient.review.onOperationProgress).mockReset();
    vi.mocked(revierClient.review.onOperationProgress).mockResolvedValue(vi.fn());
    vi.mocked(revierClient.review.onTaskUpdate).mockReset();
    vi.mocked(revierClient.review.getFileOverlay).mockReset();
    vi.mocked(revierClient.review.getCommitOverlay).mockReset();
    useNotifications().clear();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
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

  it('分支元数据加载失败时通知且继续加载作者筛选', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.listBranches).mockRejectedValue(new Error('分支读取失败'));
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockResolvedValue(vi.fn());

    mount(ReviewWorkspace, { global: { stubs: {
      FilterPanel: true, TaskProgress: true, ChangedFileList: true, DiffViewer: true,
      DiffDrilldownOverlay: true, BlockDetailPanel: true, ReviewLayoutResizer: true,
      'n-button': { template: '<button><slot /></button>' }
    } } });
    await flushPromises();

    expect(revierClient.review.listAuthors).toHaveBeenCalledWith({
      projectId: project.id,
      branch: 'develop'
    });
    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '项目分支加载失败',
        message: 'project-1：分支读取失败',
        source: 'Review'
      })
    ]);
  });

  it('任务事件订阅失败时记录可追溯通知', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.listBranches).mockResolvedValue([]);
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockRejectedValue(new Error('事件通道不可用'));

    mount(ReviewWorkspace, { global: { stubs: {
      FilterPanel: true, TaskProgress: true, ChangedFileList: true, DiffViewer: true,
      DiffDrilldownOverlay: true, BlockDetailPanel: true, ReviewLayoutResizer: true,
      'n-button': { template: '<button><slot /></button>' }
    } } });
    await flushPromises();

    expect(useNotifications().notifications.value).toEqual([
      expect.objectContaining({
        type: 'error',
        title: '分析任务事件订阅失败',
        message: 'project-1：事件通道不可用',
        source: 'Review'
      })
    ]);
  });

  it('草稿中直接切换文件会替换编辑器上下文且不弹确认，并保留真实筛选栏和文件列表', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.listBranches).mockResolvedValue([{ name: 'develop', current: true }]);
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockResolvedValue(vi.fn());
    let resolveOther!: (value: FileOverlay) => void;
    vi.mocked(revierClient.review.getFileOverlay)
      .mockResolvedValueOnce(overlay)
      .mockReturnValueOnce(new Promise((resolve) => { resolveOther = resolve; }));
    const confirm = vi.spyOn(window, 'confirm');

    const store = useReviewStore();
    store.task = { taskId: 'task-1', projectId: project.id, status: 'completed', stage: 'ready' };
    store.files = [overlay.file, { ...overlay.file, path: 'src/other.ts' }];
    const wrapper = mount(ReviewWorkspace, {
      global: { stubs: {
        TaskProgress: true,
        DiffDrilldownOverlay: true,
        BlockDetailPanel: { name: 'BlockDetailPanel', props: ['block', 'draft'], template: '<aside data-test="detail" />' },
        ReviewLayoutResizer: true,
        'n-button': { template: '<button type="button"><slot /></button>' },
        'n-select': true,
        'n-date-picker': true,
        'n-input': true,
        'n-tag': { template: '<span><slot /></span>' },
        'n-empty': true
      } }
    });
    await flushPromises();

    await wrapper.findAll('.changed-file-row')[0].trigger('click');
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

    const previousContextKey = viewer.props('contextKey');
    const previousSession = wrapper.get('[data-test="editor-session"]').element;
    await wrapper.findAll('.changed-file-row')[1].trigger('click');
    await wrapper.vm.$nextTick();
    expect(revierClient.review.getFileOverlay).toHaveBeenLastCalledWith(expect.objectContaining({ filePath: 'src/other.ts', encoding: 'utf-8' }));
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('overlay')).toBeUndefined();
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('contextKey')).not.toBe(previousContextKey);
    expect(wrapper.find('[data-test="editor-session"]').exists()).toBe(false);
    expect(previousSession.isConnected).toBe(false);
    expect(wrapper.getComponent({ name: 'BlockDetailPanel' }).props('draft')).toBe(false);
    expect(confirm).not.toHaveBeenCalled();
    resolveOther({ ...overlay, file: { ...overlay.file, path: 'src/other.ts' } });
    await flushPromises();
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('overlay').file.path).toBe('src/other.ts');
    expect(wrapper.get('[data-test="editor-session"]').element).not.toBe(previousSession);

    const filterPanel = wrapper.get('.filter-panel');
    expect(filterPanel.text()).toContain('分支');
    expect(filterPanel.text()).toContain('时间范围');
    expect(filterPanel.text()).toContain('作者');
    expect(filterPanel.text()).toContain('提交信息');
    expect(filterPanel.text()).toContain('文件规则');
    expect(wrapper.get('.changed-file-list').text()).toContain('src/app.ts');
    expect(wrapper.get('.changed-file-list').text()).toContain('src/other.ts');
  });

  it('编码重载仅在成功后更新请求编码，失败时保留当前内容', async () => {
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.listBranches).mockResolvedValue([]);
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockResolvedValue(vi.fn());
    vi.mocked(revierClient.review.getFileOverlay)
      .mockResolvedValueOnce(overlay)
      .mockResolvedValueOnce({ ...overlay, resolvedEncoding: 'gb18030', newContent: '新内容' })
      .mockRejectedValueOnce(new Error('解码失败'));
    const store = useReviewStore();
    store.task = { taskId: 'task-1', projectId: project.id, status: 'completed', stage: 'ready' };
    store.files = [overlay.file];
    const wrapper = mount(ReviewWorkspace, { global: { stubs: {
      FilterPanel: true, TaskProgress: true,
      ChangedFileList: { emits: ['selected'], template: '<button data-test="app-file" @click="$emit(\'selected\', \'src/app.ts\')" />' },
      DiffDrilldownOverlay: true, BlockDetailPanel: true, ReviewLayoutResizer: true,
      'n-button': { template: '<button><slot /></button>' }
    } } });
    await flushPromises();
    await wrapper.get('[data-test="app-file"]').trigger('click');
    await flushPromises();
    await wrapper.get('[data-test="encoding"]').trigger('click');
    await flushPromises();
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('requestedEncoding')).toBe('gb18030');
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('overlay').newContent).toBe('新内容');

    await wrapper.get('[data-test="encoding"]').trigger('click');
    await flushPromises();
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('requestedEncoding')).toBe('gb18030');
    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('overlay').newContent).toBe('新内容');
  });

  it('提交下钻失败时保留下钻错误页，且不重建底层文件编辑器上下文', async () => {
    const commit = {
      hash: 'abc123', shortHash: 'abc123', authorName: 'Alice',
      authorEmail: 'alice@example.com', committedAt: '2026-06-12T00:00:00.000Z',
      subject: 'fix: 单提交变更', matchedByFilter: true, touchedRanges: []
    };
    vi.mocked(revierClient.projects.list).mockResolvedValue([project]);
    vi.mocked(revierClient.projects.listBranches).mockResolvedValue([]);
    vi.mocked(revierClient.review.listAuthors).mockResolvedValue([]);
    vi.mocked(revierClient.review.onTaskUpdate).mockResolvedValue(vi.fn());
    vi.mocked(revierClient.review.getFileOverlay).mockResolvedValue(overlay);
    vi.mocked(revierClient.review.getCommitOverlay).mockRejectedValue(new Error('提交差异不可用'));
    const store = useReviewStore();
    store.task = { taskId: 'task-1', projectId: project.id, status: 'completed', stage: 'ready' };
    store.files = [overlay.file];

    const wrapper = mount(ReviewWorkspace, { global: { stubs: {
      FilterPanel: true,
      TaskProgress: true,
      ChangedFileList: {
        emits: ['selected'],
        template: '<button data-test="select-file" @click="$emit(\'selected\', \'src/app.ts\')" />'
      },
      DiffDrilldownOverlay: {
        name: 'DiffDrilldownOverlay',
        props: ['selectedCommitHash', 'selectedCommit', 'error'],
        emits: ['close'],
        template: '<aside data-test="drilldown" :data-hash="selectedCommitHash">{{ error }}<button data-test="close-drilldown" @click="$emit(\'close\')" /></aside>'
      },
      BlockDetailPanel: {
        emits: ['commitSelected'],
        template: '<button data-test="select-commit" @click="$emit(\'commitSelected\', commit)" />',
        data: () => ({ commit })
      },
      ReviewLayoutResizer: true,
      'n-button': { template: '<button><slot /></button>' }
    } } });
    await flushPromises();
    await wrapper.get('[data-test="select-file"]').trigger('click');
    await flushPromises();
    const fileContextKey = wrapper.getComponent({ name: 'DiffViewer' }).props('contextKey');
    const fileEditorSession = wrapper.get('[data-test="editor-session"]').element;

    await wrapper.get('[data-test="select-commit"]').trigger('click');
    await flushPromises();

    expect(wrapper.getComponent({ name: 'DiffViewer' }).props('contextKey')).toBe(fileContextKey);
    expect(wrapper.getComponent({ name: 'DiffViewer' }).classes()).toContain('is-drilldown-covered');
    expect(wrapper.get('[data-test="drilldown"]').attributes('data-hash')).toBe(commit.hash);
    expect(wrapper.get('[data-test="drilldown"]').text()).toContain('提交差异不可用');

    await wrapper.get('[data-test="close-drilldown"]').trigger('click');
    await wrapper.vm.$nextTick();
    expect(wrapper.getComponent({ name: 'DiffViewer' }).classes()).not.toContain('is-drilldown-covered');
    expect(wrapper.get('[data-test="editor-session"]').element).toBe(fileEditorSession);
  });
});
