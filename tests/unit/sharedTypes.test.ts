import { createAppError, isRecoverableError } from '../../src/shared/errors';
import type { ReviewFilters, ReviewProject } from '../../src/shared/ipcTypes';

describe('shared contracts', () => {
  it('creates recoverable and blocking app errors', () => {
    const timeout = createAppError('GIT_COMMAND_TIMEOUT', 'Git timed out', true);
    const missingRepo = createAppError('REPOSITORY_NOT_FOUND', 'Repository missing', false);

    expect(isRecoverableError(timeout)).toBe(true);
    expect(isRecoverableError(missingRepo)).toBe(false);
  });

  it('allows project preferences and review filters used by IPC', () => {
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

    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      authorQuery: 'alice',
      messageQuery: 'review',
      globRules: project.preferences.defaultGlobRules
    };

    expect(filters.globRules).toContain('!**/*.test.ts');
  });
});
