<script setup lang="ts">
import { LoaderCircle, OctagonX, Play } from 'lucide-vue-next';
import { computed, reactive, watch } from 'vue';
import type { SelectOption } from 'naive-ui';
import type {
  AuthorFilterOption,
  GitBranch,
  ProjectReviewFilters,
  ReviewFilters
} from '../../generated/bindings';

const props = defineProps<{
  projectId: string;
  defaultBranch: string;
  defaultDays?: number;
  defaultGlobRules: string[];
  savedFilters?: ProjectReviewFilters;
  branches: GitBranch[];
  authors: AuthorFilterOption[];
  authorsLoading?: boolean;
  loading?: boolean;
}>();

const emit = defineEmits<{
  submit: [filters: ReviewFilters];
  change: [filters: ReviewFilters];
  cancel: [];
}>();

type DateRangeValue = [number, number] | null;

const form = reactive(createInitialForm());
let applyingInitialFilters = false;

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
  () => [props.defaultBranch, props.defaultDays, props.defaultGlobRules, props.savedFilters] as const,
  () => applyInitialFilters(),
  { deep: true, flush: 'sync' }
);

watch(
  form,
  () => {
    if (!applyingInitialFilters) {
      emit('change', buildFilters());
    }
  },
  { deep: true, flush: 'sync' }
);

function submit(): void {
  emit('submit', buildFilters());
}

function buildFilters(): ReviewFilters {
  return {
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
  };
}

function createInitialForm() {
  return {
    branch: props.savedFilters?.branch?.trim() || props.defaultBranch || 'HEAD',
    dateRange: resolveInitialDateRange(),
    authorKeys: [...(props.savedFilters?.authorKeys ?? [])],
    messageQuery: props.savedFilters?.messageQuery ?? '',
    globRules: (props.savedFilters?.globRules ?? props.defaultGlobRules).join('\n')
  };
}

function applyInitialFilters(): void {
  applyingInitialFilters = true;
  const initialForm = createInitialForm();
  form.branch = initialForm.branch;
  form.dateRange = initialForm.dateRange;
  form.authorKeys = initialForm.authorKeys;
  form.messageQuery = initialForm.messageQuery;
  form.globRules = initialForm.globRules;
  applyingInitialFilters = false;
}

function resolveInitialDateRange(): DateRangeValue {
  const savedStart = toTimestamp(props.savedFilters?.startAt);
  const savedEnd = toTimestamp(props.savedFilters?.endAt);
  if (savedStart !== undefined && savedEnd !== undefined) {
    return [savedStart, savedEnd];
  }

  const end = new Date();
  const start = new Date(end);
  start.setUTCDate(start.getUTCDate() - defaultDays());
  return [start.getTime(), end.getTime()];
}

function defaultDays(): number {
  return props.defaultDays && props.defaultDays > 0 ? props.defaultDays : 30;
}

function toTimestamp(value?: string): number | undefined {
  if (!value) {
    return undefined;
  }

  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : undefined;
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

    <div class="action-row">
      <n-button class="filter-panel__submit" type="primary" attr-type="submit" :loading="loading">
        <Play :size="15" aria-hidden="true" />
        <span>分析</span>
      </n-button>
      <n-button v-if="loading" class="cancel-work-button" secondary type="warning" @click="emit('cancel')">
        <OctagonX :size="15" aria-hidden="true" />
        <LoaderCircle class="spin-icon" :size="15" aria-hidden="true" />
        <span>取消</span>
      </n-button>
    </div>
  </form>
</template>
