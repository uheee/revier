import { useQuery, useQueryCache, type EntryKey } from '@pinia/colada';
import { revierClient } from '../api/revierClient';
import type {
  ReviewAuthorOptionsRequest,
  ReviewProject
} from '../generated/bindings';
import { revierQueryDefaults } from './queryClient';

interface ProjectQueryKeys {
  projects: EntryKey;
  branches: (projectId: string) => EntryKey;
  branchCacheStatus: (projectId: string, branch: string) => EntryKey;
  authors: (request: ReviewAuthorOptionsRequest) => EntryKey;
}

export const projectQueryKeys = {
  projects: ['projects'] as const,
  branches: (projectId: string) => ['projects', projectId, 'branches'] as const,
  branchCacheStatus: (projectId: string, branch: string) =>
    ['review', projectId, branch, 'cache-status'] as const,
  authors: (request: ReviewAuthorOptionsRequest) =>
    [
      'review',
      request.projectId,
      request.branch,
      'authors',
      request.startAt ?? '',
      request.endAt ?? ''
    ] as const
} satisfies ProjectQueryKeys;

export function useProjectListQuery() {
  const queryCache = useQueryCache();
  const query = useQuery<ReviewProject[]>({
    key: projectQueryKeys.projects,
    query: () => revierClient.projects.list(),
    ...revierQueryDefaults,
    meta: {
      title: '项目列表加载失败',
      source: 'Projects'
    }
  });

  return {
    ...query,
    invalidate: () =>
      queryCache.invalidateQueries({ key: projectQueryKeys.projects, exact: true }, false)
  };
}
