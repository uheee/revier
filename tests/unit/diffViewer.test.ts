// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import DiffViewer from '../../src/renderer/components/review/DiffViewer.vue';
import type { FileOverlay } from '../../src/shared/reviewTypes';

const overlay: FileOverlay = {
  mode: 'range',
  file: {
    path: 'src/app.ts',
    status: 'modified',
    additions: 3,
    deletions: 3,
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
      oldText: 'old-1',
      newText: 'new-1',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 3,
      newLineNumber: 3,
      oldText: 'old-2',
      newText: 'new-2',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 4,
      newLineNumber: 4,
      oldText: 'old-3',
      newText: 'new-3',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 5,
      newLineNumber: 5,
      oldText: 'after',
      newText: 'after',
      type: 'context'
    }
  ],
  blocks: [
    {
      id: 'block-1',
      oldStart: 2,
      oldEnd: 4,
      newStart: 2,
      newEnd: 4,
      rowStartIndex: 1,
      rowEndIndex: 3,
      changeType: 'modified',
      authors: [{ name: 'Alice', email: 'alice@example.com' }],
      rows: [],
      relatedCommits: []
    }
  ],
  warnings: []
};

type DiffViewerTestProps = {
  overlay: FileOverlay;
  selectedBlockId?: string;
  loading?: boolean;
};

function mountDiffViewer(props: DiffViewerTestProps) {
  return mount(DiffViewer, {
    props,
    global: {
      directives: {
        loading: {}
      },
      stubs: {
        'n-spin': { template: '<div><slot /></div>' },
        'n-empty': true,
        'n-alert': true,
        'n-tag': { template: '<span><slot /></span>' },
        'n-tooltip': { template: '<span><slot /></span>' }
      }
    }
  });
}

describe('DiffViewer', () => {
  it('渲染完整文件行并在点击变更行时发出 selected 事件', async () => {
    const wrapper = mountDiffViewer({ overlay });

    expect(wrapper.text()).toContain('same');
    expect(wrapper.text()).toContain('after');

    await wrapper.get('[data-block-id="block-1"]').trigger('click');
    expect(wrapper.emitted('selected')?.[0]?.[0]).toEqual(overlay.blocks[0]);
  });

  it('为选中的多行变更块标记块首和块尾 class', () => {
    const wrapper = mountDiffViewer({ overlay, selectedBlockId: 'block-1' });

    const selectedRows = wrapper.findAll('[data-block-id="block-1"]');

    expect(selectedRows).toHaveLength(3);
    expect(selectedRows.every((row) => row.classes('is-selected'))).toBe(true);
    expect(selectedRows[0].classes()).toContain('is-block-start');
    expect(selectedRows[0].classes()).not.toContain('is-block-end');
    expect(selectedRows[1].classes()).not.toContain('is-block-start');
    expect(selectedRows[1].classes()).not.toContain('is-block-end');
    expect(selectedRows[2].classes()).toContain('is-block-end');
    expect(selectedRows[2].classes()).not.toContain('is-block-start');
  });
});
