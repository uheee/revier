import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AnalysisTaskSnapshot,
  AttributeBlocksRequest,
  AttributeBlocksResult,
  AuthorFilterOption,
  BranchAnalysisRestoreResult,
  BranchCacheStatus,
  ChangedFile,
  CommitOverlayRequest,
  DirectorySelection,
  EditorSettingsSnapshot,
  FileOverlay,
  FileOverlayRequest,
  GitBranch,
  OperationProgressSnapshot,
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
    restoreBranchAnalysis: (projectId: string, branch: string) =>
      invoke<BranchAnalysisRestoreResult>('review_restore_branch_analysis', { projectId, branch }),
    getBranchCacheStatus: (projectId: string, branch: string) =>
      invoke<BranchCacheStatus>('review_get_branch_cache_status', { projectId, branch }),
    setBranchSelectedFile: (projectId: string, branch: string, filePath?: string) =>
      invoke<void>('review_set_branch_selected_file', { projectId, branch, filePath }),
    startAnalysis: (filters: ReviewFilters, operationId: string) =>
      invoke<AnalysisTaskSnapshot>('review_start_analysis', { filters, operationId }),
    cancelAnalysis: (taskId: string) => invoke<void>('review_cancel_analysis', { taskId }),
    getTask: (taskId: string) => invoke<AnalysisTaskSnapshot>('review_get_task', { taskId }),
    async onTaskUpdate(callback: (task: AnalysisTaskSnapshot) => void): Promise<Unsubscribe> {
      const unlisten: UnlistenFn = await listen<AnalysisTaskSnapshot>(
        'review://task-updated',
        (event) => callback(event.payload)
      );
      return unlisten;
    },
    async onOperationProgress(
      callback: (progress: OperationProgressSnapshot) => void
    ): Promise<Unsubscribe> {
      const unlisten: UnlistenFn = await listen<OperationProgressSnapshot>(
        'review://operation-progress',
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
      invoke<FileOverlay>('review_get_commit_overlay', { request }),
    attributeBlocks: (request: AttributeBlocksRequest) =>
      invoke<AttributeBlocksResult>('review_attribute_blocks', { request })
  }
};
