import { spawn } from 'node:child_process';
import { posix, win32 } from 'node:path';
import { app } from 'electron';
import { createAppError, type AppError } from '../../shared/errors';
import type {
  AnalysisRange,
  AuthorSummary,
  ChangedFile,
  DiffBlock,
  FileOverlay,
  RelatedCommit,
  SideBySideDiffRow,
  TouchedRange,
  WordChange
} from '../../shared/reviewTypes';

export interface RustAnalysisExecutorResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export type RustAnalysisExecutor = (
  binaryPath: string,
  args: string[]
) => Promise<RustAnalysisExecutorResult>;

export interface RustBinaryPathContext {
  env: NodeJS.ProcessEnv;
  cwd: string;
  platform: NodeJS.Platform;
  isPackaged: boolean;
  resourcesPath?: string;
}

export interface QueryFilesRequest {
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  branch: string;
  startAt?: string;
  endAt?: string;
  authorKeys?: string[];
  authorQuery?: string;
  messageQuery?: string;
  globRules: string[];
}

export interface QueryFilesResult {
  files: ChangedFile[];
  warnings: string[];
}

export interface RustFileOverlayRequest {
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  branch: string;
  filePath: string;
  globRules: string[];
  authorKeys?: string[];
  authorQuery?: string;
  messageQuery?: string;
  requireIndex?: boolean;
}

export interface RustTraceBlockRequest extends RustFileOverlayRequest {
  blockId?: string;
  oldStart?: number;
  oldEnd?: number;
  newStart?: number;
  newEnd?: number;
}

export interface RustTraceBlockResult {
  version: number;
  file: string;
  blockId: string;
  attribution?: DiffBlock['attribution'];
  authors: AuthorSummary[];
  relatedCommits: RelatedCommit[];
  warnings: AppError[];
}

export class RustAnalysisError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly recoverable: boolean
  ) {
    super(message);
  }
}

export class RustAnalysisClient {
  private readonly binaryPath: string;
  private readonly executor: RustAnalysisExecutor;

  constructor(options: { binaryPath?: string; executor?: RustAnalysisExecutor } = {}) {
    this.binaryPath = options.binaryPath ?? defaultBinaryPath();
    this.executor = options.executor ?? spawnExecutor;
  }

  async queryFiles(request: QueryFilesRequest): Promise<QueryFilesResult> {
    const args = buildQueryFilesArgs(request);
    const result = await this.executor(this.binaryPath, args);
    if (result.exitCode !== 0) {
      throw mapRustError(result.exitCode, result.stderr, 'query-files');
    }

    const parsed = JSON.parse(result.stdout) as {
      files: Array<ChangedFile & { oldPath?: string | null }>;
      warnings?: string[];
    };
    return {
      files: parsed.files.map((file) => ({
        ...file,
        oldPath: file.oldPath ?? undefined
      })),
      warnings: parsed.warnings ?? []
    };
  }

  async getFileOverlay(request: RustFileOverlayRequest): Promise<FileOverlay> {
    const args = buildFileOverlayArgs(request);
    const result = await this.executor(this.binaryPath, args);
    if (result.exitCode !== 0) {
      throw mapRustError(result.exitCode, result.stderr, 'file-overlay');
    }

    const parsed = JSON.parse(result.stdout) as RawFileOverlayCommandOutput;
    return normalizeFileOverlay(parsed.overlay, parsed.warnings);
  }

  async traceBlock(request: RustTraceBlockRequest): Promise<RustTraceBlockResult> {
    const args = buildTraceBlockArgs(request);
    const result = await this.executor(this.binaryPath, args);
    if (result.exitCode !== 0) {
      throw mapRustError(result.exitCode, result.stderr, 'trace-block');
    }

    const parsed = JSON.parse(result.stdout) as RawTraceBlockOutput;
    return {
      version: parsed.version,
      file: parsed.file,
      blockId: parsed.blockId,
      attribution: nullToUndefined(parsed.attribution),
      authors: parsed.authors.map(normalizeAuthor),
      relatedCommits: parsed.relatedCommits.map(normalizeRelatedCommit),
      warnings: normalizeWarnings(parsed.warnings)
    };
  }
}

