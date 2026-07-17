import { defineStore } from 'pinia';
import { toErrorMessage } from '../api/errors';
import { revierClient } from '../api/revierClient';
import type {
  AnalysisTaskSnapshot,
  AuthorFilterOption,
  ChangedFile,
  DiffBlock,
  FileOverlay,
  ReviewAuthorOptionsRequest,
  ReviewFilters,
  TextEncoding
} from '../generated/bindings';

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

function isTerminalStatus(status: AnalysisTaskSnapshot['status']): boolean {
  return status === 'completed' || status === 'failed' || status === 'cancelled';
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
      this.error = undefined;
      this.overlay = undefined;
      this.selectedBlock = undefined;
      this.files = [];
      this.cancelOverlay();
      this.closeCommitDrilldown();
      this.loading = true;
      try {
        const task = await revierClient.review.startAnalysis(filters);
        if (requestId !== this.analysisRequestId) return;
        if (
          this.task?.taskId === task.taskId &&
          isTerminalStatus(this.task.status) &&
          !isTerminalStatus(task.status)
        ) {
          return;
        }
        this.task = task;
        if (task.status === 'completed' || task.status === 'failed' || task.status === 'cancelled') {
          await this.handleTaskUpdate(task);
        }
      } catch (error) {
        if (requestId === this.analysisRequestId) this.error = toErrorMessage(error);
        if (requestId === this.analysisRequestId) this.loading = false;
      }
    },

    async handleTaskUpdate(snapshot: AnalysisTaskSnapshot): Promise<void> {
      if (this.task && snapshot.taskId !== this.task.taskId) {
        return;
      }
      if (this.task && isTerminalStatus(this.task.status) && !isTerminalStatus(snapshot.status)) {
        return;
      }

      this.task = snapshot;
      if (snapshot.status === 'completed') {
        const requestId = this.analysisRequestId;
        try {
          const files = await revierClient.review.listChangedFiles(snapshot.taskId);
          if (requestId !== this.analysisRequestId) return;
          if (this.task?.taskId !== snapshot.taskId) return;
          this.files = files;
          this.error = undefined;
        } catch (error) {
          if (requestId === this.analysisRequestId) this.error = toErrorMessage(error);
        } finally {
          if (requestId === this.analysisRequestId && this.task?.taskId === snapshot.taskId) {
            this.loading = false;
          }
        }
        return;
      }

      if (snapshot.status === 'failed') {
        this.error = snapshot.error?.message ?? '分析任务失败';
        this.loading = false;
        return;
      }

      if (snapshot.status === 'cancelled') {
        this.loading = false;
      }
    },

    async cancelAnalysis(): Promise<void> {
      const taskId = this.task?.taskId;
      ++this.analysisRequestId;
      this.loading = false;
      this.cancelOverlay();
      this.closeCommitDrilldown();
      if (this.task && this.task.status !== 'completed' && this.task.status !== 'failed') {
        this.task = {
          ...this.task,
          status: 'cancelled',
          stage: 'ready',
          progress: undefined,
          message: '任务已取消',
          error: undefined
        };
      }
      if (taskId) {
        await revierClient.review.cancelAnalysis(taskId);
      }
    },

    async loadOverlay(filePath: string, encoding?: TextEncoding): Promise<boolean> {
      if (!this.task) {
        this.error = 'No active analysis task';
        return false;
      }

      const requestId = ++this.overlayRequestId;
      this.loading = true;
      this.activeOverlayPath = filePath;
      this.error = undefined;
      this.overlay = undefined;
      this.selectedBlock = undefined;
      this.closeCommitDrilldown();
      try {
        const overlay = await revierClient.review.getFileOverlay({
          taskId: this.task.taskId,
          filePath,
          encoding
        });
        if (requestId !== this.overlayRequestId) return false;
        this.overlay = overlay;
        return true;
      } catch (error) {
        if (requestId === this.overlayRequestId) this.error = toErrorMessage(error);
        return false;
      } finally {
        if (requestId === this.overlayRequestId) {
          this.loading = false;
          this.activeOverlayPath = undefined;
        }
      }
    },

    async reloadOverlayEncoding(filePath: string, encoding: TextEncoding): Promise<boolean> {
      if (!this.task || this.overlay?.file.path !== filePath) {
        return false;
      }

      const requestId = ++this.overlayRequestId;
      this.loading = true;
      this.activeOverlayPath = filePath;
      this.error = undefined;
      try {
        const overlay = await revierClient.review.getFileOverlay({
          taskId: this.task.taskId,
          filePath,
          encoding
        });
        if (requestId !== this.overlayRequestId) return false;
        this.overlay = overlay;
        this.selectedBlock = undefined;
        return true;
      } catch (error) {
        if (requestId === this.overlayRequestId) this.error = toErrorMessage(error);
        return false;
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

    async loadCommitOverlay(
      filePath: string,
      commitHash: string,
      encoding?: TextEncoding
    ): Promise<boolean> {
      if (!this.task) {
        this.error = 'No active analysis task';
        return false;
      }

      const requestId = ++this.drilldownRequestId;
      this.drilldownLoading = true;
      this.error = undefined;
      this.selectedCommitHash = commitHash;
      this.activeCommitHash = commitHash;
      this.drilldownOverlay = undefined;
      try {
        const overlay = await revierClient.review.getCommitOverlay({
          taskId: this.task.taskId,
          filePath,
          commitHash,
          encoding
        });
        if (requestId !== this.drilldownRequestId) return false;
        this.drilldownOverlay = overlay;
        return true;
      } catch (error) {
        if (requestId === this.drilldownRequestId) this.error = toErrorMessage(error);
        return false;
      } finally {
        if (requestId === this.drilldownRequestId) {
          this.drilldownLoading = false;
          this.activeCommitHash = undefined;
        }
      }
    },

    async reloadCommitOverlayEncoding(
      filePath: string,
      commitHash: string,
      encoding: TextEncoding
    ): Promise<boolean> {
      if (
        !this.task
        || this.selectedCommitHash !== commitHash
        || this.drilldownOverlay?.file.path !== filePath
      ) {
        return false;
      }

      const requestId = ++this.drilldownRequestId;
      this.drilldownLoading = true;
      this.activeCommitHash = commitHash;
      this.error = undefined;
      try {
        const overlay = await revierClient.review.getCommitOverlay({
          taskId: this.task.taskId,
          filePath,
          commitHash,
          encoding
        });
        if (requestId !== this.drilldownRequestId) return false;
        this.drilldownOverlay = overlay;
        return true;
      } catch (error) {
        if (requestId === this.drilldownRequestId) this.error = toErrorMessage(error);
        return false;
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
        this.authors = await revierClient.review.listAuthors(request);
      } catch (error) {
        this.error = toErrorMessage(error);
      } finally {
        this.authorsLoading = false;
      }
    }
  }
});
