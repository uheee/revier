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
import { revierClient } from '../api/revierClient';
import { toErrorMessage } from '../api/errors';
import { addNotification } from '../composables/useNotifications';
import { useReviewLayoutSizes } from '../composables/useReviewLayoutSizes';
import { useEditorSettings } from '../composables/useEditorSettings';
import { useProjectStore } from '../stores/projectStore';
import { useReviewStore } from '../stores/reviewStore';
import type {
  AnalysisTaskSnapshot,
  GitBranch,
  ProjectReviewFilters,
  RelatedCommit,
  ReviewFilters,
  TextEncoding
} from '../generated/bindings';

const route = useRoute();
const router = useRouter();
const projectStore = useProjectStore();
const reviewStore = useReviewStore();
const { snapshot: editorSettingsSnapshot, effectiveTheme } = useEditorSettings();
const editorSettings = computed(() => editorSettingsSnapshot.value.settings);
const editorThemeName = computed<'revier-light' | 'revier-dark'>(
  () => `revier-${effectiveTheme.value}`
);
const {
  task,
  files,
  overlay,
  selectedBlock,
  drilldownOverlay,
  selectedCommitHash,
  selectedCommit,
  drilldownError,
  activeOverlayPath,
  activeCommitHash,
  drilldownLoading,
  authors,
  authorsLoading,
  loading,
  error,
  diffComputationState
} = storeToRefs(reviewStore);
const projectId = computed(() => String(route.params.projectId ?? ''));
const selectedFilePath = ref<string>();
const requestedEncoding = ref<TextEncoding>(editorSettingsSnapshot.value.settings.defaultEncoding);
const commitRequestedEncoding = ref<TextEncoding>(editorSettingsSnapshot.value.settings.defaultEncoding);
const isEditorDraft = ref(false);
const branches = ref<GitBranch[]>([]);
const workspaceEl = ref<HTMLElement>();
const layout = useReviewLayoutSizes();
const project = computed(() => projectStore.projects.find((item) => item.id === projectId.value));
const overlayContextKey = computed(() => selectedFilePath.value ?? '');
const isCommitDrilldownOpen = computed(() => Boolean(
  reviewStore.selectedCommitHash || reviewStore.drilldownLoading || reviewStore.drilldownOverlay
));
const defaultBranch = computed(() => project.value?.preferences.defaultBranch ?? 'HEAD');
const defaultDays = computed(() => project.value?.preferences.defaultDays ?? 30);
const defaultGlobRules = computed(() => project.value?.preferences.defaultGlobRules ?? []);
const savedFilters = computed(() => project.value?.preferences.reviewFilters);
const initialAuthorBranch = computed(() => savedFilters.value?.branch ?? defaultBranch.value);
let unsubscribe: (() => void) | undefined;
let pendingReviewFilterSave: number | undefined;
let latestReviewFilters: ReviewFilters | undefined;
let workspaceActive = false;
let metadataRequestId = 0;

onMounted(async () => {
  workspaceActive = true;
  const subscribedProjectId = projectId.value;
  layout.setContainer(workspaceEl.value);
  void initializeProject();
  try {
    const stop = await revierClient.review.onTaskUpdate((snapshot) => {
      void handleTaskUpdate(snapshot);
    });
    if (workspaceActive) {
      unsubscribe = stop;
    } else {
      stop();
    }
  } catch (error) {
    if (workspaceActive) {
      addNotification({
        type: 'error',
        title: '分析任务事件订阅失败',
        message: `${subscribedProjectId}：${toErrorMessage(error)}`,
        source: 'Review'
      });
    }
  }
});

onBeforeUnmount(() => {
  workspaceActive = false;
  ++metadataRequestId;
  flushPendingReviewFilterSave();
  unsubscribe?.();
});

async function runAnalysis(filters: ReviewFilters): Promise<void> {
  selectedFilePath.value = undefined;
  await reviewStore.start(filters);
  if (reviewStore.task?.status === 'completed' && reviewStore.files[0]) {
    await selectFile(reviewStore.files[0].path);
  }
}

