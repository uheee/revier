// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import OperationStatusBar from '../../src/renderer/components/review/OperationStatusBar.vue';
import type { OperationProgressSnapshot } from '../../src/renderer/generated/bindings';

describe('OperationStatusBar', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-07-22T00:00:01.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('运行期间持续刷新耗时，完成后保留本次耗时', async () => {
    const operation: OperationProgressSnapshot = {
      operationId: 'operation-1',
      kind: 'project-analysis',
      status: 'running',
      projectId: 'project-1',
      branch: 'develop',
      stage: 'index-commits',
      message: '正在更新索引',
      completedUnits: 128,
      totalUnits: 420,
      startedAt: '2026-07-22T00:00:00.000Z',
      elapsedMs: 900,
      cacheState: 'refresh'
    };
    const wrapper = mount(OperationStatusBar, {
      props: { operation }
    });

    expect(wrapper.text()).toContain('128/420');
    expect(wrapper.text()).toContain('已用 1.0 秒');
    expect(wrapper.find('progress').attributes('value')).toBe(String(128 / 420));
    await vi.advanceTimersByTimeAsync(600);
    expect(wrapper.text()).toContain('已用 1.6 秒');

    await wrapper.setProps({
      operation: {
        ...operation,
        status: 'completed',
        stage: 'ready',
        message: '项目分析完成',
        elapsedMs: 1632
      }
    });
    expect(wrapper.text()).toContain('项目分析完成');
    expect(wrapper.text()).toContain('本次耗时 1.6 秒');
  });

  it('未知总量时不伪造进度条，并展示目标、缓存状态和提交短哈希', () => {
    const wrapper = mount(OperationStatusBar, {
      props: {
        operation: {
          operationId: 'operation-2',
          kind: 'commit-overlay',
          status: 'running',
          projectId: 'project-1',
          branch: 'develop',
          filePath: 'src/main.ts',
          commitHash: '1234567890abcdef',
          stage: 'read-file-content',
          message: '正在打开提交',
          startedAt: '2026-07-22T00:00:00.000Z',
          elapsedMs: 1000,
          cacheState: 'hit'
        }
      }
    });

    expect(wrapper.find('progress').exists()).toBe(false);
    expect(wrapper.text()).toContain('develop · src/main.ts · 12345678');
    expect(wrapper.text()).toContain('缓存命中');
  });

  it('耗时刷新不会改变读屏播报内容', async () => {
    const operation: OperationProgressSnapshot = {
      operationId: 'operation-3',
      kind: 'monaco-diff',
      status: 'running',
      projectId: 'project-1',
      filePath: 'src/main.ts',
      stage: 'compute-diff',
      message: '正在计算差异',
      startedAt: '2026-07-22T00:00:00.000Z',
      elapsedMs: 1000,
      cacheState: 'none'
    };
    const wrapper = mount(OperationStatusBar, { props: { operation } });
    const announcement = wrapper.get('[role="status"]');
    const initialAnnouncement = announcement.text();

    await vi.advanceTimersByTimeAsync(500);

    expect(announcement.text()).toBe(initialAnnouncement);
    expect(wrapper.text()).toContain('已用 1.5 秒');
  });
});
