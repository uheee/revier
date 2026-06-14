<script setup lang="ts">
import { FolderOpen } from 'lucide-vue-next';
import { computed, reactive } from 'vue';

const props = withDefaults(
  defineProps<{
    loading?: boolean;
    title?: string;
    submitLabel?: string;
  }>(),
  {
    title: '新增项目',
    submitLabel: '添加项目'
  }
);

const emit = defineEmits<{
  submit: [payload: { repoPath: string; name?: string }];
}>();

const form = reactive({
  repoPath: '',
  name: ''
});

const canSubmit = computed(() => form.repoPath.trim().length > 0 && !props.loading);

function submit(): void {
  if (!canSubmit.value) {
    return;
  }

  emit('submit', {
    repoPath: form.repoPath.trim(),
    name: form.name.trim() || undefined
  });
  form.repoPath = '';
  form.name = '';
}

async function selectDirectory(): Promise<void> {
  const selection = await window.revier.projects.selectDirectory();
  if (!selection) {
    return;
  }

  form.repoPath = selection.path;
  form.name = selection.name;
}
</script>

<template>
  <form class="project-editor" @submit.prevent="submit">
    <header class="panel-heading">
      <h2>{{ title }}</h2>
    </header>

    <label class="field">
      <span>仓库路径</span>
      <div class="path-picker">
        <n-input v-model:value="form.repoPath" placeholder="选择或输入本地仓库路径" clearable @click="selectDirectory" />
        <n-button
          data-test="select-repo-directory"
          aria-label="选择仓库目录"
          @click="selectDirectory"
        >
          <FolderOpen :size="16" aria-hidden="true" />
        </n-button>
      </div>
    </label>

    <label class="field">
      <span>项目名称</span>
      <n-input v-model:value="form.name" placeholder="Revier" clearable />
    </label>

    <n-button
      class="project-editor__submit"
      data-test="project-editor-submit"
      type="primary"
      attr-type="submit"
      :disabled="!canSubmit"
    >
      <FolderOpen :size="16" aria-hidden="true" />
      <span>{{ submitLabel }}</span>
    </n-button>
  </form>
</template>
