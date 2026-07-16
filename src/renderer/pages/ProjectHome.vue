<script setup lang="ts">
import { FolderOpen } from 'lucide-vue-next';
import { storeToRefs } from 'pinia';
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import ProjectEditor from '../components/projects/ProjectEditor.vue';
import ProjectList from '../components/projects/ProjectList.vue';
import NotificationCenter from '../components/NotificationCenter.vue';
import { useProjectStore } from '../stores/projectStore';

const router = useRouter();
const projectStore = useProjectStore();
const { projects, loading, error } = storeToRefs(projectStore);
const projectDialogVisible = ref(false);
const projectOpenError = ref<string>();
const dialogError = computed(() => projectOpenError.value ?? projectStore.error);

onMounted(() => {
  void projectStore.loadProjects();
});

function openProjectDialog(): void {
  projectOpenError.value = undefined;
  projectDialogVisible.value = true;
}

async function addProjectAndOpen(payload: { repoPath: string; name?: string }): Promise<void> {
  projectOpenError.value = undefined;
  await projectStore.addProject(payload.repoPath, payload.name);

  if (projectStore.error) {
    return;
  }

  const project = projects.value.find(
    (candidate) => normalizeRepoPath(candidate.repoPath) === normalizeRepoPath(payload.repoPath)
  );
  if (!project) {
    projectOpenError.value = '项目已添加，但未能定位项目记录，请刷新后重试';
    return;
  }

  projectDialogVisible.value = false;
  await router.push({ name: 'review', params: { projectId: project.id } });
}

function removeProject(projectId: string): void {
  void projectStore.removeProject(projectId);
}

function openProject(projectId: string): void {
  void router.push({ name: 'review', params: { projectId } });
}

function normalizeRepoPath(repoPath: string): string {
  return repoPath.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase();
}
</script>

<template>
  <main class="project-home">
    <header class="project-topbar">
      <div class="project-brand">
        <h1>Revier</h1>
        <span>{{ projects.length }} 个项目</span>
      </div>
      <div class="project-topbar__actions">
        <NotificationCenter />
        <n-button :loading="loading" @click="projectStore.loadProjects()">刷新</n-button>
        <n-button
          data-test="open-project-dialog"
          type="primary"
          :disabled="loading"
          @click="openProjectDialog"
        >
          <FolderOpen :size="16" aria-hidden="true" />
          <span>打开项目</span>
        </n-button>
      </div>
    </header>

    <n-alert
      v-if="error"
      class="project-alert"
      :title="error"
      type="error"
      show-icon
      :closable="false"
    />

    <section class="project-content">
      <section v-if="projects.length > 0 || loading" class="project-main-panel">
        <ProjectList
          :projects="projects"
          :loading="loading"
          @open="openProject"
          @remove="removeProject"
        />
      </section>

      <section v-else class="project-empty" aria-label="欢迎">
        <div class="project-empty__content">
          <h2>还没有项目</h2>
          <p>打开一个本地 Git 仓库，开始查看提交范围和代码差异。</p>
          <n-button data-test="open-project-dialog" type="primary" @click="openProjectDialog">
            <FolderOpen :size="16" aria-hidden="true" />
            <span>打开项目</span>
          </n-button>
        </div>
      </section>
    </section>

    <n-modal
      v-model:show="projectDialogVisible"
      class="project-dialog"
      preset="card"
      title="打开项目"
      :bordered="false"
      :mask-closable="!loading"
    >
      <div data-test="project-dialog">
        <n-alert
          v-if="dialogError"
          class="project-dialog__alert"
          :title="dialogError"
          type="error"
          show-icon
          :closable="false"
        />
        <ProjectEditor
          :loading="loading"
          title="仓库信息"
          submit-label="打开项目"
          @submit="addProjectAndOpen"
        />
      </div>
    </n-modal>
  </main>
</template>
