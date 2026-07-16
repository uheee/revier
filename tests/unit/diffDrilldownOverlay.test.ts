// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import { defineComponent, nextTick } from 'vue';
import DiffDrilldownOverlay from '../../src/renderer/components/review/DiffDrilldownOverlay.vue';
import type { FileOverlay } from '../../src/renderer/generated/bindings';

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
        overlay: undefined as FileOverlay | undefined
      }),
      template: '<DiffDrilldownOverlay :loading="loading" :overlay="overlay" />'
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
});
