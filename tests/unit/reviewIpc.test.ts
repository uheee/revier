vi.mock('electron', () => ({
  BrowserWindow: {
    getAllWindows: () => []
  },
  ipcMain: {
    handle: vi.fn()
  }
}));

import {
  buildCommitOverlayForTask,
  buildFileOverlayForTask,
  resolveAnalysisScope
} from '../../src/main/ipc/reviewIpc';
import type { ReviewProject } from '../../src/shared/projectTypes';
import type { AnalysisRange, ChangedFile, ReviewFilters } from '../../src/shared/reviewTypes';

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
        listChangedFiles,
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn()
      }
    });

    expect(listChangedFiles).toHaveBeenCalledWith(project.repoPath, 'base', 'head');
    expect(result.range.baseCommit).toBe('base');
    expect(result.range.headCommit).toBe('head');
    expect(result.files).toEqual([modifiedFile]);
  });

  it('keeps changed files touched by commits matching author and message filters', async () => {
    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['a@example.com'],
      messageQuery: 'feature',
      globRules: ['src/**/*.ts']
    };

    const result = await resolveAnalysisScope({
      project,
      filters,
      git: {
        listCommits: vi.fn(async () => [
          commit('head', '2026-05-20T00:00:00.000Z', 'Bob', 'fix bug'),
          commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app'),
          commit('base', '2026-05-01T00:00:00.000Z', 'Base', 'base')
        ]),
        listChangedFiles: vi.fn(async () => [
          modifiedFile,
          { ...modifiedFile, path: 'src/other.ts' }
        ]),
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn(async (_repoPath, commitHash, filePath) =>
          commitHash === 'alice' && filePath === modifiedFile.path ? '@@ -1 +1 @@\n-old\n+new\n' : ''
        )
      }
    });

    expect(result.files).toEqual([modifiedFile]);
  });

  it('uses Rust query-files when display commit filters are present', async () => {
    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['alice@example.com'],
      globRules: ['src/**/*.ts']
    };
    const rustQueryFiles = vi.fn(async () => ({
      files: [modifiedFile],
      warnings: []
    }));
    const listChangedFiles = vi.fn(async () => [readmeFile]);

    const result = await resolveAnalysisScope({
      project,
      filters,
      git: {
        listCommits: vi.fn(async () => [
          commit('head', '2026-05-20T00:00:00.000Z', 'Alice', 'feature'),
          commit('base', '2026-05-01T00:00:00.000Z', 'Base', 'base')
        ]),
        listChangedFiles,
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn()
      },
      rust: {
        queryFiles: rustQueryFiles
      }
    });

    expect(rustQueryFiles).toHaveBeenCalledWith({
      repoPath: project.repoPath,
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['alice@example.com'],
      authorQuery: undefined,
      messageQuery: undefined,
      globRules: ['src/**/*.ts']
    });
    expect(listChangedFiles).not.toHaveBeenCalled();
    expect(result.files).toEqual([modifiedFile]);
  });

  it('falls back to TypeScript filtering when Rust query-files is recoverable', async () => {
    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['a@example.com'],
      globRules: ['src/**/*.ts']
    };
    const rustError = Object.assign(new Error('索引不可用'), { recoverable: true });

    const result = await resolveAnalysisScope({
      project,
      filters,
      git: {
        listCommits: vi.fn(async () => [
          commit('head', '2026-05-20T00:00:00.000Z', 'Bob', 'fix bug'),
          commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app'),
          commit('base', '2026-05-01T00:00:00.000Z', 'Base', 'base')
        ]),
        listChangedFiles: vi.fn(async () => [modifiedFile]),
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n')
      },
      rust: {
        queryFiles: vi.fn(async () => {
          throw rustError;
        })
      }
    });

    expect(result.files).toEqual([modifiedFile]);
  });

  it('builds file overlay from real base and head content with related authors', async () => {
    const range: AnalysisRange = {
      branch: 'main',
      baseCommit: 'base',
      headCommit: 'head'
    };
    const overlay = await buildFileOverlayForTask({
      project,
      file: modifiedFile,
      range,
      filters: {
        projectId: project.id,
        branch: 'main',
        authorKeys: ['a@example.com'],
        globRules: []
      },
      rangeCommits: [commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app')],
      git: {
        listCommits: vi.fn(),
        listChangedFiles: vi.fn(),
        readFileAtCommit: vi.fn(async (_repoPath, commitHash) =>
          commitHash === 'base' ? 'const name = "old";\n' : 'const name = "new";\n'
        ),
        showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n')
      }
    });

    expect(overlay.blocks).toHaveLength(1);
    expect(overlay.mode).toBe('range');
    expect(overlay.rows?.map((row) => row.type)).toEqual(['modified']);
    expect(overlay.blocks[0].rowStartIndex).toBe(0);
    expect(overlay.blocks[0].rowEndIndex).toBe(0);
    expect(overlay.blocks[0].authors).toEqual([{ name: 'Alice', email: 'a@example.com' }]);
    expect(overlay.blocks[0].relatedCommits[0].matchedByFilter).toBe(true);
  });

  it('builds a commit overlay from the commit first parent to the commit', async () => {
    const range: AnalysisRange = {
      branch: 'main',
      baseCommit: 'base',
      headCommit: 'head'
    };
    const related = commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app');
    const overlay = await buildCommitOverlayForTask({
      project,
      file: modifiedFile,
      range,
      commitHash: related.hash,
      rangeCommits: [related],
      git: {
        listCommits: vi.fn(),
        listChangedFiles: vi.fn(),
        readFileAtCommit: vi.fn(async (_repoPath, commitHash) =>
          commitHash === 'parent' ? 'const name = "old";\n' : 'const name = "new";\n'
        ),
        showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n'),
        getFirstParent: vi.fn(async () => 'parent')
      }
    });

    expect(overlay.mode).toBe('commit');
    expect(overlay.parentHash).toBe('parent');
    expect(overlay.commit?.hash).toBe('alice');
    expect(overlay.blocks).toHaveLength(1);
  });
});

function commit(hash: string, committedAt: string, authorName = 'A', subject = hash) {
  return {
    hash,
    shortHash: hash.slice(0, 8),
    authorName,
    authorEmail: 'a@example.com',
    committedAt,
    subject
  };
}