async function handleTaskUpdate(snapshot: AnalysisTaskSnapshot): Promise<void> {
  const selectedBeforeUpdate = selectedFilePath.value;
  await reviewStore.handleTaskUpdate(snapshot);
  if (
    snapshot.status === 'completed' &&
    !selectedBeforeUpdate &&
    !selectedFilePath.value &&
    reviewStore.files[0]
  ) {
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

  const requestId = ++metadataRequestId;
  const metadataProjectId = projectId.value;
  try {
    const loadedBranches = await revierClient.projects.listBranches(metadataProjectId);
    if (!workspaceActive || requestId !== metadataRequestId) return;
    branches.value = loadedBranches;
  } catch (error) {
    if (!workspaceActive || requestId !== metadataRequestId) return;
    addNotification({
      type: 'error',
      title: '项目分支加载失败',
      message: `${metadataProjectId}：${toErrorMessage(error)}`,
      source: 'Review'
    });
  }
  if (!workspaceActive || requestId !== metadataRequestId) return;
  await reviewStore.loadAuthors({
    projectId: metadataProjectId,
    branch: initialAuthorBranch.value
  });
}

function scheduleReviewFilterSave(filters: ReviewFilters): void {
  latestReviewFilters = filters;
  if (pendingReviewFilterSave !== undefined) {
    window.clearTimeout(pendingReviewFilterSave);
  }

  pendingReviewFilterSave = window.setTimeout(() => {
    flushPendingReviewFilterSave();
  }, 300);
}

function flushPendingReviewFilterSave(): void {
  if (pendingReviewFilterSave !== undefined) {
    window.clearTimeout(pendingReviewFilterSave);
    pendingReviewFilterSave = undefined;
  }

  if (!latestReviewFilters) {
    return;
  }

  const filters = latestReviewFilters;
  latestReviewFilters = undefined;
  void projectStore.saveReviewFilters(filters.projectId, toProjectReviewFilters(filters));
}

function toProjectReviewFilters(filters: ReviewFilters): ProjectReviewFilters {
  return {
    branch: filters.branch,
    startAt: filters.startAt,
    endAt: filters.endAt,
    authorKeys: filters.authorKeys ?? [],
    authorQuery: filters.authorQuery,
    messageQuery: filters.messageQuery,
    globRules: filters.globRules
  };
}

async function selectFile(filePath: string): Promise<void> {
  selectedFilePath.value = filePath;
  isEditorDraft.value = false;
  requestedEncoding.value = editorSettingsSnapshot.value.settings.defaultEncoding;
  if (await reviewStore.loadOverlay(filePath, requestedEncoding.value)) {
    isEditorDraft.value = false;
  }
}

async function openCommitDrilldown(commit: RelatedCommit): Promise<void> {
  if (!selectedFilePath.value) {
    return;
  }

  isEditorDraft.value = false;
  commitRequestedEncoding.value = editorSettingsSnapshot.value.settings.defaultEncoding;
  if (await reviewStore.loadCommitOverlay(
    selectedFilePath.value,
    commit.hash,
    commitRequestedEncoding.value,
    commit
  )) {
    isEditorDraft.value = false;
  }
}

function handleDraftChange(draft: boolean): void {
  isEditorDraft.value = draft;
  if (draft) {
    reviewStore.selectBlock(undefined);
  }
}

async function changeOverlayEncoding(encoding: TextEncoding): Promise<void> {
  const filePath = selectedFilePath.value;
  if (!filePath) return;
  if (await reviewStore.reloadOverlayEncoding(filePath, encoding)) {
    requestedEncoding.value = encoding;
    isEditorDraft.value = false;
  }
}

async function changeCommitOverlayEncoding(encoding: TextEncoding): Promise<void> {
  const filePath = selectedFilePath.value;
  const commitHash = reviewStore.selectedCommitHash;
  if (!filePath || !commitHash) return;
  if (await reviewStore.reloadCommitOverlayEncoding(filePath, commitHash, encoding)) {
    commitRequestedEncoding.value = encoding;
    isEditorDraft.value = false;
  }
}

function closeCommitDrilldown(): void {
  reviewStore.closeCommitDrilldown();
  isEditorDraft.value = false;
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
        :default-days="defaultDays"
        :default-glob-rules="defaultGlobRules"
        :saved-filters="savedFilters"
        :branches="branches"
        :authors="authors"
        :authors-loading="authorsLoading"
        :loading="loading"
        @change="scheduleReviewFilterSave"
        @submit="runAnalysis"
        @cancel="reviewStore.cancelAnalysis"
      />
      <TaskProgress :task="task" :loading="loading" :error="error" />
      <ChangedFileList
        :files="files"
        :selected-path="selectedFilePath"
        :loading-path="activeOverlayPath"
        @selected="selectFile"
        @cancel="reviewStore.cancelOverlay"
      />
    </aside>

    <ReviewLayoutResizer side="left" @resize="layout.resize" />

    <section class="review-diff-pane">
      <DiffViewer
        class="review-file-diff"
        :class="{ 'is-drilldown-covered': isCommitDrilldownOpen }"
        :overlay="overlay"
        :loading="loading"
        :selected-block-id="selectedBlock?.id"
        :settings="editorSettings"
        :theme-name="editorThemeName"
        :requested-encoding="requestedEncoding"
        :context-key="overlayContextKey"
        :diff-state="diffComputationState"
        @selected="reviewStore.selectBlock"
        @diff-blocks-change="reviewStore.acceptDiffBlocks($event, overlayContextKey)"
        @draft-change="handleDraftChange"
        @encoding-change="changeOverlayEncoding"
      />
      <DiffDrilldownOverlay
        :overlay="drilldownOverlay"
        :loading="drilldownLoading"
        :settings="editorSettings"
        :theme-name="editorThemeName"
        :requested-encoding="commitRequestedEncoding"
        :selected-commit-hash="selectedCommitHash"
        :selected-commit="selectedCommit"
        :error="drilldownError"
        @draft-change="handleDraftChange"
        @encoding-change="changeCommitOverlayEncoding"
        @close="closeCommitDrilldown"
        @cancel="closeCommitDrilldown"
      />
    </section>

    <ReviewLayoutResizer side="right" @resize="layout.resize" />

    <BlockDetailPanel
      class="review-detail-pane"
      :block="selectedBlock"
      :selected-commit-hash="selectedCommitHash"
      :active-commit-hash="activeCommitHash"
      :draft="isEditorDraft"
      :attribution-state="diffComputationState"
      @commit-selected="openCommitDrilldown"
      @cancel-commit="closeCommitDrilldown"
    />
  </main>
</template>
