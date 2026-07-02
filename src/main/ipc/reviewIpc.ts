import { BrowserWindow, ipcMain } from 'electron';
import { createAppError } from '../../shared/errors';
import { ipcChannels } from '../../shared/ipcTypes';
import type { ReviewProject } from '../../shared/projectTypes';
import type {
  AnalysisRange,
  ChangedFile,
  FileOverlay,
  RelatedCommit,
  ReviewFilters
} from '../../shared/reviewTypes';
import { AnalysisTaskManager } from '../analysis/analysisTaskManager';
import { buildFileOverlay } from '../../analysis-core';
import { createGlobMatcher } from '../analysis/globRules';
import { buildFileOverlayDiff } from '../analysis/overlayEngine';
import { parsePatchTouchedRanges } from '../analysis/patchRanges';
import { selectRangeCommits, type ResolvedCommitRange } from '../analysis/rangeResolver';
import { RustAnalysisClient, type QueryFilesResult, type RustFileOverlayRequest } from '../analysis/rustAnalysisClient';
import { GitService } from '../git/gitService';
import type { GitCommitSummary } from '../git/gitTypes';
import type { JsonProjectStore } from '../projects/projectStore';

interface AnalysisGitClient {
  listCommits(repoPath: string, branch: string): Promise<GitCommitSummary[]>;
  listChangedFiles(
    repoPath: string,
    baseCommit: string,
    headCommit: string
  ): Promise<ChangedFile[]>;
  readFileAtCommit(repoPath: string, commit: string, filePath: string): Promise<string>;
  showFilePatch(repoPath: string, commit: string, filePath: string): Promise<string>;
  blameFileRange?(repoPath: string, commit: string, filePath: string, startLine: number, endLine: number): Promise<import('../../analysis-core').BlameLine[]>;
  listParents?(repoPath: string, commitHash: string): Promise<string[]>;
  getCommit?(repoPath: string, commitHash: string): Promise<RelatedCommit | undefined>;
}

interface CommitOverlayGitClient extends AnalysisGitClient {
  getFirstParent(repoPath: string, commitHash: string): Promise<string>;
}

interface RustAnalysisQueryClient {
  queryFiles(request: {
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
  }): Promise<QueryFilesResult>;
}

interface RustAnalysisOverlayClient {
  getFileOverlay(request: RustFileOverlayRequest): Promise<FileOverlay>;
}

interface ResolveAnalysisScopeInput {
  project: ReviewProject;
  filters: ReviewFilters;
  git: AnalysisGitClient;
  rust?: RustAnalysisQueryClient;
}

interface ResolvedAnalysisScope {
  range: ResolvedCommitRange;
  files: ChangedFile[];
  rangeCommits: GitCommitSummary[];
}

interface BuildFileOverlayInput {
  project: ReviewProject;
  file: ChangedFile;
  range: AnalysisRange;
  rangeCommits: GitCommitSummary[];
  filters: ReviewFilters;
  git: AnalysisGitClient;
  rust?: RustAnalysisOverlayClient;
}

interface BuildCommitOverlayInput {
  project: ReviewProject;
  file: ChangedFile;
  range: AnalysisRange;
  commitHash: string;
  rangeCommits: GitCommitSummary[];
  git: CommitOverlayGitClient;
}

export async function resolveAnalysisScope({
  project,
  filters,
  git,
  rust
}: ResolveAnalysisScopeInput): Promise<ResolvedAnalysisScope> {
  const commits = await git.listCommits(project.repoPath, filters.branch);
  const range = selectRangeCommits(commits, filters);
  const matcher = createGlobMatcher(filters.globRules);
  const rangeCommits = mapRangeCommits(commits, range);

  if (hasDisplayCommitFilters(filters) && rust) {
    try {
      const result = await rust.queryFiles({
        repoPath: project.repoPath,
        baseCommit: range.baseCommit,
        headCommit: range.headCommit,
        branch: filters.branch,
        startAt: range.startAt,
        endAt: range.endAt,
        authorKeys: filters.authorKeys,
        authorQuery: filters.authorQuery,
        messageQuery: filters.messageQuery,
        globRules: filters.globRules
      });
      return { range, files: result.files, rangeCommits };
    } catch (error) {
      if (!isRecoverableRustError(error)) {
        throw error;
      }
    }
  }

  let files = (await git.listChangedFiles(project.repoPath, range.baseCommit, range.headCommit)).filter(
    (file) => matcher(file.path)
  );

  if (hasDisplayCommitFilters(filters)) {
    files = await filterFilesByMatchingCommits(project.repoPath, files, rangeCommits, filters, git);
  }

  return { range, files, rangeCommits };
}

