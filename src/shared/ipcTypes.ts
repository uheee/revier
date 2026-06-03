import type { GitBranch, RepositoryValidation, ReviewProject } from './projectTypes';
import type {
  AnalysisTaskSnapshot,
  ChangedFile,
  FileOverlay,
  FileOverlayRequest,
  ReviewFilters
} from './reviewTypes';

export type Unsubscribe = () => void;

export interface RevierApi {
  projects: {
    list(): Promise<ReviewProject[]>;
    add(repoPath: string, options?: Partial<ReviewProject>): Promise<ReviewProject>;
    update(project: ReviewProject): Promise<ReviewProject>;
    remove(projectId: string): Promise<void>;
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
  };
}

export type { ReviewProject, ReviewFilters };
