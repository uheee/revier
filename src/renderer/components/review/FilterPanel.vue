<script setup lang="ts">
import { Play } from 'lucide-vue-next';
import { computed, reactive, watch } from 'vue';
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

const form = reactive({
  branch: props.defaultBranch,
  dateRange: [] as Date[],
  authorKeys: [] as string[],
  messageQuery: '',
  globRules: props.defaultGlobRules.join('\n')
});

const branchOptions = computed(() =>
  props.branches.length > 0 ? props.branches : [{ name: props.defaultBranch || 'HEAD', current: true }]
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
    startAt: form.dateRange[0]?.toISOString(),
    endAt: form.dateRange[1]?.toISOString(),
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
</script>

<template>
  <form class="filter-panel" @submit.prevent="submit">
    <header class="section-heading">
      <h2>筛选</h2>
    </header>

    <label class="field">
      <span>分支</span>
      <el-select v-model="form.branch" filterable>
        <el-option
          v-for="branch in branchOptions"
          :key="branch.name"
          :label="branch.current ? `${branch.name}（当前）` : branch.name"
          :value="branch.name"
        />
      </el-select>
    </label>

    <label class="field">
      <span>时间范围</span>
      <el-date-picker
        v-model="form.dateRange"
        type="datetimerange"
        start-placeholder="开始时间"
        end-placeholder="结束时间"
      />
    </label>

    <label class="field">
      <span>作者</span>
      <el-select v-model="form.authorKeys" multiple filterable clearable :loading="authorsLoading">
        <el-option
          v-for="author in authors"
          :key="author.key"
          :label="author.email ? `${author.name} <${author.email}>` : author.name"
          :value="author.key"
        />
      </el-select>
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
