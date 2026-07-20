import { defineStore } from 'pinia';
import { toErrorMessage } from '../api/errors';
import { revierClient } from '../api/revierClient';
import { addNotification } from '../composables/useNotifications';
import {
  mergeDiffBlockAttributions,
  toDiffBlockRanges,
  type MonacoDiffBlocksPayload
} from '../editor/monacoDiffBlocks';
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
  diffComputationState: 'computing' | 'attributing' | 'ready' | 'empty' | 'failed';
  diffBlocksSignature?: string;
  attributionRequestId: number;
  analysisRequestId: number;
  overlayRequestId: number;
  drilldownRequestId: number;
  authorsRequestId: number;
  notifiedFailedTaskId?: string;
  pendingAnalysis?: {
    generation: number;
    snapshots: AnalysisTaskSnapshot[];
  };
  processedTerminal?: {
    taskId: string;
    status: 'completed' | 'failed' | 'cancelled';
  };
}

export const MAX_PENDING_ANALYSIS_TASKS = 32;

function taskStatusRank(status: AnalysisTaskSnapshot['status']): number {
  if (status === 'pending') return 0;
  if (status === 'running') return 1;
  return 2;
}

function mergeConfirmedSnapshot(
  response: AnalysisTaskSnapshot,
  buffered?: AnalysisTaskSnapshot
): AnalysisTaskSnapshot {
  if (!buffered) return response;
  const responseRank = taskStatusRank(response.status);
  const bufferedRank = taskStatusRank(buffered.status);
  if (bufferedRank >= responseRank) return buffered;
  return response;
}

type TerminalAnalysisStatus = 'completed' | 'failed' | 'cancelled';

function isTerminalStatus(
  status: AnalysisTaskSnapshot['status']
): status is TerminalAnalysisStatus {
  return status === 'completed' || status === 'failed' || status === 'cancelled';
}