export async function buildFileOverlayForTask({
  project,
  file,
  range,
  rangeCommits,
  filters,
  git,
  rust
}: BuildFileOverlayInput): Promise<FileOverlay> {
  if (file.isBinary || !file.isPreviewable) {
    return {
      mode: 'range',
      file,
      range,
      rows: [],
      blocks: [],
      warnings: [
        createAppError('BINARY_FILE', 'Binary file is not previewable', true, file.path)
      ]
    };
  }

  if (rust) {
    try {
      const overlay = await rust.getFileOverlay({
        repoPath: project.repoPath,
        baseCommit: range.baseCommit,
        headCommit: range.headCommit,
        branch: range.branch,
        filePath: file.path,
        globRules: filters.globRules,
        authorKeys: filters.authorKeys,
        authorQuery: filters.authorQuery,
        messageQuery: filters.messageQuery
      });
      return applyDisplayCommitFilters(overlay, filters);
    } catch (error) {
      if (!isRecoverableRustError(error)) {
        throw error;
      }
    }
  }

  const overlay = await buildTypescriptFileOverlay({
    project,
    file,
    range,
    rangeCommits,
    filters,
    git
  });
  return applyDisplayCommitFilters(overlay, filters);
}

async function buildTypescriptFileOverlay({
  project,
  file,
  range,
  rangeCommits,
  filters,
  git
}: Omit<BuildFileOverlayInput, 'rust'>): Promise<FileOverlay> {
  return buildFileOverlay({
    repoPath: project.repoPath,
    file,
    range,
    rangeCommits,
    filters,
    git: {
      ...git,
      blameFileRange: git.blameFileRange?.bind(git) ?? (async () => []),
      listParents: git.listParents?.bind(git) ?? (async () => []),
      getCommit: git.getCommit?.bind(git) ?? (async () => undefined)
    }
  });
}

function applyDisplayCommitFilters(overlay: FileOverlay, filters: ReviewFilters): FileOverlay {
  const visibleBlocks = hasDisplayCommitFilters(filters)
    ? overlay.blocks.filter((block) => block.relatedCommits.some((commit) => commit.matchedByFilter))
    : overlay.blocks;
  const maskedRows = maskFilteredRows(overlay.rows ?? [], visibleBlocks);
  const rowsUnchanged = Boolean(overlay.rows) && maskedRows.every((row, index) => row === overlay.rows?.[index]);
  const blocksUnchanged = visibleBlocks.length === overlay.blocks.length;

  if (rowsUnchanged && blocksUnchanged) {
    return overlay;
  }

  return {
    ...overlay,
    rows: maskedRows,
    blocks: visibleBlocks
  };
}

export async function buildCommitOverlayForTask({
  project,
  file,
  range,
  commitHash,
  rangeCommits,
  git
}: BuildCommitOverlayInput): Promise<FileOverlay> {
  const commit = rangeCommits.find((item) => item.hash === commitHash);
  if (!commit) {
    throw new Error('该提交不在当前筛选范围内');
  }

  const patch = await showPatchForAnyFilePath(project.repoPath, commitHash, file, git);
  if (!patch.trim()) {
    throw new Error('该提交未修改当前文件');
  }

  const parentHash = await git.getFirstParent(project.repoPath, commitHash);
  const oldText = await git.readFileAtCommit(project.repoPath, parentHash, file.oldPath ?? file.path);
  const newText = await git.readFileAtCommit(project.repoPath, commitHash, file.path);
  const diff = buildFileOverlayDiff({ file, oldText, newText });
  const relatedCommit: RelatedCommit = {
    hash: commit.hash,
    shortHash: commit.shortHash,
    authorName: commit.authorName,
    authorEmail: commit.authorEmail,
    committedAt: commit.committedAt,
    subject: commit.subject,
    matchedByFilter: false,
    touchedRanges: parsePatchTouchedRanges(patch)
  };

  return {
    mode: 'commit',
    file,
    range,
    rows: diff.rows,
    blocks: diff.blocks.map((block) => ({
      ...block,
      authors: [{ name: commit.authorName, email: commit.authorEmail }],
      relatedCommits: [relatedCommit]
    })),
    warnings: [],
    commit: relatedCommit,
    parentHash
  };
}