export function buildQueryFilesArgs(request: QueryFilesRequest): string[] {
  const args = [
    'index',
    'query-files',
    ...buildRepoRangeArgs(request)
  ];

  appendAuthorAndMessageArgs(args, request);
  if (request.startAt) {
    args.push('--since', request.startAt);
  }
  if (request.endAt) {
    args.push('--until', request.endAt);
  }
  appendGlobArgs(args, request);
  args.push('--format', 'json');
  return args;
}

function buildFileOverlayArgs(request: RustFileOverlayRequest): string[] {
  const args = [
    'file-overlay',
    ...buildOverlayCommonArgs(request),
    '--file',
    request.filePath
  ];
  appendOverlayFiltersAndFormat(args, request);
  return args;
}

function buildTraceBlockArgs(request: RustTraceBlockRequest): string[] {
  const args = [
    'trace-block',
    ...buildOverlayCommonArgs(request),
    '--file',
    request.filePath
  ];
  if (request.blockId !== undefined) {
    args.push('--block-id', request.blockId);
  }
  if (request.oldStart !== undefined) {
    args.push('--old-start', String(request.oldStart));
  }
  if (request.oldEnd !== undefined) {
    args.push('--old-end', String(request.oldEnd));
  }
  if (request.newStart !== undefined) {
    args.push('--new-start', String(request.newStart));
  }
  if (request.newEnd !== undefined) {
    args.push('--new-end', String(request.newEnd));
  }
  appendOverlayFiltersAndFormat(args, request);
  return args;
}

function buildRepoRangeArgs(request: Pick<QueryFilesRequest, 'repoPath' | 'baseCommit' | 'headCommit' | 'branch'>): string[] {
  return [
    '--repo',
    request.repoPath,
    '--base',
    request.baseCommit,
    '--head',
    request.headCommit,
    '--branch',
    request.branch
  ];
}

function buildOverlayCommonArgs(request: RustFileOverlayRequest): string[] {
  return buildRepoRangeArgs(request);
}

function appendOverlayFiltersAndFormat(args: string[], request: RustFileOverlayRequest): void {
  appendAuthorAndMessageArgs(args, request);
  appendGlobArgs(args, request);
  if (request.requireIndex) {
    args.push('--require-index');
  }
  args.push('--format', 'json');
}

function appendAuthorAndMessageArgs(
  args: string[],
  request: Pick<QueryFilesRequest, 'authorKeys' | 'authorQuery' | 'messageQuery'>
): void {
  for (const author of request.authorKeys ?? []) {
    args.push('--author', author);
  }

  const authorQuery = request.authorQuery?.trim();
  if (authorQuery) {
    args.push('--author-query', authorQuery);
  }

  const messageQuery = request.messageQuery?.trim();
  if (messageQuery) {
    args.push('--message', messageQuery);
  }
}

function appendGlobArgs(
  args: string[],
  request: Pick<QueryFilesRequest, 'globRules'>
): void {
  for (const glob of request.globRules) {
    args.push('--glob', glob);
  }
}

type RustAnalysisCommand = 'query-files' | 'file-overlay' | 'trace-block';

function mapRustError(
  exitCode: number,
  stderr: string,
  command: RustAnalysisCommand
): RustAnalysisError {
  const message = stderr.trim() || `Rust 分析进程退出码 ${exitCode}`;
  if (command === 'query-files' && [4, 5, 6].includes(exitCode)) {
    return new RustAnalysisError('RUST_INDEX_UNAVAILABLE', message, true);
  }
  if ((command === 'file-overlay' || command === 'trace-block') && exitCode === 4) {
    return new RustAnalysisError('RUST_FILE_NOT_ANALYZABLE', message, true);
  }
  if ((command === 'file-overlay' || command === 'trace-block') && exitCode === 5) {
    return new RustAnalysisError('RUST_INDEX_UNAVAILABLE', message, true);
  }
  if ((command === 'file-overlay' || command === 'trace-block') && exitCode === 6) {
    return new RustAnalysisError('RUST_DUCKDB_ERROR', message, true);
  }
  if (exitCode === 2 || exitCode === 3) {
    return new RustAnalysisError('RUST_QUERY_FAILED', message, true);
  }
  return new RustAnalysisError('RUST_INTERNAL_ERROR', message, false);
}

