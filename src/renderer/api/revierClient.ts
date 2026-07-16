import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AnalysisTaskSnapshot,
  AuthorFilterOption,
  ChangedFile,
  CommitOverlayRequest,
  DirectorySelection,
  EditorSettingsSnapshot,
  FileOverlay,
  FileOverlayRequest,
  GitBranch,
  RepositoryValidation,
  ReviewAuthorOptionsRequest,
  ReviewFilters,
  ReviewProject
} from '../generated/bindings';

type Unsubscribe = () => void;

export const revierClient = {
  settings: {
    getEditorSettings: () => invoke<EditorSettingsSnapshot>('editor_settings_get')
  },
  projects: {
    list: () => invoke<ReviewProject[]>('projects_list'),
    add: (repoPath: string, options?: Partial<ReviewProject>) =>
      invoke<ReviewProject>('projects_add', { repoPath, options }),
    update: (project: ReviewProject) => invoke<ReviewProject>('projects_update', { project }),
    remove: (projectId: string) => invoke<void>('projects_remove', { projectId }),
    selectDirectory: async () =>
      (await invoke<DirectorySelection | null>('projects_select_directory')) ?? undefined,
    validateRepository: (repoPath: string) =>
      invoke<RepositoryValidation>('projects_validate_repository', { repoPath }),
    listBranches: (projectId: string) =>
      invoke<GitBranch[]>('projects_list_branches', { projectId })
  },
  review: {
    startAnalysis: (filters: ReviewFilters) =>
      invoke<AnalysisTaskSnapshot>('review_start_analysis', { filters }),
    cancelAnalysis: (taskId: string) => invoke<void>('review_cancel_analysis', { taskId }),
    getTask: (taskId: string) => invoke<AnalysisTaskSnapshot>('review_get_task', { taskId }),
    async onTaskUpdate(callback: (task: AnalysisTaskSnapshot) => void): Promise<Unsubscribe> {
      const unlisten: UnlistenFn = await listen<AnalysisTaskSnapshot>(
        'review://task-updated',
        (event) => callback(event.payload)
      );
      return unlisten;
    },
    listChangedFiles: (taskId: string) =>
      invoke<ChangedFile[]>('review_list_changed_files', { taskId }),
    getFileOverlay: (request: FileOverlayRequest) =>
      invoke<FileOverlay>('review_get_file_overlay', { request }),
    listAuthors: (request: ReviewAuthorOptionsRequest) =>
      invoke<AuthorFilterOption[]>('review_list_authors', { request }),
    getCommitOverlay: (request: CommitOverlayRequest) =>
      invoke<FileOverlay>('review_get_commit_overlay', { request })
  }
};
