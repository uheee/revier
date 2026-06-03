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
import { attachAttribution } from '../analysis/attributionEngine';
import { createGlobMatcher } from '../analysis/globRules';
import { buildFileOverlayBlocks } from '../analysis/overlayEngine';
import { parsePatchTouchedRanges } from '../analysis/patchRanges';
import { selectRangeCommits, type ResolvedCommitRange } from '../analysis/rangeResolver';
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
}

interface ResolveAnalysisScopeInput {
  project: ReviewProject;
  filters: ReviewFilters;
  git: AnalysisGitClient;
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
}

export async function resolveAnalysisScope({
  project,
  filters,
  git
}: ResolveAnalysisScopeInput): Promise<ResolvedAnalysisScope> {
  const commits = await git.listCommits(project.repoPath, filters.branch);
  const range = selectRangeCommits(commits, filters);
  const matcher = createGlobMatcher(filters.globRules);
  const rangeCommits = mapRangeCommits(commits, range);
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
  git
}: BuildFileOverlayInput): Promise<FileOverlay> {
  if (file.isBinary || !file.isPreviewable) {
    return {
      file,
      range,
      blocks: [],
      warnings: [
        createAppError('BINARY_FILE', 'Binary file is not previewable', true, file.path)
      ]
    };
  }

  const oldText = await git.readFileAtCommit(
    project.repoPath,
    range.baseCommit,
    file.oldPath ?? file.path
  );
  const newText = await git.readFileAtCommit(project.repoPath, range.headCommit, file.path);
  const relatedCommits = await buildRelatedCommits(project.repoPath, file, rangeCommits, filters, git);
  const blocks = attachAttribution(
    buildFileOverlayBlocks({ file, oldText, newText }),
    relatedCommits
  );

  return {
    file,
    range,
    blocks: hasDisplayCommitFilters(filters)
      ? blocks.filter((block) => block.relatedCommits.some((commit) => commit.matchedByFilter))
      : blocks,
    warnings: []
  };
}

export function registerReviewIpc(projectStore: JsonProjectStore): void {
  const git = new GitService();
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

    const scope = await resolveAnalysisScope({ project, filters, git });
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

function hasDisplayCommitFilters(filters: ReviewFilters): boolean {
  return Boolean(filters.authorQuery?.trim() || filters.messageQuery?.trim());
}

function commitMatchesDisplayFilters(commit: GitCommitSummary, filters: ReviewFilters): boolean {
  return (
    matchesQuery(`${commit.authorName} ${commit.authorEmail ?? ''}`, filters.authorQuery) &&
    matchesQuery(commit.subject, filters.messageQuery)
  );
}

function matchesQuery(value: string, query?: string): boolean {
  const normalized = query?.trim().toLowerCase();
  return !normalized || value.toLowerCase().includes(normalized);
}
