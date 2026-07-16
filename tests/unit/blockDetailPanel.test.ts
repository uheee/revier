// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import BlockDetailPanel from '../../src/renderer/components/review/BlockDetailPanel.vue';
import type { DiffBlock } from '../../src/renderer/generated/bindings';

const block: DiffBlock = {
  id: 'block-1',
  oldStart: 1,
  oldEnd: 1,
  newStart: 1,
  newEnd: 1,
  rowStartIndex: 0,
  rowEndIndex: 0,
  changeType: 'modified',
  authors: [
    {
      name: 'Alice',
      email: 'alice@example.com',
      commitCount: 1,
      lastCommittedAt: '2026-05-10T00:00:00.000Z'
    }
  ],
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
  it('在详情标题右端渲染唯一通知入口', () => {
    const wrapper = mount(BlockDetailPanel, {
      global: {
        stubs: {
          NotificationCenter: {
            name: 'NotificationCenter',
            template: '<button data-test="notification-center">通知</button>'
          },
          'n-alert': true,
          'n-empty': true,
          'n-tag': true,
          'n-tooltip': true
        }
      }
    });

    const heading = wrapper.get('.section-heading');
    const children = Array.from(heading.element.children);
    expect(wrapper.findAll('[data-test="notification-center"]')).toHaveLength(1);
    expect(children[0]?.tagName).toBe('H2');
    expect((children[1] as HTMLElement | undefined)?.dataset.test).toBe('notification-center');
  });

  it('emits selected commit for drilldown', async () => {
    const wrapper = mount(BlockDetailPanel, {
      props: { block, selectedCommitHash: undefined },
      global: {
        stubs: {
          NotificationCenter: true,
          'n-alert': true,
          'n-empty': true,
          'n-tag': { template: '<span><slot /></span>' },
          'n-tooltip': { template: '<span><slot /></span>' }
        }
      }
    });

    await wrapper.get('[data-commit-hash="abc123"]').trigger('click');
    expect(wrapper.emitted('commitSelected')?.[0]?.[0]).toEqual(block.relatedCommits[0]);
  });

  it('在提交下钻加载中显示行内取消按钮并移除下方取消按钮', async () => {
    const wrapper = mount(BlockDetailPanel, {
      props: { block, selectedCommitHash: 'abc123', activeCommitHash: 'abc123' },
      global: {
        stubs: {
          NotificationCenter: true,
          'n-alert': true,
          'n-empty': true,
          'n-tag': { template: '<span><slot /></span>' },
          'n-tooltip': { template: '<span><slot /></span>' }
        }
      }
    });

    const row = wrapper.get('[data-commit-hash="abc123"]');
    expect(row.classes()).toContain('is-selected');
    expect(wrapper.find('.commit-row__cancel').exists()).toBe(true);
    expect(wrapper.find('.commit-list__cancel').exists()).toBe(false);

    await wrapper.get('.commit-row__cancel').trigger('click');
    expect(wrapper.emitted('cancelCommit')).toHaveLength(1);
    expect(wrapper.emitted('commitSelected')).toBeUndefined();
  });
});
