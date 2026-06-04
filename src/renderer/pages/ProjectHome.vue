<script setup lang="ts">
import { storeToRefs } from 'pinia';
import { onMounted } from 'vue';
import { useRouter } from 'vue-router';
import ProjectEditor from '../components/projects/ProjectEditor.vue';
import ProjectList from '../components/projects/ProjectList.vue';
import { useProjectStore } from '../stores/projectStore';

const router = useRouter();
const projectStore = useProjectStore();
const { projects, loading, error } = storeToRefs(projectStore);

onMounted(() => {
  void projectStore.loadProjects();
});

function addProject(payload: { repoPath: string; name?: string }): void {
  void projectStore.addProject(payload.repoPath, payload.name);
}

function removeProject(projectId: string): void {
  void projectStore.removeProject(projectId);
}

function openProject(projectId: string): void {
  void router.push({ name: 'review', params: { projectId } });
}
</script>

<template>
  <main class="project-home">
    <header class="project-topbar">
      <div class="project-brand">
        <h1>Revier</h1>
        <span>{{ projects.length }} 个项目</span>
      </div>
      <n-button :loading="loading" @click="projectStore.loadProjects()">刷新</n-button>
    </header>

    <n-alert
      v-if="error"
      class="project-alert"
      :title="error"
      type="error"
      show-icon
      :closable="false"
    />

    <section class="project-layout">
      <aside class="project-side-panel">
        <ProjectEditor :loading="loading" @submit="addProject" />
      </aside>
      <section class="project-main-panel">
        <ProjectList
          :projects="projects"
          :loading="loading"
          @open="openProject"
          @remove="removeProject"
        />
      </section>
    </section>
  </main>
</template>
