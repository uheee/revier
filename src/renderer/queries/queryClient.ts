import {
  PiniaColadaQueryHooksPlugin,
  type PiniaColadaPlugin,
  type UseQueryOptionsGlobal,
  type UseQueryEntry
} from '@pinia/colada';
import { toErrorMessage } from '../api/errors';
import { logError } from '../api/logger';
import { addNotification } from '../composables/useNotifications';

export interface RevierQueryMeta {
  title: string;
  source: string;
  context?: string;
  notify?: boolean;
}

export const revierQueryDefaults: UseQueryOptionsGlobal = {
  staleTime: Number.POSITIVE_INFINITY,
  gcTime: 5 * 60 * 1000,
  refetchOnMount: false,
  refetchOnWindowFocus: false,
  refetchOnReconnect: false
};

export function createRevierQueryHooksPlugin(): PiniaColadaPlugin {
  return PiniaColadaQueryHooksPlugin({
    onError(error, entry) {
      const meta = entry.meta as Partial<RevierQueryMeta> | undefined;
      logError(meta?.title ?? '普通查询失败', error, {
        source: meta?.source ?? 'Query',
        queryKey: entry.key.join(':')
      });
      if (meta?.notify === false || !meta?.title) {
        return;
      }

      const message = meta.context
        ? `${meta.context}：${toErrorMessage(error)}`
        : toErrorMessage(error);
      addNotification({
        type: 'error',
        title: meta.title,
        message,
        source: meta.source ?? 'Query'
      });
    }
  });
}

export function queryMeta(entry: UseQueryEntry): RevierQueryMeta | undefined {
  return entry.meta as unknown as RevierQueryMeta | undefined;
}
