import { defineComponent, h, nextTick } from 'vue';
import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { editor } from 'monaco-editor';
import type { DiffBlock } from '../../src/renderer/generated/bindings';
import DiffAuthorRail from '../../src/renderer/components/review/DiffAuthorRail.vue';

vi.mock('monaco-editor', () => ({
  editor: { EditorOption: { lineHeight: 67 } }
}));

const NPopoverStub = defineComponent({
  name: 'NPopover',
  inheritAttrs: false,
  props: { show: Boolean, placement: String },
  emits: ['update:show', 'clickoutside'],
  setup(props, { attrs, emit, slots }) {
    return () => h('div', { class: 'popover-stub', ...attrs, 'data-placement': props.placement }, [
      slots.trigger?.(),
      props.show ? h('div', { class: 'popover-content' }, [
        slots.default?.(),
        h('button', { class: 'popover-clickoutside', onClick: () => emit('clickoutside') }, '模拟外部点击')
      ]) : undefined
    ]);
  }
});

function diffBlock(id = 'block-1'): DiffBlock {
  return {
    id,
    oldStart: 2,
    oldEnd: 4,
    newStart: 2,
    newEnd: 4,
    changeType: 'modified',
    authors: [
      { name: '低频', email: 'low@example.com', commitCount: 1, lastCommittedAt: '' },
      { name: '高频', email: 'high@example.com', commitCount: 3, lastCommittedAt: '2026-07-01T08:00:00Z' },
      { name: '次高频', commitCount: 2, lastCommittedAt: '2026-06-01T08:00:00Z' },
      { name: '时间异常', commitCount: 1, lastCommittedAt: 'not-a-date' }
    ],
    rows: [],
    relatedCommits: []
  };
}

type ListenerName = 'scroll' | 'layout' | 'hidden';

function mockEditor() {
  const listeners: Record<ListenerName, Array<() => void>> = { scroll: [], layout: [], hidden: [] };
  const disposes: Array<ReturnType<typeof vi.fn>> = [];
  const subscribe = (name: ListenerName, callback: () => void) => {
    listeners[name].push(callback);
    const dispose = vi.fn();
    disposes.push(dispose);
    return { dispose };
  };
  const getVisibleRanges = vi.fn(() => [{ startLineNumber: 1, endLineNumber: 20 }]);
  let layoutHeight = 200;
  const value = {
    getVisibleRanges,
    getTopForLineNumber: vi.fn((line: number) => (line - 1) * 20),
    getBottomForLineNumber: vi.fn((line: number) => line * 20),
    getScrolledVisiblePosition: vi.fn((position: { lineNumber: number }) => ({ top: (position.lineNumber - 1) * 20, left: 0, height: 20 })),
    getScrollTop: vi.fn(() => 0),
    getOption: vi.fn(() => 20),
    getLayoutInfo: vi.fn(() => ({ height: layoutHeight })),
    onDidScrollChange: vi.fn((callback: () => void) => subscribe('scroll', callback)),
    onDidLayoutChange: vi.fn((callback: () => void) => subscribe('layout', callback)),
    onDidChangeHiddenAreas: vi.fn((callback: () => void) => subscribe('hidden', callback))
  } as unknown as editor.ICodeEditor;
  return {
    value,
    listeners,
    disposes,
    getVisibleRanges,
    setLayoutHeight(height: number) { layoutHeight = height; }
  };
}

function mountRail(options: { draft?: boolean; blocks?: DiffBlock[]; original?: editor.ICodeEditor; modified?: editor.ICodeEditor } = {}) {
  const original = options.original ?? mockEditor().value;
  const modified = options.modified ?? mockEditor().value;
  return mount(DiffAuthorRail, {
    props: { blocks: options.blocks ?? [diffBlock()], draft: options.draft ?? false, originalEditor: original, modifiedEditor: modified },
    global: { stubs: { NPopover: NPopoverStub, Popover: NPopoverStub, 'n-popover': NPopoverStub } }
  });
}

