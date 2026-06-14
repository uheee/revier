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
  activeOverlayPath?: string;
  activeCommitHash?: string;
  drilldownLoading: boolean;
  authors: AuthorFilterOption[];
  authorsLoading: boolean;
  loading: boolean;
  error?: string;
  analysisRequestId: number;
  overlayRequestId: number;
  drilldownRequestId: number;
}

export const useReviewStore = defineStore('review', {
  state: (): ReviewState => ({
    task: undefined,
    files: [],
    overlay: undefined,
    selectedBlock: undefined,
    drilldownOverlay: undefined,
    selectedCommitHash: undefined,
    activeOverlayPath: undefined,
    activeCommitHash: undefined,
    drilldownLoading: false,
    authors: [],
    authorsLoading: false,
    loading: false,
    error: undefined,
    analysisRequestId: 0,
    overlayRequestId: 0,
    drilldownRequestId: 0
  }),
  actions: {
    async start(filters: ReviewFilters): Promise<void> {
      const requestId = ++this.analysisRequestId;
      this.loading = true;
      this.error = undefined;
      this.overlay = undefined;
      this.selectedBlock = undefined;
      this.files = [];
      this.cancelOverlay();
      this.closeCommitDrilldown();
      try {
        const task = await window.revier.review.startAnalysis(filters);
        if (requestId !== this.analysisRequestId) return;
        this.task = task;
        const files = await window.revier.review.listChangedFiles(task.taskId);
        if (requestId !== this.analysisRequestId) return;
        this.files = files;
      } catch (error) {
        if (requestId === this.analysisRequestId) this.error = toErrorMessage(error);
      } finally {
        if (requestId === this.analysisRequestId) this.loading = false;
      }
    },

    async cancelAnalysis(): Promise<void> {
      const taskId = this.task?.taskId;
      ++this.analysisRequestId;
      this.loading = false;
      this.cancelOverlay();
      this.closeCommitDrilldown();
      if (taskId) {
        await window.revier.review.cancelAnalysis(taskId);
      }
    },

    async loadOverlay(filePath: string): Promise<void> {
      if (!this.task) {
        this.error = 'No active analysis task';
        return;
      }

      const requestId = ++this.overlayRequestId;
      this.loading = true;
      this.activeOverlayPath = filePath;
      this.error = undefined;
      this.selectedBlock = undefined;
      this.closeCommitDrilldown();
      try {
        const overlay = await window.revier.review.getFileOverlay({
          taskId: this.task.taskId,
          filePath
        });
        if (requestId !== this.overlayRequestId) return;
        this.overlay = overlay;
      } catch (error) {
        if (requestId === this.overlayRequestId) this.error = toErrorMessage(error);
      } finally {
        if (requestId === this.overlayRequestId) {
          this.loading = false;
          this.activeOverlayPath = undefined;
        }
      }
    },

    cancelOverlay(): void {
      ++this.overlayRequestId;
      this.loading = false;
      this.activeOverlayPath = undefined;
    },

    selectBlock(block?: DiffBlock): void {
      this.selectedBlock = block;
    },

    async loadCommitOverlay(filePath: string, commitHash: string): Promise<void> {
      if (!this.task) {
        this.error = 'No active analysis task';
        return;
      }

      const requestId = ++this.drilldownRequestId;
      this.drilldownLoading = true;
      this.error = undefined;
      this.selectedCommitHash = commitHash;
      this.activeCommitHash = commitHash;
      this.drilldownOverlay = undefined;
      try {
        const overlay = await window.revier.review.getCommitOverlay({
          taskId: this.task.taskId,
          filePath,
          commitHash
        });
        if (requestId !== this.drilldownRequestId) return;
        this.drilldownOverlay = overlay;
      } catch (error) {
        if (requestId === this.drilldownRequestId) this.error = toErrorMessage(error);
      } finally {
        if (requestId === this.drilldownRequestId) {
          this.drilldownLoading = false;
          this.activeCommitHash = undefined;
        }
      }
    },

    closeCommitDrilldown(): void {
      ++this.drilldownRequestId;
      this.drilldownLoading = false;
      this.drilldownOverlay = undefined;
      this.selectedCommitHash = undefined;
      this.activeCommitHash = undefined;
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
