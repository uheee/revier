vi.mock('electron', () => ({
  BrowserWindow: {
    getAllWindows: () => []
  },
  ipcMain: {
    handle: vi.fn()
  }
}));

import * as reviewIpc from '../../src/main/ipc/reviewIpc';
import type { ReviewProject } from '../../src/shared/projectTypes';
import type { AnalysisRange, ChangedFile, FileOverlay, ReviewFilters } from '../../src/shared/reviewTypes';

const {
  buildCommitOverlayForTask,
  buildFileOverlayForTask,
  resolveAnalysisScope
} = reviewIpc;

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

  it('prefers Rust file overlay when a Rust client is provided and passes range and filter parameters', async () => {
    const range = analysisRange();
    const rustOverlay = rustFileOverlay(range);
    const getFileOverlay = vi.fn(async () => rustOverlay);
    const input = {
      project,
      file: modifiedFile,
      range,
      filters: {
        projectId: project.id,
        branch: 'main',
        startAt: '2026-05-01T00:00:00.000Z',
        endAt: '2026-05-31T00:00:00.000Z',
        authorKeys: ['a@example.com'],
        authorQuery: 'alice',
        messageQuery: 'feature',
        globRules: ['src/**/*.ts']
      },
      rangeCommits: [commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app')],
      git: overlayGit(),
      rust: { getFileOverlay }
    };

    const overlay = await buildFileOverlayForTask(input);

    expect(getFileOverlay).toHaveBeenCalledWith({
      repoPath: project.repoPath,
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: modifiedFile.path,
      globRules: ['src/**/*.ts'],
      authorKeys: ['a@example.com'],
      authorQuery: 'alice',
      messageQuery: 'feature'
    });
    expect(overlay).toBe(rustOverlay);
  });

  it('applies display commit filters to successful Rust overlay blocks and rows', async () => {
    const range = analysisRange();
    const rustOverlay = rustFileOverlay(range, {
      rows: [
        diffRow('visible-block', 1),
        diffRow('hidden-block', 2)
      ],
      blocks: [
        diffBlock('visible-block', true, 'Alice', 'feature: update app'),
        diffBlock('hidden-block', false, 'Bob', 'chore: update app')
      ]
    });
    const getFileOverlay = vi.fn(async () => rustOverlay);
    const input = {
      project,
      file: modifiedFile,
      range,
      filters: {
        projectId: project.id,
        branch: 'main',
        authorQuery: 'alice',
        messageQuery: 'feature',
        globRules: []
      },
      rangeCommits: [
        commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app'),
        commit('bob', '2026-05-11T00:00:00.000Z', 'Bob', 'chore: update app')
      ],
      git: overlayGit(),
      rust: { getFileOverlay }
    };

    const overlay = await buildFileOverlayForTask(input);

    expect(getFileOverlay).toHaveBeenCalledTimes(1);
    expect(overlay.blocks.map((block) => block.id)).toEqual(['visible-block']);
    expect(overlay.rows?.map((row) => row.blockId)).toEqual(['visible-block', undefined]);
  });

  it('falls back to TypeScript file overlay when Rust overlay returns a recoverable error', async () => {
    const range = analysisRange();
    const rustError = Object.assign(new Error('索引不可用'), { recoverable: true });
    const getFileOverlay = vi.fn(async () => {
      throw rustError;
    });
    const input = {
      project,
      file: modifiedFile,
      range,
      filters: {
        projectId: project.id,
        branch: 'main',
        globRules: []
      },
      rangeCommits: [commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app')],
      git: overlayGit(),
      rust: { getFileOverlay }
    };

    const overlay = await buildFileOverlayForTask(input);

    expect(getFileOverlay).toHaveBeenCalledTimes(1);
    expect(overlay.mode).toBe('range');
    expect(overlay.blocks).toHaveLength(1);
    expect(overlay.rows?.map((row) => row.type)).toEqual(['modified']);
  });

  it('falls back to TypeScript file overlay when Git client methods are prototype methods', async () => {
    const range = analysisRange();
    const rustError = Object.assign(new Error('索引不可用'), { recoverable: true });
    const getFileOverlay = vi.fn(async () => {
      throw rustError;
    });
    const input = {
      project,
      file: modifiedFile,
      range,
      filters: {
        projectId: project.id,
        branch: 'main',
        globRules: []
      },
      rangeCommits: [commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app')],
      git: new PrototypeOverlayGit(),
      rust: { getFileOverlay }
    };

    const overlay = await buildFileOverlayForTask(input);

    expect(getFileOverlay).toHaveBeenCalledTimes(1);
    expect(overlay.mode).toBe('range');
    expect(overlay.blocks).toHaveLength(1);
    expect(overlay.rows?.map((row) => row.type)).toEqual(['modified']);
  });

  it('throws non-recoverable Rust overlay errors without TypeScript fallback', async () => {
    const range = analysisRange();
    const rustError = Object.assign(new Error('Rust 内部错误'), { recoverable: false });
    const getFileOverlay = vi.fn(async () => {
      throw rustError;
    });
    const input = {
      project,
      file: modifiedFile,
      range,
      filters: {
        projectId: project.id,
        branch: 'main',
        globRules: []
      },
      rangeCommits: [commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app')],
      git: overlayGit(),
      rust: { getFileOverlay }
    };

    await expect(buildFileOverlayForTask(input)).rejects.toThrow('Rust 内部错误');
    expect(getFileOverlay).toHaveBeenCalledTimes(1);
  });

  it('does not call Rust overlay for binary or non-previewable files', async () => {
    const getFileOverlay = vi.fn(async () => rustFileOverlay(analysisRange()));
    const input = {
      project,
      file: {
        ...modifiedFile,
        status: 'binary' as const,
        isBinary: true,
        isPreviewable: false
      },
      range: analysisRange(),
      filters: {
        projectId: project.id,
        branch: 'main',
        globRules: []
      },
      rangeCommits: [],
      git: overlayGit(),
      rust: { getFileOverlay }
    };

    const overlay = await buildFileOverlayForTask(input);

    expect(getFileOverlay).not.toHaveBeenCalled();
    expect(overlay.warnings[0].code).toBe('BINARY_FILE');
  });

  it('enables Rust overlay by default unless REVIER_USE_RUST_OVERLAY is 0', () => {
    const isRustOverlayEnabled = (
      reviewIpc as typeof reviewIpc & { isRustOverlayEnabled?: () => boolean }
    ).isRustOverlayEnabled;
    const original = process.env.REVIER_USE_RUST_OVERLAY;
    try {
      delete process.env.REVIER_USE_RUST_OVERLAY;
      expect(isRustOverlayEnabled?.()).toBe(true);

      process.env.REVIER_USE_RUST_OVERLAY = '0';
      expect(isRustOverlayEnabled?.()).toBe(false);

      process.env.REVIER_USE_RUST_OVERLAY = '1';
      expect(isRustOverlayEnabled?.()).toBe(true);
    } finally {
      if (original === undefined) {
        delete process.env.REVIER_USE_RUST_OVERLAY;
      } else {
        process.env.REVIER_USE_RUST_OVERLAY = original;
      }
    }
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

function analysisRange(): AnalysisRange {
  return {
    branch: 'main',
    baseCommit: 'base',
    headCommit: 'head'
  };
}

function overlayGit() {
  return {
    listCommits: vi.fn(),
    listChangedFiles: vi.fn(),
    readFileAtCommit: vi.fn(async (_repoPath: string, commitHash: string) =>
      commitHash === 'base' ? 'const name = "old";\n' : 'const name = "new";\n'
    ),
    showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n')
  };
}

class PrototypeOverlayGit {
  async listCommits() {
    return [];
  }

  async listChangedFiles() {
    return [];
  }

  async readFileAtCommit(_repoPath: string, commitHash: string) {
    return commitHash === 'base' ? 'const name = "old";\n' : 'const name = "new";\n';
  }

  async showFilePatch() {
    return '@@ -1 +1 @@\n-old\n+new\n';
  }
}

function rustFileOverlay(
  range: AnalysisRange,
  options: Partial<Pick<FileOverlay, 'rows' | 'blocks'>> = {}
): FileOverlay {
  const rows = options.rows ?? [diffRow('visible-block', 1)];
  return {
    mode: 'range',
    file: modifiedFile,
    range,
    rows,
    blocks: options.blocks ?? [diffBlock('visible-block', true, 'Alice', 'feature: update app')],
    warnings: []
  };
}

function diffRow(blockId: string, lineNumber: number) {
  return {
    oldLineNumber: lineNumber,
    newLineNumber: lineNumber,
    oldText: 'old',
    newText: 'new',
    type: 'modified' as const,
    blockId
  };
}

function diffBlock(
  id: string,
  matchedByFilter: boolean,
  authorName: string,
  subject: string
) {
  return {
    id,
    oldStart: 1,
    oldEnd: 1,
    newStart: 1,
    newEnd: 1,
    rowStartIndex: id === 'visible-block' ? 0 : 1,
    rowEndIndex: id === 'visible-block' ? 0 : 1,
    changeType: 'modified' as const,
    authors: [{ name: authorName, email: `${authorName.toLowerCase()}@example.com` }],
    rows: [diffRow(id, id === 'visible-block' ? 1 : 2)],
    relatedCommits: [
      {
        hash: authorName.toLowerCase(),
        shortHash: authorName.toLowerCase(),
        authorName,
        authorEmail: `${authorName.toLowerCase()}@example.com`,
        committedAt: '2026-05-10T00:00:00.000Z',
        subject,
        matchedByFilter,
        touchedRanges: [{ oldStart: 1, oldEnd: 1, newStart: 1, newEnd: 1 }]
      }
    ]
  };
}

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
