<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { storeToRefs } from 'pinia';
import { useRoute, useRouter } from 'vue-router';
import BlockDetailPanel from '../components/review/BlockDetailPanel.vue';
import ChangedFileList from '../components/review/ChangedFileList.vue';
import DiffViewer from '../components/review/DiffViewer.vue';
import FilterPanel from '../components/review/FilterPanel.vue';
import TaskProgress from '../components/review/TaskProgress.vue';
import { useProjectStore } from '../stores/projectStore';
import { useReviewStore } from '../stores/reviewStore';
import type { ReviewFilters } from '../../shared/reviewTypes';

const route = useRoute();
const router = useRouter();
const projectStore = useProjectStore();
const reviewStore = useReviewStore();
const { task, files, overlay, selectedBlock, loading, error } = storeToRefs(reviewStore);
const projectId = computed(() => String(route.params.projectId ?? ''));
const selectedFilePath = ref<string>();
const project = computed(() => projectStore.projects.find((item) => item.id === projectId.value));
const defaultBranch = computed(() => project.value?.preferences.defaultBranch ?? 'HEAD');
const defaultGlobRules = computed(() => project.value?.preferences.defaultGlobRules ?? []);
let unsubscribe: (() => void) | undefined;

onMounted(() => {
  void projectStore.loadProjects();
  unsubscribe = window.revier.review.onTaskUpdate((snapshot) => {
    if (snapshot.taskId === reviewStore.task?.taskId) {
      reviewStore.task = snapshot;
    }
  });
});

onBeforeUnmount(() => {
  unsubscribe?.();
});

async function runAnalysis(filters: ReviewFilters): Promise<void> {
  selectedFilePath.value = undefined;
  await reviewStore.start(filters);
  if (reviewStore.files[0]) {
    await selectFile(reviewStore.files[0].path);
  }
}

async function selectFile(filePath: string): Promise<void> {
  selectedFilePath.value = filePath;
  await reviewStore.loadOverlay(filePath);
}
</script>

<template>
  <main class="review-workspace">
    <aside class="review-sidebar">
      <section class="review-project-summary">
        <el-button text @click="router.push({ name: 'projects' })">返回</el-button>
        <div>
          <h1>{{ project?.name ?? 'Review' }}</h1>
          <p>{{ project?.repoPath ?? projectId }}</p>
        </div>
      </section>

      <FilterPanel
        :project-id="projectId"
        :default-branch="defaultBranch"
        :default-glob-rules="defaultGlobRules"
        :loading="loading"
        @submit="runAnalysis"
      />
      <TaskProgress :task="task" :loading="loading" :error="error" />
      <ChangedFileList :files="files" :selected-path="selectedFilePath" @selected="selectFile" />
    </aside>

    <section class="review-diff-pane">
      <DiffViewer
        :overlay="overlay"
        :loading="loading"
        :selected-block-id="selectedBlock?.id"
        @selected="reviewStore.selectBlock"
      />
    </section>

    <BlockDetailPanel class="review-detail-pane" :block="selectedBlock" />
  </main>
</template>
