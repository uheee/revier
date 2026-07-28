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
import { parseBoundary } from '../contracts/runtime/parseBoundary';
import {
  editorSettingsSnapshotSchema,
  reviewProjectListSchema,
  reviewProjectSchema
} from '../contracts/runtime/schemas';

type Unsubscribe = () => void;

export const revierClient = {
  settings: {
    async getEditorSettings(): Promise<EditorSettingsSnapshot> {
      return parseBoundary(editorSettingsSnapshotSchema, await invoke<unknown>('editor_settings_get'), {
        source: 'editor_settings_get'
      });
    }
  },
  projects: {
    async list(): Promise<ReviewProject[]> {
      return parseBoundary(reviewProjectListSchema, await invoke<unknown>('projects_list'), {
        source: 'projects_list'
      });
    },
    async add(repoPath: string, options?: Partial<ReviewProject>): Promise<ReviewProject> {
      return parseBoundary(
        reviewProjectSchema,
        await invoke<unknown>('projects_add', { repoPath, options }),
        { source: 'projects_add' }
      );
    },
    async update(project: ReviewProject): Promise<ReviewProject> {
      return parseBoundary(
        reviewProjectSchema,
        await invoke<unknown>('projects_update', { project }),
        { source: 'projects_update' }
      );
    },
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
