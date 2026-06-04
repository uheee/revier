<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { storeToRefs } from 'pinia';
import { useRoute, useRouter } from 'vue-router';
import BlockDetailPanel from '../components/review/BlockDetailPanel.vue';
import ChangedFileList from '../components/review/ChangedFileList.vue';
import DiffDrilldownOverlay from '../components/review/DiffDrilldownOverlay.vue';
import DiffViewer from '../components/review/DiffViewer.vue';
import FilterPanel from '../components/review/FilterPanel.vue';
import ReviewLayoutResizer from '../components/review/ReviewLayoutResizer.vue';
import TaskProgress from '../components/review/TaskProgress.vue';
import { useReviewLayoutSizes } from '../composables/useReviewLayoutSizes';
import { useProjectStore } from '../stores/projectStore';
import { useReviewStore } from '../stores/reviewStore';
import type { RelatedCommit, ReviewFilters } from '../../shared/reviewTypes';
import type { GitBranch } from '../../shared/projectTypes';

const route = useRoute();
const router = useRouter();
const projectStore = useProjectStore();
const reviewStore = useReviewStore();
const {
  task,
  files,
  overlay,
  selectedBlock,
  drilldownOverlay,
  selectedCommitHash,
  drilldownLoading,
  authors,
  authorsLoading,
  loading,
  error
} = storeToRefs(reviewStore);
const projectId = computed(() => String(route.params.projectId ?? ''));
const selectedFilePath = ref<string>();
const branches = ref<GitBranch[]>([]);
const workspaceEl = ref<HTMLElement>();
const layout = useReviewLayoutSizes();
const project = computed(() => projectStore.projects.find((item) => item.id === projectId.value));
const defaultBranch = computed(() => project.value?.preferences.defaultBranch ?? 'HEAD');
const defaultGlobRules = computed(() => project.value?.preferences.defaultGlobRules ?? []);
let unsubscribe: (() => void) | undefined;

onMounted(() => {
  layout.setContainer(workspaceEl.value);
  void initializeProject();
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

async function initializeProject(): Promise<void> {
  await projectStore.loadProjects();
  await loadReviewMetadata();
}

async function loadReviewMetadata(): Promise<void> {
  if (!projectId.value) {
    return;
  }

  branches.value = await window.revier.projects.listBranches(projectId.value);
  await reviewStore.loadAuthors({
    projectId: projectId.value,
    branch: defaultBranch.value
  });
}

async function selectFile(filePath: string): Promise<void> {
  selectedFilePath.value = filePath;
  await reviewStore.loadOverlay(filePath);
}

async function openCommitDrilldown(commit: RelatedCommit): Promise<void> {
  if (!selectedFilePath.value) {
    return;
  }

  await reviewStore.loadCommitOverlay(selectedFilePath.value, commit.hash);
}
</script>

<template>
  <main
    ref="workspaceEl"
    class="review-workspace"
    :style="{ gridTemplateColumns: layout.gridTemplateColumns.value }"
  >
    <aside class="review-sidebar">
      <section class="review-project-summary">
        <n-button text @click="router.push({ name: 'projects' })">返回</n-button>
        <div>
          <h1>{{ project?.name ?? 'Review' }}</h1>
          <p>{{ project?.repoPath ?? projectId }}</p>
        </div>
      </section>

      <FilterPanel
        :project-id="projectId"
        :default-branch="defaultBranch"
        :default-glob-rules="defaultGlobRules"
        :branches="branches"
        :authors="authors"
        :authors-loading="authorsLoading"
        :loading="loading"
        @submit="runAnalysis"
      />
      <TaskProgress :task="task" :loading="loading" :error="error" />
      <ChangedFileList :files="files" :selected-path="selectedFilePath" @selected="selectFile" />
    </aside>

    <ReviewLayoutResizer side="left" @resize="layout.resize" />

    <section class="review-diff-pane">
      <DiffViewer
        :overlay="overlay"
        :loading="loading"
        :selected-block-id="selectedBlock?.id"
        @selected="reviewStore.selectBlock"
      />
      <DiffDrilldownOverlay
        :overlay="drilldownOverlay"
        :loading="drilldownLoading"
        @close="reviewStore.closeCommitDrilldown"
      />
    </section>

    <ReviewLayoutResizer side="right" @resize="layout.resize" />

    <BlockDetailPanel
      class="review-detail-pane"
      :block="selectedBlock"
      :selected-commit-hash="selectedCommitHash"
      @commit-selected="openCommitDrilldown"
    />
  </main>
</template>
