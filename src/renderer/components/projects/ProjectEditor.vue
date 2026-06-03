<script setup lang="ts">
import { FolderOpen } from 'lucide-vue-next';
import { computed, reactive } from 'vue';

const props = defineProps<{
  loading?: boolean;
}>();

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
</script>

<template>
  <form class="project-editor" @submit.prevent="submit">
    <header class="panel-heading">
      <h2>新增项目</h2>
    </header>

    <label class="field">
      <span>仓库路径</span>
      <el-input v-model="form.repoPath" placeholder="E:/Projects/revier" clearable />
    </label>

    <label class="field">
      <span>项目名称</span>
      <el-input v-model="form.name" placeholder="Revier" clearable />
    </label>

    <el-button class="project-editor__submit" type="primary" native-type="submit" :disabled="!canSubmit">
      <FolderOpen :size="16" aria-hidden="true" />
      <span>添加项目</span>
    </el-button>
  </form>
</template>
