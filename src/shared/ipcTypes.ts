import type { DirectorySelection, GitBranch, RepositoryValidation, ReviewProject } from './projectTypes';
import type {
  AnalysisTaskSnapshot,
  AuthorFilterOption,
  ChangedFile,
  CommitOverlayRequest,
  FileOverlay,
  FileOverlayRequest,
  ReviewAuthorOptionsRequest,
  ReviewFilters
} from './reviewTypes';

export type Unsubscribe = () => void;

export const ipcChannels = {
  projectsList: 'projects:list',
  projectsAdd: 'projects:add',
  projectsUpdate: 'projects:update',
  projectsRemove: 'projects:remove',
  projectsSelectDirectory: 'projects:selectDirectory',
  projectsValidateRepository: 'projects:validateRepository',
  projectsListBranches: 'projects:listBranches',
  reviewStartAnalysis: 'review:startAnalysis',
  reviewCancelAnalysis: 'review:cancelAnalysis',
  reviewGetTask: 'review:getTask',
  reviewListChangedFiles: 'review:listChangedFiles',
  reviewGetFileOverlay: 'review:getFileOverlay',
  reviewListAuthors: 'review:listAuthors',
  reviewGetCommitOverlay: 'review:getCommitOverlay',
  reviewTaskUpdated: 'review:taskUpdated'
} as const;

export interface RevierApi {
  projects: {
    list(): Promise<ReviewProject[]>;
    add(repoPath: string, options?: Partial<ReviewProject>): Promise<ReviewProject>;
    update(project: ReviewProject): Promise<ReviewProject>;
    remove(projectId: string): Promise<void>;
    selectDirectory(): Promise<DirectorySelection | undefined>;
    validateRepository(repoPath: string): Promise<RepositoryValidation>;
    listBranches(projectId: string): Promise<GitBranch[]>;
  };
  review: {
    startAnalysis(filters: ReviewFilters): Promise<AnalysisTaskSnapshot>;
    cancelAnalysis(taskId: string): Promise<void>;
    getTask(taskId: string): Promise<AnalysisTaskSnapshot>;
    onTaskUpdate(callback: (task: AnalysisTaskSnapshot) => void): Unsubscribe;
    listChangedFiles(taskId: string): Promise<ChangedFile[]>;
    getFileOverlay(request: FileOverlayRequest): Promise<FileOverlay>;
    listAuthors(request: ReviewAuthorOptionsRequest): Promise<AuthorFilterOption[]>;
    getCommitOverlay(request: CommitOverlayRequest): Promise<FileOverlay>;
  };
}

export type { ReviewProject, ReviewFilters };