export function registerReviewIpc(projectStore: JsonProjectStore): void {
  const git = new GitService();
  const rust = new RustAnalysisClient();
  const taskManager = new AnalysisTaskManager();
  const filesByTask = new Map<string, ChangedFile[]>();
  const rangesByTask = new Map<string, AnalysisRange>();
  const rangeCommitsByTask = new Map<string, GitCommitSummary[]>();
  const projectsByTask = new Map<string, ReviewProject>();
  const filtersByTask = new Map<string, ReviewFilters>();

  taskManager.onUpdate((snapshot) => {
    for (const window of BrowserWindow.getAllWindows()) {
      window.webContents.send(ipcChannels.reviewTaskUpdated, snapshot);
    }
  });

  ipcMain.handle(ipcChannels.reviewStartAnalysis, async (_event, filters: ReviewFilters) => {
    const task = taskManager.createTask(filters.projectId);
    filtersByTask.set(task.taskId, filters);
    taskManager.markRunning(task.taskId, 'loadChangedFiles', 'Loading changed files');

    const project = (await projectStore.list()).find((item) => item.id === filters.projectId);
    if (!project) {
      throw new Error('Project not found');
    }

    const scope = await resolveAnalysisScope({ project, filters, git, rust });
    filesByTask.set(task.taskId, scope.files);
    rangeCommitsByTask.set(task.taskId, scope.rangeCommits);
    projectsByTask.set(task.taskId, project);
    rangesByTask.set(task.taskId, {
      branch: scope.range.branch,
      baseCommit: scope.range.baseCommit,
      headCommit: scope.range.headCommit,
      startAt: scope.range.startAt,
      endAt: scope.range.endAt
    });
    taskManager.markCompleted(task.taskId);
    return taskManager.getTask(task.taskId);
  });

  ipcMain.handle(ipcChannels.reviewCancelAnalysis, (_event, taskId: string) => {
    taskManager.markCancelled(taskId);
  });
  ipcMain.handle(ipcChannels.reviewGetTask, (_event, taskId: string) =>
    taskManager.getTask(taskId)
  );
  ipcMain.handle(ipcChannels.reviewListChangedFiles, (_event, taskId: string) =>
    filesByTask.get(taskId) ?? []
  );
  ipcMain.handle(ipcChannels.reviewListAuthors, async (_event, request) => {
    const project = (await projectStore.list()).find((item) => item.id === request.projectId);
    if (!project) {
      throw new Error('Project not found');
    }

    return git.listAuthors(project.repoPath, request.branch, request.startAt, request.endAt);
  });
  ipcMain.handle(ipcChannels.reviewGetFileOverlay, async (_event, request): Promise<FileOverlay> => {
    const filters = filtersByTask.get(request.taskId);
    const range = rangesByTask.get(request.taskId);
    const project = projectsByTask.get(request.taskId);
    const rangeCommits = rangeCommitsByTask.get(request.taskId) ?? [];
    const file = filesByTask.get(request.taskId)?.find((item) => item.path === request.filePath);
    if (!filters || !range || !project || !file) {
      throw new Error('Overlay request is not associated with an active task file');
    }

    return buildFileOverlayForTask({
      project,
      file,
      range,
      rangeCommits,
      filters,
      git,
      rust: isRustOverlayEnabled() ? rust : undefined
    });
  });
  ipcMain.handle(ipcChannels.reviewGetCommitOverlay, async (_event, request): Promise<FileOverlay> => {
    const range = rangesByTask.get(request.taskId);
    const project = projectsByTask.get(request.taskId);
    const rangeCommits = rangeCommitsByTask.get(request.taskId) ?? [];
    const file = filesByTask.get(request.taskId)?.find((item) => item.path === request.filePath);
    if (!range || !project || !file) {
      throw new Error('Commit overlay request is not associated with an active task file');
    }

    return buildCommitOverlayForTask({
      project,
      file,
      range,
      commitHash: request.commitHash,
      rangeCommits,
      git
    });
  });
}

function mapRangeCommits(
  commits: GitCommitSummary[],
  range: ResolvedCommitRange
): GitCommitSummary[] {
  const commitsByHash = new Map(commits.map((commit) => [commit.hash, commit]));
  return range.rangeCommits
    .map((commit) => commitsByHash.get(commit.hash))
    .filter((commit): commit is GitCommitSummary => commit !== undefined);
}

function maskFilteredRows(
  rows: import('../../shared/reviewTypes').SideBySideDiffRow[],
  blocks: import('../../shared/reviewTypes').DiffBlock[]
) {
  const visibleBlockIds = new Set(blocks.map((block) => block.id));
  return rows.map((row) =>
    row.blockId && !visibleBlockIds.has(row.blockId) ? { ...row, blockId: undefined } : row
  );
}

