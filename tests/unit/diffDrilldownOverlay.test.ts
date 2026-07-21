// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import { defineComponent, nextTick } from 'vue';
import DiffDrilldownOverlay from '../../src/renderer/components/review/DiffDrilldownOverlay.vue';
import type { FileOverlay } from '../../src/renderer/generated/bindings';

vi.mock('../../src/renderer/components/review/DiffViewer.vue', () => ({
  default: {
    name: 'DiffViewer',
    props: ['overlay', 'loading', 'settings', 'themeName', 'requestedEncoding', 'contextKey'],
    emits: ['draftChange', 'encodingChange'],
    template: '<button data-testid="diff-viewer-stub" @click="$emit(\'draftChange\', true)" @contextmenu.prevent="$emit(\'encodingChange\', \'gb18030\')" />'
  }
}));

const commitOverlay: FileOverlay = {
  mode: 'commit',
  file: {
    path: 'src/app.ts',
    status: 'modified',
    additions: 1,
    deletions: 1,
    isBinary: false,
    isPreviewable: true
  },
  range: {
    branch: 'main',
    baseCommit: 'base',
    headCommit: 'head'
  },
  rows: [],
  blocks: [],
  warnings: [],
  commit: {
    hash: 'abc123',
    shortHash: 'abc123',
    authorName: 'Alice',
    authorEmail: 'alice@example.com',
    committedAt: '2026-06-12T00:00:00.000Z',
    subject: 'fix: 修复下钻闪烁',
    matchedByFilter: false,
    touchedRanges: []
  },
  parentHash: 'parent',
  oldContent: 'old',
  newContent: 'new',
  resolvedEncoding: 'utf-8'
};

describe('DiffDrilldownOverlay', () => {
  it('keeps the drilldown container mounted when loading resolves to commit content', async () => {
    const Harness = defineComponent({
      components: { DiffDrilldownOverlay },
      data: () => ({
        loading: true,
        overlay: undefined as FileOverlay | undefined,
        settings: { defaultEncoding: 'auto' },
        themeName: 'revier-light',
        requestedEncoding: 'auto'
      }),
      template: '<DiffDrilldownOverlay :loading="loading" :overlay="overlay" :settings="settings" :theme-name="themeName" :requested-encoding="requestedEncoding" />'
    });

    const wrapper = mount(Harness, {
      global: {
        stubs: {
          'n-button': { template: '<button type="button"><slot /></button>' },
          'n-spin': { template: '<div><slot /></div>' },
          'n-empty': true,
          'n-alert': true,
          'n-tag': { template: '<span><slot /></span>' },
          'n-tooltip': { template: '<span><slot /></span>' }
        }
      }
    });

    const initialContainer = wrapper.get('.diff-drilldown').element;

    const vm = wrapper.vm as unknown as { loading: boolean; overlay?: FileOverlay };
    vm.loading = false;
    vm.overlay = commitOverlay;
    await nextTick();

    expect(wrapper.get('.diff-drilldown').element).toBe(initialContainer);
    expect(wrapper.text()).toContain('fix: 修复下钻闪烁');
  });

  it('向编辑器透传编码和提交上下文，并转发草稿与编码事件', async () => {
    const settings = { defaultEncoding: 'utf-8' } as never;
    const wrapper = mount(DiffDrilldownOverlay, {
      props: { overlay: commitOverlay, settings, themeName: 'revier-dark', requestedEncoding: 'utf-16le' },
      global: { stubs: {
        'n-button': { template: '<button><slot /></button>' },
        'n-alert': true
      } }
    });
    const viewer = wrapper.getComponent({ name: 'DiffViewer' });
    expect(viewer.props()).toMatchObject({
      settings,
      themeName: 'revier-dark',
      requestedEncoding: 'utf-16le',
      contextKey: 'abc123:parent'
    });
    await wrapper.get('[data-testid="diff-viewer-stub"]').trigger('click');
    await wrapper.get('[data-testid="diff-viewer-stub"]').trigger('contextmenu');
    expect(wrapper.emitted('draftChange')).toEqual([[true]]);
    expect(wrapper.emitted('encodingChange')).toEqual([['gb18030']]);
  });

  it('提交加载失败后仍保留下钻容器、提交摘要和返回入口', async () => {
    const wrapper = mount(DiffDrilldownOverlay, {
      props: {
        selectedCommitHash: commitOverlay.commit!.hash,
        selectedCommit: commitOverlay.commit,
        error: '该提交未修改当前文件',
        settings: { defaultEncoding: 'utf-8' } as never,
        themeName: 'revier-dark',
        requestedEncoding: 'utf-8'
      },
      global: {
        stubs: {
          'n-button': { template: '<button type="button"><slot /></button>' },
          'n-alert': { props: ['title'], template: '<div role="alert"><strong>{{ title }}</strong><slot /></div>' }
        }
      }
    });

    expect(wrapper.find('.diff-drilldown').exists()).toBe(true);
    expect(wrapper.text()).toContain('abc123');
    expect(wrapper.text()).toContain('fix: 修复下钻闪烁');
    expect(wrapper.text()).toContain('该提交未修改当前文件');
    expect(wrapper.findComponent({ name: 'DiffViewer' }).exists()).toBe(false);

    await wrapper.get('button').trigger('click');
    expect(wrapper.emitted('close')).toEqual([[]]);
  });
});