function defaultBinaryPath(): string {
  return resolveRustAnalysisBinaryPath({
    env: process.env,
    cwd: process.cwd(),
    platform: process.platform,
    isPackaged: app.isPackaged,
    resourcesPath: process.resourcesPath
  });
}

export function resolveRustAnalysisBinaryPath(context: RustBinaryPathContext): string {
  const binaryName = context.platform === 'win32' ? 'revier-analysis.exe' : 'revier-analysis';
  const joinPath = context.platform === 'win32' ? win32.join : posix.join;
  if (context.env.REVIER_ANALYSIS_BIN !== undefined) {
    return context.env.REVIER_ANALYSIS_BIN;
  }

  if (context.isPackaged) {
    if (!context.resourcesPath) {
      throw new RustAnalysisError(
        'RUST_BINARY_UNAVAILABLE',
        '打包态缺少 Electron resourcesPath',
        false
      );
    }
    return joinPath(context.resourcesPath, 'revier-analysis', binaryName);
  }

  return joinPath(context.cwd, 'target', 'debug', binaryName);
}

type RawChangedFile = Omit<ChangedFile, 'oldPath'> & {
  oldPath?: string | null;
};

type RawAnalysisRange = Omit<AnalysisRange, 'startAt' | 'endAt'> & {
  startAt?: string | null;
  endAt?: string | null;
};

type RawAuthorSummary = Omit<AuthorSummary, 'email'> & {
  email?: string | null;
};

type RawTouchedRange = {
  oldStart?: number | null;
  oldEnd?: number | null;
  newStart?: number | null;
  newEnd?: number | null;
};

type RawRelatedCommit = Omit<RelatedCommit, 'authorEmail' | 'touchedRanges' | 'attribution'> & {
  authorEmail?: string | null;
  touchedRanges: RawTouchedRange[];
  attribution?: RelatedCommit['attribution'] | null;
};

type RawWordChange = Omit<WordChange, 'added' | 'removed'> & {
  added?: boolean | null;
  removed?: boolean | null;
};

type RawSideBySideDiffRow = Omit<
  SideBySideDiffRow,
  'oldLineNumber' | 'newLineNumber' | 'oldText' | 'newText' | 'wordChanges' | 'blockId'
> & {
  oldLineNumber?: number | null;
  newLineNumber?: number | null;
  oldText?: string | null;
  newText?: string | null;
  wordChanges?: RawWordChange[] | null;
  blockId?: string | null;
};

type RawDiffBlock = Omit<
  DiffBlock,
  'rowStartIndex' | 'rowEndIndex' | 'authors' | 'rows' | 'relatedCommits' | 'attribution'
> & {
  rowStartIndex?: number | null;
  rowEndIndex?: number | null;
  authors: RawAuthorSummary[];
  rows: RawSideBySideDiffRow[];
  relatedCommits: RawRelatedCommit[];
  attribution?: DiffBlock['attribution'] | null;
};

type RawFileOverlay = Omit<
  FileOverlay,
  'file' | 'range' | 'rows' | 'blocks' | 'warnings' | 'commit' | 'parentHash'
> & {
  file: RawChangedFile;
  range: RawAnalysisRange;
  rows?: RawSideBySideDiffRow[] | null;
  blocks: RawDiffBlock[];
  warnings?: Array<string | AppError> | null;
  commit?: RawRelatedCommit | null;
  parentHash?: string | null;
};

interface RawFileOverlayCommandOutput {
  version: number;
  overlay: RawFileOverlay;
  warnings?: Array<string | AppError> | null;
}

interface RawTraceBlockOutput {
  version: number;
  file: string;
  blockId: string;
  attribution?: DiffBlock['attribution'] | null;
  authors: RawAuthorSummary[];
  relatedCommits: RawRelatedCommit[];
  warnings?: Array<string | AppError> | null;
}

