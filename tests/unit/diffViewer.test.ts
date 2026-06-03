// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import DiffViewer from '../../src/renderer/components/review/DiffViewer.vue';
import type { FileOverlay } from '../../src/shared/reviewTypes';

const overlay: FileOverlay = {
  mode: 'range',
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
  rows: [
    {
      oldLineNumber: 1,
      newLineNumber: 1,
      oldText: 'same',
      newText: 'same',
      type: 'context'
    },
    {
      oldLineNumber: 2,
      newLineNumber: 2,
      oldText: 'old',
      newText: 'new',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 3,
      newLineNumber: 3,
      oldText: 'after',
      newText: 'after',
      type: 'context'
    }
  ],
  blocks: [
    {
      id: 'block-1',
      oldStart: 2,
      oldEnd: 2,
      newStart: 2,
      newEnd: 2,
      rowStartIndex: 1,
      rowEndIndex: 1,
      changeType: 'modified',
      authors: [{ name: 'Alice', email: 'alice@example.com' }],
      rows: [],
      relatedCommits: []
    }
  ],
  warnings: []
};

describe('DiffViewer', () => {
  it('renders full file rows and emits selected block when a changed row is clicked', async () => {
    const wrapper = mount(DiffViewer, {
      props: { overlay },
      global: {
        directives: {
          loading: {}
        },
        stubs: {
          'el-empty': true,
          'el-alert': true,
          'el-tag': { template: '<span><slot /></span>' },
          'el-tooltip': { template: '<span><slot /></span>' }
        }
      }
    });

    expect(wrapper.text()).toContain('same');
    expect(wrapper.text()).toContain('after');

    await wrapper.get('[data-block-id="block-1"]').trigger('click');
    expect(wrapper.emitted('selected')?.[0]?.[0]).toEqual(overlay.blocks[0]);
  });
});
