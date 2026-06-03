import { createAppError, isRecoverableError } from '../../src/shared/errors';
import type { ReviewFilters, ReviewProject } from '../../src/shared/ipcTypes';
import type {
  AuthorFilterOption,
  CommitOverlayRequest,
  FileOverlay,
  SideBySideDiffRow
} from '../../src/shared/reviewTypes';

describe('shared contracts', () => {
  it('creates recoverable and blocking app errors', () => {
    const timeout = createAppError('GIT_COMMAND_TIMEOUT', 'Git timed out', true);
    const missingRepo = createAppError('REPOSITORY_NOT_FOUND', 'Repository missing', false);

    expect(isRecoverableError(timeout)).toBe(true);
    expect(isRecoverableError(missingRepo)).toBe(false);
  });

  it('allows project preferences and structured review filters used by IPC', () => {
    const project: ReviewProject = {
      id: 'project-1',
      name: 'demo',
      repoPath: 'E:/repos/demo',
      pinned: true,
      preferences: {
        defaultBranch: 'main',
        defaultDays: 30,
        defaultGlobRules: ['src/**/*.ts', '!**/*.test.ts']
      }
    };

    const alice: AuthorFilterOption = {
      key: 'alice@example.com',
      name: 'Alice',
      email: 'alice@example.com',
      commitCount: 3
    };

    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      authorKeys: [alice.key],
      messageQuery: 'review',
      globRules: project.preferences.defaultGlobRules
    };

    expect(filters.authorKeys).toEqual(['alice@example.com']);
    expect(filters.globRules).toContain('!**/*.test.ts');
  });

  it('supports full-file overlay rows and commit drilldown requests', () => {
    const row: SideBySideDiffRow = {
      oldLineNumber: 1,
      newLineNumber: 1,
      oldText: 'old',
      newText: 'new',
      type: 'modified',
      blockId: 'block-1'
    };

    const overlay: FileOverlay = {
      mode: 'range',
      file: {
        path: 'src/app.ts',
        status: 'modified',
        additions: 1,
        deletions: 1,
        isBinary: false,
        isPreviewable: true
      },
      range: {
        branch: 'main',
        baseCommit: 'base',
        headCommit: 'head'
      },
      rows: [row],
      blocks: [
        {
          id: 'block-1',
          oldStart: 1,
          oldEnd: 1,
          newStart: 1,
          newEnd: 1,
          rowStartIndex: 0,
          rowEndIndex: 0,
          changeType: 'modified',
          authors: [],
          rows: [row],
          relatedCommits: []
        }
      ],
      warnings: []
    };

    const request: CommitOverlayRequest = {
      taskId: 'task-1',
      filePath: 'src/app.ts',
      commitHash: 'abc123'
    };

    expect(overlay.rows?.[0].blockId).toBe('block-1');
    expect(request.commitHash).toBe('abc123');
  });
});
