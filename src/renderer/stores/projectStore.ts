import { defineStore } from 'pinia';
import type { ReviewProject } from '../../shared/projectTypes';

interface ProjectState {
  projects: ReviewProject[];
  loading: boolean;
  error?: string;
}

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
        this.projects = await window.revier.projects.list();
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
        await window.revier.projects.add(repoPath, name ? { name } : undefined);
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
        await window.revier.projects.remove(projectId);
        await this.loadProjects();
      } catch (error) {
        this.error = toErrorMessage(error);
        this.loading = false;
      }
    }
  }
});

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
