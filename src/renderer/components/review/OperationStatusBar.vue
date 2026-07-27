<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useIntervalFn } from '@vueuse/shared';
import type { OperationProgressSnapshot } from '../../generated/bindings';

const props = defineProps<{ operation?: OperationProgressSnapshot }>();
const now = ref(Date.now());
const { pause, resume } = useIntervalFn(() => {
  now.value = Date.now();
}, 100, { immediate: false });

watch(
  () => props.operation?.status,
  (status) => {
    pause();
    now.value = Date.now();
    if (status === 'running') {
      resume();
    }
  },
  { immediate: true }
);

const elapsedMs = computed(() => {
  const operation = props.operation;
  if (!operation || operation.status !== 'running') return operation?.elapsedMs ?? 0;
  const startedAt = Date.parse(operation.startedAt);
  const localElapsed = Number.isFinite(startedAt) ? Math.max(0, now.value - startedAt) : 0;
  return Math.max(operation.elapsedMs, localElapsed);
});

const normalizedProgress = computed(() => {
  const operation = props.operation;
  if (!operation) return undefined;
  if (operation.status === 'completed') return 1;
  if (operation.completedUnits !== undefined && operation.totalUnits !== undefined && operation.totalUnits > 0) {
    return Math.min(1, Math.max(0, operation.completedUnits / operation.totalUnits));
  }
  if (operation.progress !== undefined) return Math.min(1, Math.max(0, operation.progress));
  return undefined;
});

const progressText = computed(() => {
  const operation = props.operation;
  if (!operation) return '';
  if (operation.status === 'completed') return '100%';
  if (operation.completedUnits !== undefined && operation.totalUnits !== undefined) {
    return `${operation.completedUnits}/${operation.totalUnits}`;
  }
  if (normalizedProgress.value !== undefined) return `${Math.round(normalizedProgress.value * 100)}%`;
  return '';
});

const cacheText = computed(() => ({
  hit: '缓存命中',
  miss: '缓存未命中',
  stale: '缓存已过期',
  refresh: '强制刷新',
  none: ''
})[props.operation?.cacheState ?? 'none']);

const targetText = computed(() => {
  const operation = props.operation;
  if (!operation) return '';
  const parts = [operation.branch, operation.filePath];
  if (operation.commitHash) parts.push(operation.commitHash.slice(0, 8));
  return parts.filter(Boolean).join(' · ');
});

// 只在操作阶段或终态变化时播报，避免 100ms 耗时刷新持续打断读屏。
const announcement = computed(() => {
  const operation = props.operation;
  if (!operation) return '就绪';
  return [operation.message, targetText.value, progressText.value].filter(Boolean).join('，');
});

const elapsedText = computed(() =>
  elapsedMs.value < 1000
    ? `${elapsedMs.value} 毫秒`
    : `${(elapsedMs.value / 1000).toFixed(1)} 秒`
);
</script>

<template>
  <footer class="operation-status-bar">
    <template v-if="operation">
      <span class="operation-status-bar__primary">
        <span class="operation-status-bar__state" :data-status="operation.status">
          {{ operation.message }}
        </span>
        <span v-if="targetText" class="operation-status-bar__target" :title="targetText">{{ targetText }}</span>
      </span>
      <span v-if="normalizedProgress !== undefined" class="operation-status-bar__progress">
        <progress :value="normalizedProgress" max="1" aria-label="当前操作进度" />
        <span>{{ progressText }}</span>
      </span>
      <span v-if="cacheText" class="operation-status-bar__cache">{{ cacheText }}</span>
      <span class="operation-status-bar__elapsed">
        {{ operation.status === 'running' ? '已用' : '本次耗时' }} {{ elapsedText }}
      </span>
    </template>
    <span v-else>就绪</span>
    <span class="operation-status-bar__announcement" role="status" aria-live="polite" aria-atomic="true">
      {{ announcement }}
    </span>
  </footer>
</template>

<style scoped>
.operation-status-bar {
  position: relative;
  grid-column: 1 / -1;
  grid-row: 2;
  display: flex;
  gap: 12px;
  align-items: center;
  min-width: 0;
  height: 28px;
  padding: 0 12px;
  overflow: hidden;
  color: var(--muted-color);
  font-size: 12px;
  white-space: nowrap;
  background: var(--panel-background);
  border-top: 1px solid var(--border-color);
}

.operation-status-bar__primary {
  display: flex;
  flex: 1 1 auto;
  gap: 8px;
  min-width: 0;
  overflow: hidden;
}

.operation-status-bar__state {
  flex: 0 1 auto;
  overflow: hidden;
  color: var(--foreground-color);
  text-overflow: ellipsis;
}

.operation-status-bar__target {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.operation-status-bar__progress {
  display: flex;
  flex: 0 0 auto;
  gap: 6px;
  align-items: center;
}

.operation-status-bar__progress progress {
  width: clamp(48px, 10vw, 112px);
  height: 4px;
}

.operation-status-bar__cache,
.operation-status-bar__elapsed {
  flex: 0 0 auto;
}

.operation-status-bar__announcement {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

.operation-status-bar__state[data-status='failed'] {
  color: var(--error-color, #d03050);
}

@media (max-width: 760px) {
  .operation-status-bar__cache,
  .operation-status-bar__progress progress {
    display: none;
  }
}
</style>