function notifyReviewError(title: string, message: string): void {
  addNotification({ type: 'error', title, message, source: 'Review' });
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
    diffComputationState: 'computing',
    diffBlocksSignature: undefined,
    attributionRequestId: 0,
    analysisRequestId: 0,
    overlayRequestId: 0,
    drilldownRequestId: 0,
    authorsRequestId: 0,
    notifiedFailedTaskId: undefined,
    pendingAnalysis: undefined,
    processedTerminal: undefined
  }),
  actions: {
    async start(filters: ReviewFilters): Promise<void> {
      const requestId = ++this.analysisRequestId;
      this.pendingAnalysis = { generation: requestId, snapshots: [] };
      this.error = undefined;
      this.overlay = undefined;
      this.selectedBlock = undefined;
      this.invalidateAttribution();
      this.files = [];
      this.cancelOverlay();
      this.closeCommitDrilldown();
      this.loading = true;
      try {
        const task = await revierClient.review.startAnalysis(filters);
        if (requestId !== this.analysisRequestId) return;
        const bufferedSnapshot = this.pendingAnalysis?.generation === requestId
          ? this.pendingAnalysis.snapshots.find((snapshot) => snapshot.taskId === task.taskId)
          : undefined;
        const effectiveSnapshot = mergeConfirmedSnapshot(task, bufferedSnapshot);
        this.pendingAnalysis = undefined;
        if (
          this.task?.taskId === effectiveSnapshot.taskId &&
          isTerminalStatus(this.task.status) &&
          !isTerminalStatus(effectiveSnapshot.status)
        ) {
          return;
        }
        if (this.task?.taskId !== effectiveSnapshot.taskId) {
          this.processedTerminal = undefined;
        }
        this.task = effectiveSnapshot;
        if (isTerminalStatus(effectiveSnapshot.status)) {
          await this.handleTaskUpdate(effectiveSnapshot);
        }
      } catch (error) {
        if (requestId === this.analysisRequestId) {
          this.pendingAnalysis = undefined;
          const message = toErrorMessage(error);
          this.error = message;
          this.loading = false;
          notifyReviewError(
            '分析启动失败',
            `${filters.projectId} / ${filters.branch ?? 'HEAD'}：${message}`
          );
        }
      }
    },

    async handleTaskUpdate(snapshot: AnalysisTaskSnapshot): Promise<void> {
      if (this.pendingAnalysis?.generation === this.analysisRequestId) {
        const existingIndex = this.pendingAnalysis.snapshots.findIndex(
          (candidate) => candidate.taskId === snapshot.taskId
        );
        if (existingIndex !== -1) {
          this.pendingAnalysis.snapshots.splice(existingIndex, 1);
        }
        this.pendingAnalysis.snapshots.push(snapshot);
        if (this.pendingAnalysis.snapshots.length > MAX_PENDING_ANALYSIS_TASKS) {
          this.pendingAnalysis.snapshots.shift();
        }
        return;
      }
      if (!this.task || snapshot.taskId !== this.task.taskId) {
        return;
      }
      if (this.task && isTerminalStatus(this.task.status) && !isTerminalStatus(snapshot.status)) {
        return;
      }
      if (this.task?.status === 'cancelled' && snapshot.status === 'failed') {
        return;
      }

      if (
        isTerminalStatus(snapshot.status)
        && this.processedTerminal?.taskId === snapshot.taskId
        && this.processedTerminal.status === snapshot.status
      ) {
        return;
      }

      this.task = snapshot;
      if (isTerminalStatus(snapshot.status)) {
        this.processedTerminal = { taskId: snapshot.taskId, status: snapshot.status };
      }
      if (snapshot.status === 'completed') {
        const requestId = this.analysisRequestId;
        try {
          const files = await revierClient.review.listChangedFiles(snapshot.taskId);
          if (requestId !== this.analysisRequestId) return;
          if (this.task?.taskId !== snapshot.taskId) return;
          this.files = files;
          this.error = undefined;
        } catch (error) {
          if (requestId === this.analysisRequestId && this.task?.taskId === snapshot.taskId) {
            const message = toErrorMessage(error);
            this.error = message;
            notifyReviewError('变更文件加载失败', `${snapshot.taskId}：${message}`);
          }
        } finally {
          if (requestId === this.analysisRequestId && this.task?.taskId === snapshot.taskId) {
            this.loading = false;
          }
        }
        return;
      }

      if (snapshot.status === 'failed') {
        const message = snapshot.error?.message ?? '分析任务失败';
        this.error = message;
        if (this.notifiedFailedTaskId !== snapshot.taskId) {
          this.notifiedFailedTaskId = snapshot.taskId;
          notifyReviewError('分析任务失败', `${snapshot.taskId}：${message}`);
        }
        this.loading = false;
        return;
      }

      if (snapshot.status === 'cancelled') {
        this.loading = false;
      }
    },

    async cancelAnalysis(): Promise<void> {
      const taskId = this.task?.taskId;
      const requestId = ++this.analysisRequestId;
      this.pendingAnalysis = undefined;
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
        this.processedTerminal = { taskId: this.task.taskId, status: 'cancelled' };
      }
      if (taskId) {
        try {
          await revierClient.review.cancelAnalysis(taskId);
        } catch (error) {
          if (
            requestId === this.analysisRequestId
            && this.task?.taskId === taskId
            && this.task.status === 'cancelled'
          ) {
            const message = toErrorMessage(error);
            this.error = message;
            notifyReviewError('取消分析失败', `${taskId}：${message}`);
          }
        }
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
      this.invalidateAttribution();
      this.closeCommitDrilldown();
      try {
        const overlay = await revierClient.review.getFileOverlay({
          taskId: this.task.taskId,
          filePath,
          encoding
        });
        if (requestId !== this.overlayRequestId) return false;
        this.overlay = overlay;
        this.diffComputationState = 'computing';
        this.diffBlocksSignature = undefined;
        return true;
      } catch (error) {
        if (requestId === this.overlayRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          notifyReviewError('文件差异加载失败', `${filePath}：${message}`);
        }
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
        this.invalidateAttribution();
        this.diffComputationState = 'computing';
        return true;
      } catch (error) {
        if (requestId === this.overlayRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          notifyReviewError('文件编码重载失败', `${filePath}（${encoding}）：${message}`);
        }
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
      this.invalidateAttribution();
      this.loading = false;
      this.activeOverlayPath = undefined;
    },

    selectBlock(block?: DiffBlock): void {
      this.selectedBlock = block;
    },

    invalidateAttribution(): void {
      ++this.attributionRequestId;
      this.diffBlocksSignature = undefined;
      this.diffComputationState = 'computing';
    },

    acceptDiffBlocks(payload: MonacoDiffBlocksPayload, expectedContextKey: string | number): void {
      if (!this.overlay || payload.contextKey !== expectedContextKey) {
        return;
      }
      if (payload.signature === this.diffBlocksSignature) {
        return;
      }
      this.diffBlocksSignature = payload.signature;
      this.overlay = { ...this.overlay, blocks: payload.blocks };
      this.selectedBlock = this.selectedBlock
        ? payload.blocks.find((block) => block.id === this.selectedBlock?.id)
        : undefined;
      if (payload.blocks.length === 0) {
        this.diffComputationState = 'empty';
        return;
      }
      this.diffComputationState = 'attributing';
      void this.loadBlockAttribution(payload.signature);
    },

    async loadBlockAttribution(signature: string): Promise<void> {
      if (!this.task || !this.overlay || this.overlay.blocks.length === 0) {
        return;
      }
      const requestId = ++this.attributionRequestId;
      const taskId = this.task.taskId;
      const filePath = this.overlay.file.path;
      const resolvedEncoding = this.overlay.resolvedEncoding;
      const blocks = [...this.overlay.blocks];
      try {
        const result = await revierClient.review.attributeBlocks({
          taskId,
          filePath,
          resolvedEncoding,
          blocks: toDiffBlockRanges(blocks)
        });
        if (requestId !== this.attributionRequestId) return;
        if (!this.overlay || this.task?.taskId !== taskId) return;
        if (
          this.overlay.file.path !== filePath
          || this.overlay.resolvedEncoding !== resolvedEncoding
          || this.diffBlocksSignature !== signature
          || result.resolvedEncoding !== resolvedEncoding
        ) {
          return;
        }
        const mergedBlocks = mergeDiffBlockAttributions(this.overlay.blocks, result.attributions);
        const warnings = [...this.overlay.warnings];
        for (const warning of result.warnings) {
          if (!warnings.some((item) => item.code === warning.code && item.message === warning.message && item.detail === warning.detail)) {
            warnings.push(warning);
          }
        }
        this.overlay = { ...this.overlay, blocks: mergedBlocks, warnings };
        this.selectedBlock = this.selectedBlock
          ? mergedBlocks.find((block) => block.id === this.selectedBlock?.id)
          : undefined;
        this.diffComputationState = 'ready';
      } catch (error) {
        if (requestId !== this.attributionRequestId) return;
        const message = toErrorMessage(error);
        this.diffComputationState = 'failed';
        notifyReviewError('变更块归因失败', `${this.overlay?.file.path ?? filePath}：${message}`);
      }
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
        if (requestId === this.drilldownRequestId) {
          this.selectedCommitHash = undefined;
          const message = toErrorMessage(error);
          this.error = message;
          notifyReviewError('提交差异加载失败', `${filePath} @ ${commitHash}：${message}`);
        }
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
        if (requestId === this.drilldownRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          notifyReviewError(
            '提交编码重载失败',
            `${filePath} @ ${commitHash}（${encoding}）：${message}`
          );
        }
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
      const requestId = ++this.authorsRequestId;
      this.authorsLoading = true;
      this.error = undefined;
      try {
        const authors = await revierClient.review.listAuthors(request);
        if (requestId !== this.authorsRequestId) return;
        this.authors = authors;
      } catch (error) {
        if (requestId === this.authorsRequestId) {
          const message = toErrorMessage(error);
          this.error = message;
          notifyReviewError(
            '作者筛选加载失败',
            `${request.projectId} / ${request.branch ?? 'HEAD'}：${message}`
          );
        }
      } finally {
        if (requestId === this.authorsRequestId) {
          this.authorsLoading = false;
        }
      }
    }
  }
});
