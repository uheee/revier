<script setup lang="ts">
import { Play } from 'lucide-vue-next';
import { reactive, watch } from 'vue';
import type { ReviewFilters } from '../../../shared/reviewTypes';

const props = defineProps<{
  projectId: string;
  defaultBranch: string;
  defaultGlobRules: string[];
  loading?: boolean;
}>();

const emit = defineEmits<{
  submit: [filters: ReviewFilters];
}>();

const form = reactive({
  branch: props.defaultBranch,
  startAt: '',
  endAt: '',
  authorQuery: '',
  messageQuery: '',
  globRules: props.defaultGlobRules.join('\n')
});

watch(
  () => props.defaultBranch,
  (branch) => {
    if (!form.branch || form.branch === 'HEAD') {
      form.branch = branch;
    }
  }
);

watch(
  () => props.defaultGlobRules,
  (rules) => {
    if (!form.globRules.trim()) {
      form.globRules = rules.join('\n');
    }
  }
);

function submit(): void {
  emit('submit', {
    projectId: props.projectId,
    branch: form.branch.trim() || 'HEAD',
    startAt: optionalText(form.startAt),
    endAt: optionalText(form.endAt),
    authorQuery: optionalText(form.authorQuery),
    messageQuery: optionalText(form.messageQuery),
    globRules: form.globRules
      .split(/\r?\n/)
      .map((rule) => rule.trim())
      .filter(Boolean)
  });
}

function optionalText(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}
</script>

<template>
  <form class="filter-panel" @submit.prevent="submit">
    <header class="section-heading">
      <h2>筛选</h2>
    </header>

    <label class="field">
      <span>分支</span>
      <el-input v-model="form.branch" placeholder="HEAD" />
    </label>

    <div class="field-grid">
      <label class="field">
        <span>开始时间</span>
        <el-input v-model="form.startAt" placeholder="2026-05-01T00:00:00.000Z" />
      </label>
      <label class="field">
        <span>结束时间</span>
        <el-input v-model="form.endAt" placeholder="2026-06-01T00:00:00.000Z" />
      </label>
    </div>

    <label class="field">
      <span>作者</span>
      <el-input v-model="form.authorQuery" clearable />
    </label>

    <label class="field">
      <span>提交信息</span>
      <el-input v-model="form.messageQuery" clearable />
    </label>

    <label class="field">
      <span>文件规则</span>
      <el-input v-model="form.globRules" type="textarea" :rows="4" resize="none" />
    </label>

    <el-button class="filter-panel__submit" type="primary" native-type="submit" :loading="loading">
      <Play :size="15" aria-hidden="true" />
      <span>分析</span>
    </el-button>
  </form>
</template>