function normalizeFileOverlay(
  overlay: RawFileOverlay,
  commandWarnings?: Array<string | AppError> | null
): FileOverlay {
  return {
    ...overlay,
    file: normalizeChangedFile(overlay.file),
    range: normalizeRange(overlay.range),
    rows: nullToUndefined(overlay.rows)?.map(normalizeDiffRow),
    blocks: overlay.blocks.map(normalizeDiffBlock),
    warnings: normalizeWarnings(overlay.warnings ?? commandWarnings),
    commit: overlay.commit ? normalizeRelatedCommit(overlay.commit) : undefined,
    parentHash: nullToUndefined(overlay.parentHash)
  };
}

function normalizeChangedFile(file: RawChangedFile): ChangedFile {
  return {
    ...file,
    oldPath: nullToUndefined(file.oldPath)
  };
}

function normalizeRange(range: RawAnalysisRange): AnalysisRange {
  return {
    ...range,
    startAt: nullToUndefined(range.startAt),
    endAt: nullToUndefined(range.endAt)
  };
}

function normalizeAuthor(author: RawAuthorSummary): AuthorSummary {
  return {
    ...author,
    email: nullToUndefined(author.email)
  };
}

function normalizeDiffBlock(block: RawDiffBlock): DiffBlock {
  return {
    ...block,
    rowStartIndex: nullToUndefined(block.rowStartIndex),
    rowEndIndex: nullToUndefined(block.rowEndIndex),
    authors: block.authors.map(normalizeAuthor),
    rows: block.rows.map(normalizeDiffRow),
    relatedCommits: block.relatedCommits.map(normalizeRelatedCommit),
    attribution: nullToUndefined(block.attribution)
  };
}

function normalizeDiffRow(row: RawSideBySideDiffRow): SideBySideDiffRow {
  return {
    ...row,
    oldLineNumber: nullToUndefined(row.oldLineNumber),
    newLineNumber: nullToUndefined(row.newLineNumber),
    oldText: nullToUndefined(row.oldText),
    newText: nullToUndefined(row.newText),
    wordChanges: nullToUndefined(row.wordChanges)?.map(normalizeWordChange),
    blockId: nullToUndefined(row.blockId)
  };
}

function normalizeWordChange(change: RawWordChange): WordChange {
  return {
    ...change,
    added: nullToUndefined(change.added),
    removed: nullToUndefined(change.removed)
  };
}

function normalizeRelatedCommit(commit: RawRelatedCommit): RelatedCommit {
  return {
    ...commit,
    authorEmail: nullToUndefined(commit.authorEmail),
    touchedRanges: commit.touchedRanges.map(normalizeTouchedRange),
    attribution: nullToUndefined(commit.attribution)
  };
}

function normalizeTouchedRange(range: RawTouchedRange): TouchedRange {
  return {
    oldStart: nullToUndefined(range.oldStart),
    oldEnd: nullToUndefined(range.oldEnd),
    newStart: nullToUndefined(range.newStart),
    newEnd: nullToUndefined(range.newEnd)
  };
}

function normalizeWarnings(warnings?: Array<string | AppError> | null): AppError[] {
  return (warnings ?? []).map((warning) =>
    typeof warning === 'string' ? createAppError('UNKNOWN', warning, true) : warning
  );
}

function nullToUndefined<T>(value: T | null | undefined): T | undefined {
  return value ?? undefined;
}

function spawnExecutor(binaryPath: string, args: string[]): Promise<RustAnalysisExecutorResult> {
  return new Promise((resolve) => {
    const child = spawn(binaryPath, args, { windowsHide: true });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on('data', (chunk: Buffer) => stderr.push(chunk));
    child.on('error', (error) => {
      resolve({ exitCode: 4, stdout: '', stderr: error.message });
    });
    child.on('close', (exitCode) => {
      resolve({
        exitCode: exitCode ?? 10,
        stdout: Buffer.concat(stdout).toString('utf8'),
        stderr: Buffer.concat(stderr).toString('utf8')
      });
    });
  });
}
