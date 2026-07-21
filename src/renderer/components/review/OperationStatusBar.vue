<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { OperationProgressSnapshot } from '../../generated/bindings';

const props = defineProps<{ operation?: OperationProgressSnapshot }>();
const now = ref(Date.now());
let timer: number | undefined;

watch(
  () => props.operation?.status,
  (status) => {
    if (timer !== undefined) window.clearInterval(timer);
    timer = undefined;
    now.value = Date.now();
    if (status === 'running') {
      timer = window.setInterval(() => { now.value = Date.now(); }, 100);
    }
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  if (timer !== undefined) window.clearInterval(timer);
});

const elapsedMs = computed(() => {
  const operation = props.operation;
  if (!operation || operation.status !== 'running') return operation?.elapsedMs ?? 0;
  const startedAt = Date.parse(operation.startedAt);
  const localElapsed = Number.isFinite(startedAt) ? Math.max(0, now.value - startedAt) : 0;
  return Math.max(operation.elapsedMs, localElapsed);
});

const progressText = computed(() => {
  const operation = props.operation;
  if (!operation) return '';
  if (operation.completedUnits !== undefined && operation.totalUnits !== undefined) {
    return `${operation.completedUnits}/${operation.totalUnits}`;
  }
  if (operation.progress !== undefined) return `${Math.round(operation.progress * 100)}%`;
  return '';
});

const elapsedText = computed(() =>
  elapsedMs.value < 1000
    ? `${elapsedMs.value} 毫秒`
    : `${(elapsedMs.value / 1000).toFixed(1)} 秒`
);
</script>

<template>
  <footer class="operation-status-bar" role="status" aria-live="polite">
    <template v-if="operation">
      <span class="operation-status-bar__state" :data-status="operation.status">
        {{ operation.message }}
      </span>
      <span v-if="operation.branch">{{ operation.branch }}</span>
      <span v-if="operation.filePath">{{ operation.filePath }}</span>
      <span v-if="progressText">{{ progressText }}</span>
      <span>本次耗时 {{ elapsedText }}</span>
    </template>
    <span v-else>就绪</span>
  </footer>
</template>

<style scoped>
.operation-status-bar {
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

.operation-status-bar__state {
  overflow: hidden;
  color: var(--foreground-color);
  text-overflow: ellipsis;
}

.operation-status-bar__state[data-status='failed'] {
  color: var(--error-color, #d03050);
}
</style>
