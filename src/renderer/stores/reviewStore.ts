import { defineStore } from 'pinia';
import type {
  AnalysisTaskSnapshot,
  ChangedFile,
  DiffBlock,
  FileOverlay,
  ReviewFilters
} from '../../shared/reviewTypes';

interface ReviewState {
  task?: AnalysisTaskSnapshot;
  files: ChangedFile[];
  overlay?: FileOverlay;
  selectedBlock?: DiffBlock;
  loading: boolean;
  error?: string;
}

export const useReviewStore = defineStore('review', {
  state: (): ReviewState => ({
    task: undefined,
    files: [],
    overlay: undefined,
    selectedBlock: undefined,
    loading: false,
    error: undefined
  }),
  actions: {
    async start(filters: ReviewFilters): Promise<void> {
      this.loading = true;
      this.error = undefined;
      this.overlay = undefined;
      this.selectedBlock = undefined;
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
    }
  }
});

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
