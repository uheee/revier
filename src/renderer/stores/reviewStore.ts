import { defineStore } from 'pinia';
import type {
  AnalysisTaskSnapshot,
  AuthorFilterOption,
  ChangedFile,
  DiffBlock,
  FileOverlay,
  ReviewAuthorOptionsRequest,
  ReviewFilters
} from '../../shared/reviewTypes';

interface ReviewState {
  task?: AnalysisTaskSnapshot;
  files: ChangedFile[];
  overlay?: FileOverlay;
  selectedBlock?: DiffBlock;
  drilldownOverlay?: FileOverlay;
  selectedCommitHash?: string;
  drilldownLoading: boolean;
  authors: AuthorFilterOption[];
  authorsLoading: boolean;
  loading: boolean;
  error?: string;
}

export const useReviewStore = defineStore('review', {
  state: (): ReviewState => ({
    task: undefined,
    files: [],
    overlay: undefined,
    selectedBlock: undefined,
    drilldownOverlay: undefined,
    selectedCommitHash: undefined,
    drilldownLoading: false,
    authors: [],
    authorsLoading: false,
    loading: false,
    error: undefined
  }),
  actions: {
    async start(filters: ReviewFilters): Promise<void> {
      this.loading = true;
      this.error = undefined;
      this.overlay = undefined;
      this.selectedBlock = undefined;
      this.closeCommitDrilldown();
      try {
        this.task = await window.revier.review.startAnalysis(filters);
        this.files = await window.revier.review.listChangedFiles(this.task.taskId);
      } catch (error) {
        this.error = toErrorMessage(error);
      } finally {
        this.loading = false;
      }
    },

    async loadOverlay(filePath: string): Promise<void> {
      if (!this.task) {
        this.error = 'No active analysis task';
        return;
      }

      this.loading = true;
      this.error = undefined;
      this.selectedBlock = undefined;
      this.closeCommitDrilldown();
      try {
        this.overlay = await window.revier.review.getFileOverlay({
          taskId: this.task.taskId,
          filePath
        });
      } catch (error) {
        this.error = toErrorMessage(error);
      } finally {
        this.loading = false;
      }
    },

    selectBlock(block?: DiffBlock): void {
      this.selectedBlock = block;
    },

    async loadCommitOverlay(filePath: string, commitHash: string): Promise<void> {
      if (!this.task) {
        this.error = 'No active analysis task';
        return;
      }

      this.drilldownLoading = true;
      this.error = undefined;
      this.selectedCommitHash = commitHash;
      try {
        this.drilldownOverlay = await window.revier.review.getCommitOverlay({
          taskId: this.task.taskId,
          filePath,
          commitHash
        });
      } catch (error) {
        this.error = toErrorMessage(error);
      } finally {
        this.drilldownLoading = false;
      }
    },

    closeCommitDrilldown(): void {
      this.drilldownOverlay = undefined;
      this.selectedCommitHash = undefined;
    },

    async loadAuthors(request: ReviewAuthorOptionsRequest): Promise<void> {
      this.authorsLoading = true;
      this.error = undefined;
      try {
        this.authors = await window.revier.review.listAuthors(request);
      } catch (error) {
        this.error = toErrorMessage(error);
      } finally {
        this.authorsLoading = false;
      }
    }
  }
});

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
