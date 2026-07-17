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
  template: '<footer data-testid="status" :data-mode="mode" :data-language="languageId" :data-line="line" :data-column="column" />'
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

  it('同路径 contextKey 变化会销毁草稿并重置光标、编辑器引用和手动语言', async () => {
    const wrapper = mountViewer();
    const surface = wrapper.findComponent(surfaceStub);
    surface.vm.$emit('draftChange', true);
    surface.vm.$emit('cursorChange', 12, 8);
    surface.vm.$emit('editorsReady', { side: 'old' }, { side: 'new' });
    wrapper.findComponent(statusStub).vm.$emit('languageChange', 'rust');
    await wrapper.vm.$nextTick();
    const previousSurface = wrapper.get('[data-testid="surface"]').element;
    expect(wrapper.get('[data-testid="status"]').attributes()).toMatchObject({
      'data-mode': 'draft', 'data-language': 'rust', 'data-line': '12', 'data-column': '8'
    });

    await wrapper.setProps({ contextKey: 'range:file:next' });

    expect(wrapper.get('[data-testid="surface"]').element).not.toBe(previousSurface);
    expect(wrapper.get('[data-testid="status"]').attributes()).toMatchObject({
      'data-mode': 'original', 'data-language': 'typescript', 'data-line': '1', 'data-column': '1'
    });
    expect(wrapper.findComponent(railStub).props('originalEditor')).toBeUndefined();
    expect(wrapper.findComponent(railStub).props('modifiedEditor')).toBeUndefined();
    expect(wrapper.emitted('draftChange')).toEqual([[true], [false]]);
  });

  it('透传块选择、光标、编辑器实例和编码变更事件', async () => {
    const wrapper = mountViewer();
    const block = makeOverlay().blocks[0];
    const original = { side: 'old' };
    const modified = { side: 'new' };
    const surface = wrapper.findComponent(surfaceStub);

    surface.vm.$emit('selected', block);
    surface.vm.$emit('cursorChange', 7, 4);
    surface.vm.$emit('editorsReady', original, modified);
    wrapper.findComponent(statusStub).vm.$emit('encodingChange', 'gb18030');
    await wrapper.vm.$nextTick();

    expect(wrapper.emitted('selected')).toEqual([[block]]);
    expect(wrapper.emitted('cursorChange')).toEqual([[7, 4]]);
    expect(wrapper.emitted('editorsReady')).toEqual([[original, modified]]);
    expect(wrapper.emitted('encodingChange')).toEqual([['gb18030']]);
    expect(wrapper.findComponent(railStub).props()).toMatchObject({
      originalEditor: original,
      modifiedEditor: modified
    });
  });

  it('阈值恰等时直接加载，并按 UTF-8 字节、空文本和 CRLF 计算指标', () => {
    const exactSettings = {
      ...settings,
      largeFile: { maxBytes: 6, maxLines: 1 }
    } as EditorSettings;
    expect(mountViewer(
      makeOverlay({ oldContent: '你好', newContent: '' }),
      { settings: exactSettings }
    ).find('[data-testid="surface"]').exists()).toBe(true);

    const emptySettings = {
      ...settings,
      largeFile: { maxBytes: 0, maxLines: 0 }
    } as EditorSettings;
    expect(mountViewer(
      makeOverlay({ oldContent: '', newContent: '' }),
      { settings: emptySettings }
    ).find('[data-testid="surface"]').exists()).toBe(true);

    const unicode = mountViewer(
      makeOverlay({ oldContent: '你好', newContent: '' }),
      { settings: { ...settings, largeFile: { maxBytes: 5, maxLines: 100 } } }
    );
    expect(unicode.text()).toContain('6 字节');

    const crlf = mountViewer(
      makeOverlay({ oldContent: '甲\r\n乙', newContent: '' }),
      { settings: { ...settings, largeFile: { maxBytes: 1_000, maxLines: 1 } } }
    );
    expect(crlf.text()).toContain('2 行');
  });
});
