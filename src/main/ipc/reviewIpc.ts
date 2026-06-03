import { BrowserWindow, ipcMain } from 'electron';
import { ipcChannels } from '../../shared/ipcTypes';
import type { ReviewProject } from '../../shared/projectTypes';
import type {
  AnalysisRange,
  ChangedFile,
  FileOverlay,
  ReviewFilters
} from '../../shared/reviewTypes';
import { AnalysisTaskManager } from '../analysis/analysisTaskManager';
import { createGlobMatcher } from '../analysis/globRules';
import { buildFileOverlayBlocks } from '../analysis/overlayEngine';
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
}

interface ResolveAnalysisScopeInput {
  project: ReviewProject;
  filters: ReviewFilters;
  git: AnalysisGitClient;
}

interface ResolvedAnalysisScope {
  range: ResolvedCommitRange;
  files: ChangedFile[];
}

export async function resolveAnalysisScope({
  project,
  filters,
  git
}: ResolveAnalysisScopeInput): Promise<ResolvedAnalysisScope> {
  const commits = await git.listCommits(project.repoPath, filters.branch);
  const range = selectRangeCommits(commits, filters);
  const matcher = createGlobMatcher(filters.globRules);
  const files = (await git.listChangedFiles(project.repoPath, range.baseCommit, range.headCommit)).filter(
    (file) => matcher(file.path)
  );

  return { range, files };
}

export function registerReviewIpc(projectStore: JsonProjectStore): void {
  const git = new GitService();
  const taskManager = new AnalysisTaskManager();
  const filesByTask = new Map<string, ChangedFile[]>();
  const rangesByTask = new Map<string, AnalysisRange>();
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
    const file = filesByTask.get(request.taskId)?.find((item) => item.path === request.filePath);
    if (!filters || !range || !file) {
      throw new Error('Overlay request is not associated with an active task file');
    }

    // TODO: Task 11 replaces this empty placeholder with real base/head file content.
    return {
      file,
      range,
      blocks: buildFileOverlayBlocks({ file, oldText: '', newText: '' }),
      warnings: []
    };
  });
}
