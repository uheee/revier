<script setup lang="ts">
import { h } from 'vue';
import { NButton, NPopconfirm, type DataTableColumns } from 'naive-ui';
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

const columns: DataTableColumns<ReviewProject> = [
  {
    title: '项目',
    key: 'name',
    width: 150,
    render(row) {
      return h('div', { class: 'project-name-cell' }, [
        row.pinned ? h('span', { class: 'pin-dot', 'aria-label': '已固定' }) : null,
        h('span', row.name)
      ]);
    }
  },
  {
    title: '仓库路径',
    key: 'repoPath',
    width: 280,
    render(row) {
      return h('span', { class: 'path-cell' }, row.repoPath);
    }
  },
  {
    title: '默认分支',
    key: 'defaultBranch',
    width: 110,
    render(row) {
      return row.preferences.defaultBranch || '-';
    }
  },
  {
    title: '默认天数',
    key: 'defaultDays',
    width: 100,
    render(row) {
      return String(row.preferences.defaultDays ?? '-');
    }
  },
  {
    title: '最近打开',
    key: 'lastOpenedAt',
    width: 140,
    render(row) {
      return formatDate(row.lastOpenedAt);
    }
  },
  {
    title: '操作',
    key: 'actions',
    width: 100,
    render(row) {
      return h('div', { class: 'project-actions' }, [
        h(
          NButton,
          {
            text: true,
            type: 'primary',
            size: 'small',
            onClick: () => emit('open', row.id)
          },
          { default: () => '打开' }
        ),
        h(
          NPopconfirm,
          {
            positiveText: '移除',
            negativeText: '取消',
            onPositiveClick: () => emit('remove', row.id)
          },
          {
            trigger: () =>
              h(
                NButton,
                {
                  text: true,
                  type: 'error',
                  size: 'small'
                },
                { default: () => '移除' }
              ),
            default: () => '移除这个项目记录？'
          }
        )
      ]);
    }
  }
];
</script>

<template>
  <section class="project-list">
    <header class="panel-heading project-list__heading">
      <h2>项目列表</h2>
    </header>

    <n-data-table
      class="project-table"
      :loading="loading"
      :columns="columns"
      :data="projects"
      :row-key="(row: ReviewProject) => row.id"
      :bordered="false"
      :single-line="false"
      :scroll-x="880"
      table-layout="fixed"
      flex-height
      size="small"
    />
  </section>
</template>