describe('DiffAuthorRail', () => {
  let frames: FrameRequestCallback[];

  beforeEach(() => {
    frames = [];
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
  });

  it('渲染固定宽度、块几何与按容量排序后的作者', () => {
    const wrapper = mountRail();
    expect(wrapper.get('.diff-author-rail').attributes('style')).toContain('width: 112px');
    expect(wrapper.get('.diff-author-rail__block').attributes('style')).toContain('top: 20px');
    expect(wrapper.get('.diff-author-rail__block').attributes('style')).toContain('height: 60px');
    expect(wrapper.findAll('.diff-author-rail__author').map((item) => item.text())).toEqual(['高频']);
    expect(wrapper.find('.diff-author-rail__more').exists()).toBe(true);
  });

  it('点击块选择，点击省略号只打开向左的完整作者列表', async () => {
    const wrapper = mountRail();
    await wrapper.get('.diff-author-rail__block').trigger('click');
    expect(wrapper.emitted('selected')).toEqual([[expect.objectContaining({ id: 'block-1' })]]);

    await wrapper.get('.diff-author-rail__more').trigger('click');
    expect(wrapper.emitted('selected')).toHaveLength(1);
    expect(wrapper.get('.popover-stub').attributes('data-placement')).toBe('left');
    const content = wrapper.get('.diff-author-rail__popover');
    expect(content.attributes('style')).toContain('max-height: 320px');
    expect(content.text()).toContain('高频');
    expect(content.text()).toContain('high@example.com');
    expect(content.text()).toContain('3 次提交');
    expect(content.text()).toContain('2026-07-01T08:00:00Z');
    expect(content.text()).toContain('未知');
    expect(content.text()).not.toContain('not-a-date');
  });

  it('外部点击、普通块选择和删除当前块都会关闭 Popover', async () => {
    const second = { ...diffBlock('block-2'), oldStart: 6, oldEnd: 8, newStart: 6, newEnd: 8 };
    const wrapper = mountRail({ blocks: [diffBlock(), second] });

    await wrapper.findAll('.diff-author-rail__more')[0].trigger('click');
    expect(wrapper.find('.popover-content').exists()).toBe(true);
    await wrapper.get('.popover-clickoutside').trigger('click');
    expect(wrapper.find('.popover-content').exists()).toBe(false);

    await wrapper.findAll('.diff-author-rail__more')[0].trigger('click');
    await wrapper.findAll('.diff-author-rail__block')[1].trigger('click');
    expect(wrapper.find('.popover-content').exists()).toBe(false);

    await wrapper.findAll('.diff-author-rail__more')[0].trigger('click');
    await wrapper.setProps({ blocks: [second] });
    await wrapper.setProps({ blocks: [diffBlock(), second] });
    expect(wrapper.find('.popover-content').exists()).toBe(false);
  });

  it('作者块可由键盘聚焦和激活', async () => {
    const wrapper = mountRail();
    const authorBlock = wrapper.get('.diff-author-rail__block');
    expect(authorBlock.attributes('role')).toBe('button');
    expect(authorBlock.attributes('tabindex')).toBe('0');
    await authorBlock.trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('selected')).toHaveLength(1);
  });

  it('草稿态不渲染轨道且不订阅编辑器事件', () => {
    const original = mockEditor();
    const modified = mockEditor();
    const wrapper = mountRail({ draft: true, original: original.value, modified: modified.value });
    expect(wrapper.find('.diff-author-rail').exists()).toBe(false);
    expect(original.value.onDidScrollChange).not.toHaveBeenCalled();
    expect(modified.value.onDidScrollChange).not.toHaveBeenCalled();
  });

  it('同一帧合并两侧所有公开事件，编辑器变化时解绑旧监听并订阅新监听', async () => {
    const oldOriginal = mockEditor();
    const oldModified = mockEditor();
    const wrapper = mountRail({ original: oldOriginal.value, modified: oldModified.value });
    const callsAfterMount = oldModified.getVisibleRanges.mock.calls.length;
    oldOriginal.listeners.scroll[0]();
    oldOriginal.listeners.layout[0]();
    oldModified.listeners.hidden[0]();
    expect(requestAnimationFrame).toHaveBeenCalledTimes(1);

    frames.shift()?.(0);
    expect(oldModified.getVisibleRanges.mock.calls.length).toBe(callsAfterMount + 1);

    const nextOriginal = mockEditor();
    const nextModified = mockEditor();
    await wrapper.setProps({ originalEditor: nextOriginal.value, modifiedEditor: nextModified.value });
    expect(oldOriginal.disposes.every((dispose) => dispose.mock.calls.length === 1)).toBe(true);
    expect(oldModified.disposes.every((dispose) => dispose.mock.calls.length === 1)).toBe(true);
    expect(nextOriginal.value.onDidScrollChange).toHaveBeenCalledOnce();
    expect(nextModified.value.onDidChangeHiddenAreas).toHaveBeenCalledOnce();
  });

  it('布局数据变化后重算裁剪高度，草稿切换会解绑并按需重新订阅', async () => {
    const original = mockEditor();
    const modified = mockEditor();
    const wrapper = mountRail({ original: original.value, modified: modified.value });

    modified.setLayoutHeight(40);
    modified.listeners.layout[0]();
    frames.shift()?.(0);
    await nextTick();
    expect(wrapper.get('.diff-author-rail__block').attributes('style')).toContain('height: 20px');
    expect(wrapper.findAll('.diff-author-rail__author')).toHaveLength(0);
    expect(wrapper.find('.diff-author-rail__more').exists()).toBe(true);

    await wrapper.setProps({ draft: true });
    expect(wrapper.find('.diff-author-rail').exists()).toBe(false);
    expect([...original.disposes, ...modified.disposes].every((dispose) => dispose.mock.calls.length === 1)).toBe(true);

    await wrapper.setProps({ draft: false });
    expect(original.value.onDidScrollChange).toHaveBeenCalledTimes(2);
    expect(modified.value.onDidLayoutChange).toHaveBeenCalledTimes(2);
  });

  it('卸载时取消待执行帧并恰好释放全部监听', () => {
    const original = mockEditor();
    const modified = mockEditor();
    const wrapper = mountRail({ original: original.value, modified: modified.value });
    original.listeners.scroll[0]();
    wrapper.unmount();
    expect(cancelAnimationFrame).toHaveBeenCalledOnce();
    expect([...original.disposes, ...modified.disposes].every((dispose) => dispose.mock.calls.length === 1)).toBe(true);
  });

  it('同一编辑器实例只订阅一次，单个清理异常不阻断其余监听释放', async () => {
    const shared = mockEditor();
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const wrapper = mountRail({ original: shared.value, modified: shared.value });
    expect(shared.value.onDidScrollChange).toHaveBeenCalledOnce();
    expect(shared.value.onDidLayoutChange).toHaveBeenCalledOnce();
    expect(shared.value.onDidChangeHiddenAreas).toHaveBeenCalledOnce();

    shared.disposes[0].mockImplementation(() => { throw new Error('清理失败'); });
    await expect(wrapper.setProps({ draft: true })).resolves.toBeUndefined();
    expect(shared.disposes.every((dispose) => dispose.mock.calls.length === 1)).toBe(true);
    expect(error).toHaveBeenCalled();
    error.mockRestore();
  });
});
