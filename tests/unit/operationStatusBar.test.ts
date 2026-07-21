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
    expect(wrapper.text()).toContain('本次耗时 1.0 秒');
    await vi.advanceTimersByTimeAsync(600);
    expect(wrapper.text()).toContain('本次耗时 1.6 秒');

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
});
