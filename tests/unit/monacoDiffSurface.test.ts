// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import type { DiffBlock, EditorSettings } from '../../src/renderer/generated/bindings';

const createSession = vi.hoisted(() => vi.fn());
vi.mock('../../src/renderer/editor/monacoDiffSession', () => ({
  createMonacoDiffSession: createSession
}));

import MonacoDiffSurface from '../../src/renderer/components/review/MonacoDiffSurface.vue';

const resize = vi.hoisted(() => ({
  callback: undefined as ResizeObserverCallback | undefined,
  disconnect: vi.fn(),
  observe: vi.fn()
}));
class ResizeObserverMock {
  observe = resize.observe;
  disconnect = resize.disconnect;
  constructor(callback: ResizeObserverCallback) { resize.callback = callback; }
}

const settings = {
  version: 1, theme: 'system', defaultEncoding: 'auto',
  editor: { fontFamilies: ['monospace'], fontSize: 13, lineHeight: 22, minimap: true },
  largeFile: { maxBytes: 100, maxLines: 100 }, themes: {}
} as EditorSettings;
const block = {
  id: 'b1', oldStart: 1, oldEnd: 1, newStart: 1, newEnd: 1,
  changeType: 'modified', authors: [], rows: [], relatedCommits: []
} as DiffBlock;

function sessionMock() {
  return {
    diffEditor: {}, originalEditor: {}, modifiedEditor: {},
    setLanguage: vi.fn(), setTheme: vi.fn(), setSelectedBlock: vi.fn(),
    layout: vi.fn(), dispose: vi.fn()
  };
}

function mountSurface(overrides: Record<string, unknown> = {}) {
  return mount(MonacoDiffSurface, {
    props: {
      path: 'src/app.ts', oldContent: 'old', newContent: 'new', contextKey: 'file-a',
      languageId: 'typescript', settings, themeName: 'revier-dark', blocks: [block],
      ...overrides
    }
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  resize.callback = undefined;
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
  createSession.mockImplementation(() => sessionMock());
});

afterEach(() => vi.unstubAllGlobals());

describe('MonacoDiffSurface', () => {
  it('挂载一次并精确转发会话事件', () => {
    const wrapper = mountSurface();
    expect(createSession).toHaveBeenCalledTimes(1);
    const options = createSession.mock.calls[0][0];
    options.onDraftChange(true);
    options.onBlockSelected(block);
    options.onCursorChange(4, 9);
    options.onEditorsReady('old-editor', 'new-editor');
    expect(wrapper.emitted('draftChange')).toEqual([[true]]);
    expect(wrapper.emitted('selected')).toEqual([[block]]);
    expect(wrapper.emitted('cursorChange')).toEqual([[4, 9]]);
    expect(wrapper.emitted('editorsReady')).toEqual([['old-editor', 'new-editor']]);
  });

  it('内容或上下文变化先销毁再重建，语言主题和选中块只调用 setter', async () => {
    const first = sessionMock();
    const second = sessionMock();
    const order: string[] = [];
    first.dispose.mockImplementation(() => order.push('dispose'));
    createSession
      .mockReturnValueOnce(first)
      .mockImplementation(() => { order.push('create'); return second; });
    const wrapper = mountSurface();
    await wrapper.setProps({ languageId: 'rust' });
    await wrapper.setProps({ themeName: 'revier-light' });
    await wrapper.setProps({ selectedBlock: block });
    expect(first.setLanguage).toHaveBeenCalledWith('rust');
    expect(first.setTheme).toHaveBeenCalledWith('revier-light');
    expect(first.setSelectedBlock).toHaveBeenCalledWith(block);
    expect(createSession).toHaveBeenCalledTimes(1);

    await wrapper.setProps({ oldContent: 'changed' });
    expect(order).toEqual(['dispose', 'create']);
    await wrapper.setProps({ contextKey: 'file-b' });
    expect(second.dispose).toHaveBeenCalledTimes(1);
    expect(createSession).toHaveBeenCalledTimes(3);
  });

  it('块引用变化触发重建，同一 tick 的多项上下文变化只重建一次', async () => {
    const first = sessionMock();
    const second = sessionMock();
    const order: string[] = [];
    first.dispose.mockImplementation(() => order.push('dispose'));
    createSession
      .mockReturnValueOnce(first)
      .mockImplementation(() => { order.push('create'); return second; });
    const wrapper = mountSurface();
    const nextBlock = { ...block, id: 'b2' };

    await wrapper.setProps({ blocks: [nextBlock] });
    expect(order).toEqual(['dispose', 'create']);
    expect(createSession).toHaveBeenCalledTimes(2);

    order.length = 0;
    const third = sessionMock();
    second.dispose.mockImplementation(() => order.push('dispose'));
    createSession.mockImplementation(() => { order.push('create'); return third; });

    await wrapper.setProps({
      path: 'src/next.ts',
      oldContent: 'next old',
      newContent: 'next new',
      contextKey: 'file-next',
      blocks: [{ ...nextBlock }]
    });

    expect(order).toEqual(['dispose', 'create']);
    expect(createSession).toHaveBeenCalledTimes(3);
    expect(createSession.mock.calls[2][0]).toEqual(expect.objectContaining({
      path: 'src/next.ts',
      oldContent: 'next old',
      newContent: 'next new',
      blocks: [{ ...nextBlock }]
    }));
  });

  it('尺寸变化触发布局，卸载时断开观察并销毁', () => {
    const session = sessionMock();
    createSession.mockReturnValue(session);
    const wrapper = mountSurface();
    resize.callback?.([], {} as ResizeObserver);
    expect(session.layout).toHaveBeenCalledTimes(1);
    wrapper.unmount();
    expect(session.dispose).toHaveBeenCalledTimes(1);
    expect(resize.disconnect).toHaveBeenCalledTimes(1);
  });

  it('初始化失败显示错误，重试前清理并可恢复', async () => {
    const half = sessionMock();
    const recovered = sessionMock();
    createSession
      .mockImplementationOnce(() => { throw new Error('worker failed'); })
      .mockReturnValueOnce(recovered);
    const wrapper = mountSurface();
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain('编辑器加载失败');
    expect(wrapper.get('button').text()).toContain('重试加载');
    await wrapper.get('button').trigger('click');
    expect(createSession).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).not.toContain('编辑器加载失败');
    expect(half.dispose).not.toHaveBeenCalled();
  });
});
