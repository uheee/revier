vi.mock('electron', () => ({
  BrowserWindow: {
    getAllWindows: () => []
  },
  ipcMain: {
    handle: vi.fn()
  }
}));

import { resolveAnalysisScope } from '../../src/main/ipc/reviewIpc';
import type { ReviewProject } from '../../src/shared/projectTypes';
import type { ChangedFile, ReviewFilters } from '../../src/shared/reviewTypes';

const project: ReviewProject = {
  id: 'project-1',
  name: 'Example',
  repoPath: 'E:/repo/example',
  pinned: false,
  lastOpenedAt: '2026-06-03T00:00:00.000Z',
  preferences: {
    defaultDays: 30,
    defaultGlobRules: []
  }
};

const modifiedFile: ChangedFile = {
  path: 'src/main/index.ts',
  status: 'modified',
  additions: 3,
  deletions: 1,
  isBinary: false,
  isPreviewable: true
};

const readmeFile: ChangedFile = {
  path: 'README.md',
  status: 'modified',
  additions: 1,
  deletions: 0,
  isBinary: false,
  isPreviewable: true
};

describe('reviewIpc', () => {
  it('resolves the selected date range and filters changed files by glob rules', async () => {
    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      startAt: '2026-05-05T00:00:00.000Z',
      endAt: '2026-05-25T00:00:00.000Z',
      globRules: ['src/**/*.ts']
    };
    const listChangedFiles = vi.fn(async () => [modifiedFile, readmeFile]);

    const result = await resolveAnalysisScope({
      project,
      filters,
      git: {
        listCommits: vi.fn(async () => [
          commit('after', '2026-06-01T00:00:00.000Z'),
          commit('head', '2026-05-20T00:00:00.000Z'),
          commit('inside', '2026-05-10T00:00:00.000Z'),
          commit('base', '2026-05-01T00:00:00.000Z')
        ]),
        listChangedFiles
      }
    });

    expect(listChangedFiles).toHaveBeenCalledWith(project.repoPath, 'base', 'head');
    expect(result.range.baseCommit).toBe('base');
    expect(result.range.headCommit).toBe('head');
    expect(result.files).toEqual([modifiedFile]);
  });
});

function commit(hash: string, committedAt: string) {
  return {
    hash,
    shortHash: hash.slice(0, 8),
    authorName: 'A',
    authorEmail: 'a@example.com',
    committedAt,
    subject: hash
  };
}
