import { defineStore } from 'pinia';
import { toErrorMessage } from '../api/errors';
import { revierClient } from '../api/revierClient';
import { addNotification } from '../composables/useNotifications';
import type {
  ProjectReviewFilters,
  ReviewProject
} from '../generated/bindings';

type ProjectId = string;

interface ProjectState {
  projects: ReviewProject[];
  loading: boolean;
  error?: string;
  loadRequestId: number;
  addRequestId: number;
  removeRequestId: number;
  reviewFilterSaveSequence: number;
}

function notifyProjectError(title: string, message: string): void {
  addNotification({ type: 'error', title, message, source: 'Projects' });
}

export const useProjectStore = defineStore('projects', {
  state: (): ProjectState => ({
    projects: [],
    loading: false,
    error: undefined,
    loadRequestId: 0,
    addRequestId: 0,
    removeRequestId: 0,
    reviewFilterSaveSequence: 0
  }),
  actions: {
    async loadProjects(): Promise<void> {
      const requestId = ++this.loadRequestId;
      this.loading = true;
      this.error = undefined;
      try {
        const projects = await revierClient.projects.list();
        if (requestId !== this.loadRequestId) return;
        this.projects = projects;
      } catch (error) {
        if (requestId === this.loadRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          notifyProjectError('项目列表加载失败', message);
        }
      } finally {
        if (requestId === this.loadRequestId) {
          this.loading = false;
        }
      }
    },

    async addProject(repoPath: string, name?: string): Promise<void> {
      const requestId = ++this.addRequestId;
      this.loading = true;
      this.error = undefined;
      try {
        await revierClient.projects.add(repoPath, name ? { name } : undefined);
        if (requestId !== this.addRequestId) return;
        await this.loadProjects();
      } catch (error) {
        if (requestId === this.addRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          this.loading = false;
          notifyProjectError('项目添加失败', `${repoPath}：${message}`);
        }
      }
    },

    async removeProject(projectId: string): Promise<void> {
      const requestId = ++this.removeRequestId;
      const projectName = this.projects.find((project) => project.id === projectId)?.name ?? projectId;
      this.loading = true;
      this.error = undefined;
      try {
        await revierClient.projects.remove(projectId);
        if (requestId !== this.removeRequestId) return;
        await this.loadProjects();
      } catch (error) {
        if (requestId === this.removeRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          this.loading = false;
          notifyProjectError('项目删除失败', `${projectName}：${message}`);
        }
      }
    },

    async saveReviewFilters(projectId: ProjectId, filters: ProjectReviewFilters): Promise<void> {
      const index = this.projects.findIndex((project) => project.id === projectId);
      if (index === -1) {
        return;
      }

      this.error = undefined;
      const updatedProject = withReviewFilters(this.projects[index], filters);
      this.projects.splice(index, 1, updatedProject);

      const saveId = ++this.reviewFilterSaveSequence;
      try {
        const savedProject = await revierClient.projects.update(updatedProject);
        if (saveId !== this.reviewFilterSaveSequence) {
          return;
        }

        const savedIndex = this.projects.findIndex((project) => project.id === projectId);
        if (savedIndex !== -1) {
          this.projects.splice(savedIndex, 1, savedProject);
        }
      } catch (error) {
        if (saveId === this.reviewFilterSaveSequence) {
          const message = toErrorMessage(error);
          this.error = message;
          notifyProjectError('分析筛选保存失败', `${updatedProject.name}：${message}`);
        }
      }
    }
  }
});

function withReviewFilters(
  project: ReviewProject,
  filters: ProjectReviewFilters
): ReviewProject {
  return {
    ...project,
    preferences: {
      ...project.preferences,
      defaultBranch: filters.branch ?? project.preferences.defaultBranch,
      defaultGlobRules: filters.globRules ?? project.preferences.defaultGlobRules,
      reviewFilters: {
        ...project.preferences.reviewFilters,
        ...filters,
        authorKeys: filters.authorKeys ?? [],
        globRules: filters.globRules ?? project.preferences.reviewFilters?.globRules
      }
    }
  };
}
