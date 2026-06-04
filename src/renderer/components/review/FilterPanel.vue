<script setup lang="ts">
import { Play } from 'lucide-vue-next';
import { computed, reactive, watch } from 'vue';
import type { SelectOption } from 'naive-ui';
import type { GitBranch } from '../../../shared/projectTypes';
import type { AuthorFilterOption, ReviewFilters } from '../../../shared/reviewTypes';

const props = defineProps<{
  projectId: string;
  defaultBranch: string;
  defaultGlobRules: string[];
  branches: GitBranch[];
  authors: AuthorFilterOption[];
  authorsLoading?: boolean;
  loading?: boolean;
}>();

const emit = defineEmits<{
  submit: [filters: ReviewFilters];
}>();

type DateRangeValue = [number, number] | null;

const form = reactive({
  branch: props.defaultBranch,
  dateRange: null as DateRangeValue,
  authorKeys: [] as string[],
  messageQuery: '',
  globRules: props.defaultGlobRules.join('\n')
});

const branchOptions = computed<SelectOption[]>(() =>
  (props.branches.length > 0 ? props.branches : [{ name: props.defaultBranch || 'HEAD', current: true }]).map(
    (branch) => ({
      label: branch.current ? `${branch.name}（当前）` : branch.name,
      value: branch.name
    })
  )
);

const authorOptions = computed<SelectOption[]>(() =>
  props.authors.map((author) => ({
    label: author.email ? `${author.name} <${author.email}>` : author.name,
    value: author.key
  }))
);

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
    startAt: toIsoString(form.dateRange?.[0]),
    endAt: toIsoString(form.dateRange?.[1]),
    authorKeys: [...form.authorKeys],
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

function toIsoString(value?: number): string | undefined {
  return value === undefined ? undefined : new Date(value).toISOString();
}
</script>

<template>
  <form class="filter-panel" @submit.prevent="submit">
    <header class="section-heading">
      <h2>筛选</h2>
    </header>

    <label class="field">
      <span>分支</span>
      <n-select
        v-model:value="form.branch"
        data-test="branch-select"
        :options="branchOptions"
        filterable
      />
    </label>

    <label class="field">
      <span>时间范围</span>
      <n-date-picker
        v-model:value="form.dateRange"
        data-test="date-range"
        type="datetimerange"
        start-placeholder="开始时间"
        end-placeholder="结束时间"
        clearable
      />
    </label>

    <label class="field">
      <span>作者</span>
      <n-select
        v-model:value="form.authorKeys"
        :options="authorOptions"
        multiple
        filterable
        clearable
        :loading="authorsLoading"
      />
    </label>

    <label class="field">
      <span>提交信息</span>
      <n-input v-model:value="form.messageQuery" clearable />
    </label>

    <label class="field">
      <span>文件规则</span>
      <n-input v-model:value="form.globRules" type="textarea" :rows="4" />
    </label>

    <n-button class="filter-panel__submit" type="primary" attr-type="submit" :loading="loading">
      <Play :size="15" aria-hidden="true" />
      <span>分析</span>
    </n-button>
  </form>
</template>
