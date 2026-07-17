// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import type { EditorSettings, FileOverlay } from '../../src/renderer/generated/bindings';

const surfaceStub = vi.hoisted(() => ({
  name: 'MonacoDiffSurface',
  props: ['path', 'oldContent', 'newContent', 'contextKey', 'languageId', 'settings', 'themeName', 'blocks', 'selectedBlock'],
  emits: ['draftChange', 'selected', 'cursorChange', 'editorsReady'],
  template: '<div data-testid="surface" :data-language="languageId" :data-context="contextKey" />'
}));
const railStub = vi.hoisted(() => ({
  name: 'DiffAuthorRail',
  props: ['blocks', 'draft', 'originalEditor', 'modifiedEditor'],
  emits: ['selected'],
  template: '<aside data-testid="author-rail" />'
}));
const statusStub = vi.hoisted(() => ({
  name: 'EditorStatusBar',
  props: ['mode', 'line', 'column', 'resolvedEncoding', 'requestedEncoding', 'languageId', 'binary', 'selectedBlockIndex', 'blockCount'],
  emits: ['encodingChange', 'languageChange'],
  template: '<footer data-testid="status" :data-mode="mode" :data-language="languageId" />'
}));

vi.mock('../../src/renderer/components/review/MonacoDiffSurface.vue', () => ({ default: surfaceStub }));
vi.mock('../../src/renderer/components/review/DiffAuthorRail.vue', () => ({ default: railStub }));
vi.mock('../../src/renderer/components/review/EditorStatusBar.vue', () => ({ default: statusStub }));

import DiffViewer from '../../src/renderer/components/review/DiffViewer.vue';

const settings = {
  version: 1,
  theme: 'dark',
  defaultEncoding: 'auto',
  editor: { fontFamilies: ['monospace'], fontSize: 13, lineHeight: 22, minimap: true },
  largeFile: { maxBytes: 1_000, maxLines: 100 },
  themes: {}
} as EditorSettings;

function makeOverlay(overrides: Partial<FileOverlay> = {}): FileOverlay {
  return {
    mode: 'range',
    file: {
      path: 'src/app.ts', status: 'modified', additions: 1, deletions: 1,
      isBinary: false, isPreviewable: true
    },
    range: { branch: 'main', baseCommit: 'base-commit', headCommit: 'head-commit' },
    rows: [],
    blocks: [{
      id: 'block-1', oldStart: 1, oldEnd: 1, newStart: 1, newEnd: 1,
      changeType: 'modified', authors: [], rows: [], relatedCommits: []
    }],
    warnings: [], oldContent: 'old', newContent: 'new', resolvedEncoding: 'utf-8',
    ...overrides
  };
}

function mountViewer(overlay = makeOverlay(), overrides: Record<string, unknown> = {}) {
  return mount(DiffViewer, {
    props: {
      overlay,
      settings,
      themeName: 'revier-dark',
      requestedEncoding: 'auto',
      contextKey: 'range:file',
      ...overrides
    },
    global: {
      stubs: {
        'n-spin': { template: '<div><slot /></div>' },
        'n-empty': true,
        'n-alert': { template: '<div><slot /></div>' }
      }
    }
  });
}

describe('DiffViewer', () => {
  it('顶部只显示路径、范围和块数，正常小文件组合 Monaco、作者轨与状态栏', () => {
    const wrapper = mountViewer();
    const header = wrapper.get('.diff-viewer__header');
    expect(header.text()).toContain('src/app.ts');
    expect(header.text()).toContain('base-com..head-com');
    expect(header.text()).toContain('1 个变更块');
    expect(header.text()).not.toContain('TypeScript');
    expect(header.text()).not.toContain('Minimap');
    expect(wrapper.find('[data-testid="surface"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="author-rail"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="status"]').exists()).toBe(true);
  });

  it.each([
    ['字节', { oldContent: 'a'.repeat(1_001), newContent: '' }],
    ['行数', { oldContent: '', newContent: `${'x\n'.repeat(100)}x` }]
  ])('超过%s限制时延迟创建 Monaco，仍然加载后进入原始态', async (_name, content) => {
    const wrapper = mountViewer(makeOverlay(content));
    expect(wrapper.find('[data-testid="surface"]').exists()).toBe(false);
    expect(wrapper.text()).toContain('大文件');
    expect(wrapper.text()).toContain('src/app.ts');
    await wrapper.get('[data-testid="load-large-file"]').trigger('click');
    expect(wrapper.find('[data-testid="surface"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="status"]').attributes('data-mode')).toBe('original');
  });

  it('取消大文件加载后仍保留路径、字节数和行数且不创建 Monaco', async () => {
    const wrapper = mountViewer(makeOverlay({ oldContent: '甲\n乙', newContent: 'a'.repeat(1_001) }));
    await wrapper.get('[data-testid="cancel-large-file"]').trigger('click');
    expect(wrapper.text()).toContain('src/app.ts');
    expect(wrapper.text()).toContain('1001 字节');
    expect(wrapper.text()).toContain('2 行');
    expect(wrapper.find('[data-testid="surface"]').exists()).toBe(false);
  });

  it('首次编辑进入草稿，隐藏归因轨、清除真实选中并转发状态', async () => {
    const wrapper = mountViewer(makeOverlay(), { selectedBlockId: 'block-1' });
    const surface = wrapper.findComponent(surfaceStub);
    expect(surface.props('selectedBlock')).toEqual(makeOverlay().blocks[0]);
    surface.vm.$emit('draftChange', true);
    await wrapper.vm.$nextTick();

    expect(wrapper.get('[data-testid="status"]').attributes('data-mode')).toBe('draft');
    expect(wrapper.find('[data-testid="author-rail"]').exists()).toBe(false);
    expect(wrapper.findComponent(surfaceStub).props('selectedBlock')).toBeUndefined();
    expect(wrapper.emitted('draftChange')).toEqual([[true]]);
    expect(wrapper.text()).toContain('恢复原始内容');
  });

  it('恢复原始内容通过 session key 重建编辑器并转发状态', async () => {
    const wrapper = mountViewer();
    wrapper.findComponent(surfaceStub).vm.$emit('draftChange', true);
    await wrapper.vm.$nextTick();
    const draftSurface = wrapper.get('[data-testid="surface"]').element;

    await wrapper.get('[data-testid="restore-original"]').trigger('click');
    expect(wrapper.get('[data-testid="surface"]').element).not.toBe(draftSurface);
    expect(wrapper.emitted('draftChange')).toEqual([[true], [false]]);
    expect(wrapper.find('[data-testid="author-rail"]').exists()).toBe(true);
  });

  it('手动语言只作用当前 overlay，文件路径变化恢复自动识别', async () => {
    const wrapper = mountViewer();
    expect(wrapper.get('[data-testid="surface"]').attributes('data-language')).toBe('typescript');
    wrapper.findComponent(statusStub).vm.$emit('languageChange', 'rust');
    await wrapper.vm.$nextTick();
    expect(wrapper.get('[data-testid="surface"]').attributes('data-language')).toBe('rust');

    await wrapper.setProps({
      overlay: makeOverlay({ file: { ...makeOverlay().file, path: 'src/main.py' } })
    });
    expect(wrapper.get('[data-testid="surface"]').attributes('data-language')).toBe('python');
  });
});
