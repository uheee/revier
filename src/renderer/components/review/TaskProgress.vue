<script setup lang="ts">
import type { AnalysisTaskSnapshot } from '../../../shared/reviewTypes';

const props = defineProps<{
  task?: AnalysisTaskSnapshot;
  loading?: boolean;
  error?: string;
}>();

function statusText(task?: AnalysisTaskSnapshot): string {
  if (!task) {
    return '未开始';
  }

  const statuses: Record<AnalysisTaskSnapshot['status'], string> = {
    pending: '等待中',
    running: '运行中',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消'
  };
  return statuses[task.status];
}

function stageText(task?: AnalysisTaskSnapshot): string {
  if (!task) {
    return '-';
  }

  const stages: Record<AnalysisTaskSnapshot['stage'], string> = {
    readRepository: '读取仓库',
    resolveRange: '解析范围',
    loadCommits: '加载提交',
    loadChangedFiles: '加载文件',
    ready: '可浏览'
  };
  return stages[task.stage];
}
</script>

<template>
  <section class="task-progress">
    <header class="section-heading">
      <h2>任务</h2>
    </header>

    <el-alert v-if="error" :title="error" type="error" show-icon :closable="false" />
    <template v-else>
      <div class="task-progress__row">
        <span>状态</span>
        <strong>{{ statusText(task) }}</strong>
      </div>
      <div class="task-progress__row">
        <span>阶段</span>
        <strong>{{ stageText(task) }}</strong>
      </div>
      <el-progress
        v-if="loading || task"
        :percentage="Math.round((task?.progress ?? (loading ? 0.35 : 0)) * 100)"
        :show-text="false"
      />
      <p v-if="task?.message" class="task-progress__message">{{ task.message }}</p>
    </template>
  </section>
</template>
