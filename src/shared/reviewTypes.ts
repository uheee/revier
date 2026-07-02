import type { AppError } from './errors';
import type { ProjectId } from './projectTypes';

export type TaskId = string;

export interface ReviewFilters {
  projectId: ProjectId;
  branch: string;
  startAt?: string;
  endAt?: string;
  authorKeys?: string[];
  authorQuery?: string;
  messageQuery?: string;
  globRules: string[];
}

export interface AnalysisRange {
  branch: string;
  baseCommit: string;
  headCommit: string;
  startAt?: string;
  endAt?: string;
}

export type AnalysisTaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export type AnalysisStage =
  | 'readRepository'
  | 'resolveRange'
  | 'loadCommits'
  | 'loadChangedFiles'
  | 'ready';

export interface AnalysisTaskSnapshot {
  taskId: TaskId;
  projectId: ProjectId;
  status: AnalysisTaskStatus;
  stage: AnalysisStage;
  progress?: number;
  message?: string;
  error?: AppError;
}

export interface ChangedFile {
  path: string;
  oldPath?: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed' | 'binary';
  additions: number;
  deletions: number;
  isBinary: boolean;
  isPreviewable: boolean;
}

export interface FileOverlayRequest {
  taskId: TaskId;
  filePath: string;
}

export interface AuthorSummary {
  name: string;
  email?: string;
}

export interface AuthorFilterOption extends AuthorSummary {
  key: string;
  commitCount: number;
}

export type AttributionConfidence = 'precise' | 'inferred' | 'partial';

export type AttributionMethod = 'blame' | 'merge-trace' | 'patch-inference' | 'deletion-trace';

export type AttributionWarningCode =
  | 'BLAME_UNAVAILABLE'
  | 'MERGE_TRACE_AMBIGUOUS'
  | 'PATH_HISTORY_INCOMPLETE'
  | 'DELETION_TRACE_INCOMPLETE';

export interface AttributionWarning {
  code: AttributionWarningCode;
  message: string;
}

export interface BlockAttributionSummary {
  confidence: AttributionConfidence;
  warnings: AttributionWarning[];
}

export interface RelatedCommitAttribution {
  method: AttributionMethod;
  viaMergeHashes: string[];
}

export interface ReviewAuthorOptionsRequest {
  projectId: ProjectId;
  branch: string;
  startAt?: string;
  endAt?: string;
}

export interface CommitOverlayRequest {
  taskId: TaskId;
  filePath: string;
  commitHash: string;
}

export interface TouchedRange {
  oldStart?: number;
  oldEnd?: number;
  newStart?: number;
  newEnd?: number;
}

export interface WordChange {
  value: string;
  added?: boolean;
  removed?: boolean;
}

export interface RelatedCommit {
  hash: string;
  shortHash: string;
  authorName: string;
  authorEmail?: string;
  committedAt: string;
  subject: string;
  matchedByFilter: boolean;
  touchedRanges: TouchedRange[];
  attribution?: RelatedCommitAttribution;
}

export interface SideBySideDiffRow {
  oldLineNumber?: number;
  newLineNumber?: number;
  oldText?: string;
  newText?: string;
  type: 'context' | 'added' | 'deleted' | 'modified';
  wordChanges?: WordChange[];
  blockId?: string;
}

export interface DiffBlock {
  id: string;
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
  rowStartIndex?: number;
  rowEndIndex?: number;
  changeType: 'added' | 'deleted' | 'modified';
  authors: AuthorSummary[];
  rows: SideBySideDiffRow[];
  relatedCommits: RelatedCommit[];
  attribution?: BlockAttributionSummary;
}

export interface FileOverlay {
  mode?: 'range' | 'commit';
  file: ChangedFile;
  range: AnalysisRange;
  rows?: SideBySideDiffRow[];
  blocks: DiffBlock[];
  warnings: AppError[];
  commit?: RelatedCommit;
  parentHash?: string;
}
