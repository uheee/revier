// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import BlockDetailPanel from '../../src/renderer/components/review/BlockDetailPanel.vue';
import type { DiffBlock } from '../../src/shared/reviewTypes';

const block: DiffBlock = {
  id: 'block-1',
  oldStart: 1,
  oldEnd: 1,
  newStart: 1,
  newEnd: 1,
  rowStartIndex: 0,
  rowEndIndex: 0,
  changeType: 'modified',
  authors: [{ name: 'Alice', email: 'alice@example.com' }],
  rows: [],
  relatedCommits: [
    {
      hash: 'abc123',
      shortHash: 'abc123',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      committedAt: '2026-05-10T00:00:00.000Z',
      subject: 'feature: update app',
      matchedByFilter: true,
      touchedRanges: []
    }
  ]
};

describe('BlockDetailPanel', () => {
  it('emits selected commit for drilldown', async () => {
    const wrapper = mount(BlockDetailPanel, {
      props: { block, selectedCommitHash: undefined },
      global: {
        stubs: {
          'el-empty': true,
          'el-tag': { template: '<span><slot /></span>' },
          'el-tooltip': { template: '<span><slot /></span>' }
        }
      }
    });

    await wrapper.get('[data-commit-hash="abc123"]').trigger('click');
    expect(wrapper.emitted('commitSelected')?.[0]?.[0]).toEqual(block.relatedCommits[0]);
  });
});
