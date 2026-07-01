import { spawn } from 'node:child_process';
import { join } from 'node:path';
import type { ChangedFile } from '../../shared/reviewTypes';

export interface RustAnalysisExecutorResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export type RustAnalysisExecutor = (
  binaryPath: string,
  args: string[]
) => Promise<RustAnalysisExecutorResult>;

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
      throw mapRustError(result.exitCode, result.stderr);
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
}

export function buildQueryFilesArgs(request: QueryFilesRequest): string[] {
  const args = [
    'index',
    'query-files',
    '--repo',
    request.repoPath,
    '--base',
    request.baseCommit,
    '--head',
    request.headCommit,
    '--branch',
    request.branch
  ];

  for (const author of request.authorKeys ?? []) {
    args.push('--author', author);
  }
  if (request.authorQuery?.trim()) {
    args.push('--author-query', request.authorQuery.trim());
  }
  if (request.messageQuery?.trim()) {
    args.push('--message', request.messageQuery.trim());
  }
  if (request.startAt) {
    args.push('--since', request.startAt);
  }
  if (request.endAt) {
    args.push('--until', request.endAt);
  }
  for (const glob of request.globRules) {
    args.push('--glob', glob);
  }
  args.push('--format', 'json');
  return args;
}

function mapRustError(exitCode: number, stderr: string): RustAnalysisError {
  const message = stderr.trim() || `Rust 分析进程退出码 ${exitCode}`;
  if ([4, 5, 6].includes(exitCode)) {
    return new RustAnalysisError('RUST_INDEX_UNAVAILABLE', message, true);
  }
  if (exitCode === 2 || exitCode === 3) {
    return new RustAnalysisError('RUST_QUERY_FAILED', message, true);
  }
  return new RustAnalysisError('RUST_INTERNAL_ERROR', message, false);
}

function defaultBinaryPath(): string {
  return (
    process.env.REVIER_ANALYSIS_BIN ??
    join(
      process.cwd(),
      'target',
      'debug',
      process.platform === 'win32' ? 'revier-analysis.exe' : 'revier-analysis'
    )
  );
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
