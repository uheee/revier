import { defineStore } from 'pinia';
import { toErrorMessage } from '../api/errors';
import { revierClient } from '../api/revierClient';
import type {
  ProjectId,
  ProjectReviewFilters,
  ReviewProject
} from '../generated/bindings';

interface ProjectState {
  projects: ReviewProject[];
  loading: boolean;
  error?: string;
}

let reviewFilterSaveSequence = 0;

export const useProjectStore = defineStore('projects', {
  state: (): ProjectState => ({
    projects: [],
    loading: false,
    error: undefined
  }),
  actions: {
    async loadProjects(): Promise<void> {
      this.loading = true;
      this.error = undefined;
      try {
        this.projects = await revierClient.projects.list();
      } catch (error) {
        this.error = toErrorMessage(error);
      } finally {
        this.loading = false;
      }
    },

    async addProject(repoPath: string, name?: string): Promise<void> {
      this.loading = true;
      this.error = undefined;
      try {
        await revierClient.projects.add(repoPath, name ? { name } : undefined);
        await this.loadProjects();
      } catch (error) {
        this.error = toErrorMessage(error);
        this.loading = false;
      }
    },

    async removeProject(projectId: string): Promise<void> {
      this.loading = true;
      this.error = undefined;
      try {
        await revierClient.projects.remove(projectId);
        await this.loadProjects();
      } catch (error) {
        this.error = toErrorMessage(error);
        this.loading = false;
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

      const saveId = ++reviewFilterSaveSequence;
      try {
        const savedProject = await revierClient.projects.update(updatedProject);
        if (saveId !== reviewFilterSaveSequence) {
          return;
        }

        const savedIndex = this.projects.findIndex((project) => project.id === projectId);
        if (savedIndex !== -1) {
          this.projects.splice(savedIndex, 1, savedProject);
        }
      } catch (error) {
        this.error = toErrorMessage(error);
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
