<script setup lang="ts">
import type { ChangedFile } from '../../../shared/reviewTypes';

defineProps<{
  files: ChangedFile[];
  selectedPath?: string;
}>();

const emit = defineEmits<{
  selected: [path: string];
}>();

const statusLabels: Record<ChangedFile['status'], string> = {
  added: '新增',
  modified: '修改',
  deleted: '删除',
  renamed: '重命名',
  binary: '二进制'
};
</script>

<template>
  <section class="changed-file-list">
    <header class="section-heading">
      <h2>变更文件</h2>
    </header>

    <n-empty v-if="files.length === 0" size="small" description="暂无文件" />
    <div v-else class="changed-file-list__items">
      <button
        v-for="file in files"
        :key="file.path"
        class="changed-file-row"
        :class="{ 'is-selected': file.path === selectedPath }"
        type="button"
        @click="emit('selected', file.path)"
      >
        <span class="changed-file-row__path">
          <template v-if="file.oldPath">{{ file.oldPath }} -> </template>{{ file.path }}
        </span>
        <span class="changed-file-row__meta">
          <n-tag size="small" :bordered="false">{{ statusLabels[file.status] }}</n-tag>
          <span class="line-stat add">+{{ file.additions }}</span>
          <span class="line-stat del">-{{ file.deletions }}</span>
        </span>
      </button>
    </div>
  </section>
</template>
