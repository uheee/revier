<script setup lang="ts">
import type { ReviewProject } from '../../../shared/projectTypes';

defineProps<{
  projects: ReviewProject[];
  loading?: boolean;
}>();

const emit = defineEmits<{
  open: [projectId: string];
  remove: [projectId: string];
}>();

function formatDate(value?: string): string {
  if (!value) {
    return '-';
  }

  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  }).format(new Date(value));
}
</script>

<template>
  <section class="project-list">
    <header class="panel-heading project-list__heading">
      <h2>项目列表</h2>
    </header>

    <el-table
      v-loading="loading"
      class="project-table"
      :data="projects"
      height="100%"
      table-layout="fixed"
      empty-text="暂无项目"
    >
      <el-table-column label="项目" min-width="170">
        <template #default="{ row }: { row: ReviewProject }">
          <div class="project-name-cell">
            <span v-if="row.pinned" class="pin-dot" aria-label="已固定" />
            <span>{{ row.name }}</span>
          </div>
        </template>
      </el-table-column>

      <el-table-column label="仓库路径" min-width="300">
        <template #default="{ row }: { row: ReviewProject }">
          <span class="path-cell">{{ row.repoPath }}</span>
        </template>
      </el-table-column>

      <el-table-column label="默认分支" width="120">
        <template #default="{ row }: { row: ReviewProject }">
          {{ row.preferences.defaultBranch || '-' }}
        </template>
      </el-table-column>

      <el-table-column label="默认天数" width="110">
        <template #default="{ row }: { row: ReviewProject }">
          {{ row.preferences.defaultDays ?? '-' }}
        </template>
      </el-table-column>

      <el-table-column label="最近打开" width="170">
        <template #default="{ row }: { row: ReviewProject }">
          {{ formatDate(row.lastOpenedAt) }}
        </template>
      </el-table-column>

      <el-table-column label="操作" width="150" fixed="right">
        <template #default="{ row }: { row: ReviewProject }">
          <el-button link type="primary" @click="emit('open', row.id)">打开</el-button>
          <el-popconfirm title="移除这个项目记录？" @confirm="emit('remove', row.id)">
            <template #reference>
              <el-button link type="danger">移除</el-button>
            </template>
          </el-popconfirm>
        </template>
      </el-table-column>
    </el-table>
  </section>
</template>
