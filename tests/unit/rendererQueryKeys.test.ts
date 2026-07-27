import { projectQueryKeys } from '../../src/renderer/queries/projectQueries';
import { revierQueryDefaults } from '../../src/renderer/queries/queryClient';

describe('renderer query keys', () => {
  it('项目列表使用稳定全局 key', () => {
    expect(projectQueryKeys.projects).toEqual(['projects']);
  });

  it('分支和缓存状态 key 包含项目与分支上下文', () => {
    expect(projectQueryKeys.branches('project-1')).toEqual([
      'projects',
      'project-1',
      'branches'
    ]);
    expect(projectQueryKeys.branchCacheStatus('project-1', 'develop')).toEqual([
      'review',
      'project-1',
      'develop',
      'cache-status'
    ]);
  });

  it('作者 key 包含所有影响查询结果的筛选字段', () => {
    expect(projectQueryKeys.authors({
      projectId: 'project-1',
      branch: 'develop',
      startAt: '2026-07-01T00:00:00.000Z',
      endAt: '2026-07-28T00:00:00.000Z'
    })).toEqual([
      'review',
      'project-1',
      'develop',
      'authors',
      '2026-07-01T00:00:00.000Z',
      '2026-07-28T00:00:00.000Z'
    ]);
  });

  it('本地 IPC 查询默认不重试、不后台刷新、不窗口聚焦刷新', () => {
    expect(revierQueryDefaults).toMatchObject({
      staleTime: Number.POSITIVE_INFINITY,
      refetchOnMount: false,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false
    });
  });
});