async function filterFilesByMatchingCommits(
  repoPath: string,
  files: ChangedFile[],
  rangeCommits: GitCommitSummary[],
  filters: ReviewFilters,
  git: AnalysisGitClient
): Promise<ChangedFile[]> {
  const matchingCommits = rangeCommits.filter((commit) => commitMatchesDisplayFilters(commit, filters));
  const visibleFiles: ChangedFile[] = [];
  for (const file of files) {
    // TODO: Refine this file-level prefilter with cached block attribution so files without
    // matching visible blocks are hidden before the user opens them.
    if (await fileTouchedByAnyCommit(repoPath, file, matchingCommits, git)) {
      visibleFiles.push(file);
    }
  }
  return visibleFiles;
}

async function fileTouchedByAnyCommit(
  repoPath: string,
  file: ChangedFile,
  commits: GitCommitSummary[],
  git: AnalysisGitClient
): Promise<boolean> {
  for (const commit of commits) {
    if ((await touchedRangesForFile(repoPath, commit.hash, file, git)).length > 0) {
      return true;
    }
  }
  return false;
}

async function buildRelatedCommits(
  repoPath: string,
  file: ChangedFile,
  rangeCommits: GitCommitSummary[],
  filters: ReviewFilters,
  git: AnalysisGitClient
): Promise<RelatedCommit[]> {
  const commits: RelatedCommit[] = [];
  const markMatches = hasDisplayCommitFilters(filters);
  for (const commit of rangeCommits) {
    const touchedRanges = await touchedRangesForFile(repoPath, commit.hash, file, git);
    if (touchedRanges.length === 0) {
      continue;
    }

    commits.push({
      hash: commit.hash,
      shortHash: commit.shortHash,
      authorName: commit.authorName,
      authorEmail: commit.authorEmail,
      committedAt: commit.committedAt,
      subject: commit.subject,
      matchedByFilter: markMatches ? commitMatchesDisplayFilters(commit, filters) : false,
      touchedRanges
    });
  }
  return commits;
}

async function touchedRangesForFile(
  repoPath: string,
  commitHash: string,
  file: ChangedFile,
  git: AnalysisGitClient
) {
  const paths = [...new Set([file.path, file.oldPath].filter((path): path is string => Boolean(path)))];
  const ranges = [];
  for (const path of paths) {
    const patch = await git.showFilePatch(repoPath, commitHash, path);
    ranges.push(...parsePatchTouchedRanges(patch));
  }
  return ranges;
}

async function showPatchForAnyFilePath(
  repoPath: string,
  commitHash: string,
  file: ChangedFile,
  git: Pick<AnalysisGitClient, 'showFilePatch'>
): Promise<string> {
  const paths = [...new Set([file.path, file.oldPath].filter((path): path is string => Boolean(path)))];
  for (const path of paths) {
    const patch = await git.showFilePatch(repoPath, commitHash, path);
    if (patch.trim()) {
      return patch;
    }
  }
  return '';
}

function hasDisplayCommitFilters(filters: ReviewFilters): boolean {
  return Boolean((filters.authorKeys?.length ?? 0) > 0 || filters.authorQuery?.trim() || filters.messageQuery?.trim());
}

export function isRustOverlayEnabled(): boolean {
  return process.env.REVIER_USE_RUST_OVERLAY === '1';
}

function isRecoverableRustError(error: unknown): boolean {
  return Boolean(
    error &&
      typeof error === 'object' &&
      'recoverable' in error &&
      (error as { recoverable?: unknown }).recoverable
  );
}

function commitMatchesDisplayFilters(commit: GitCommitSummary, filters: ReviewFilters): boolean {
  return (
    matchesAuthorKeys(commit, filters.authorKeys ?? []) &&
    matchesQuery(`${commit.authorName} ${commit.authorEmail ?? ''}`, filters.authorQuery) &&
    matchesQuery(commit.subject, filters.messageQuery)
  );
}

function matchesAuthorKeys(commit: GitCommitSummary, authorKeys: string[]): boolean {
  if (authorKeys.length === 0) {
    return true;
  }

  const normalizedKeys = new Set(authorKeys.map((key) => key.toLowerCase()));
  return normalizedKeys.has(authorKey(commit.authorName, commit.authorEmail));
}

function matchesQuery(value: string, query?: string): boolean {
  const normalized = query?.trim().toLowerCase();
  return !normalized || value.toLowerCase().includes(normalized);
}

function authorKey(name: string, email?: string): string {
  return (email?.trim() || name.trim()).toLowerCase();
}
