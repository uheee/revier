import {
  RustAnalysisClient,
  type RustAnalysisExecutor
} from '../../src/main/analysis/rustAnalysisClient';
import type { AttributionMethod } from '../../src/shared/reviewTypes';

describe('RustAnalysisClient', () => {
  it('assembles query-files arguments and parses changed files', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        version: 1,
        range: { baseCommit: 'base', headCommit: 'head' },
        files: [
          {
            path: 'src/app.ts',
            oldPath: null,
            status: 'modified',
            additions: 2,
            deletions: 1,
            isBinary: false,
            isPreviewable: true
          }
        ],
        warnings: []
      }),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    const result = await client.queryFiles({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      authorKeys: ['alice@example.com'],
      messageQuery: 'fix',
      globRules: ['src/**/*.ts']
    });

    expect(executor).toHaveBeenCalledWith('revier-analysis', [
      'index',
      'query-files',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--author',
      'alice@example.com',
      '--message',
      'fix',
      '--glob',
      'src/**/*.ts',
      '--format',
      'json'
    ]);
    expect(result.files).toEqual([
      {
        path: 'src/app.ts',
        oldPath: undefined,
        status: 'modified',
        additions: 2,
        deletions: 1,
        isBinary: false,
        isPreviewable: true
      }
    ]);
  });

  it('marks index exit codes as recoverable fallback errors', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 4,
      stdout: '',
      stderr: '索引不可用：索引文件不存在'
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    await expect(
      client.queryFiles({
        repoPath: 'E:/repo/app',
        baseCommit: 'base',
        headCommit: 'head',
        branch: 'main',
        globRules: []
      })
    ).rejects.toMatchObject({
      recoverable: true,
      code: 'RUST_INDEX_UNAVAILABLE'
    });
  });

  it('maps file-overlay exit code 4 to file not analyzable error', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 4,
      stdout: '',
      stderr: '文件不可分析：二进制文件'
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    await expect(
      client.getFileOverlay({
        repoPath: 'E:/repo/app',
        baseCommit: 'base',
        headCommit: 'head',
        branch: 'main',
        filePath: 'src/app.bin',
        globRules: []
      })
    ).rejects.toMatchObject({
      recoverable: true,
      code: 'RUST_FILE_NOT_ANALYZABLE'
    });
  });

  it('maps trace-block exit code 6 to DuckDB recoverable error', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 6,
      stdout: '',
      stderr: 'DuckDB 查询失败'
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    await expect(
      client.traceBlock({
        repoPath: 'E:/repo/app',
        baseCommit: 'base',
        headCommit: 'head',
        branch: 'main',
        filePath: 'src/app.ts',
        globRules: [],
        blockId: 'block-1'
      })
    ).rejects.toMatchObject({
      recoverable: true,
      code: 'RUST_DUCKDB_ERROR'
    });
  });

  it('assembles file-overlay arguments and normalizes overlay JSON', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        version: 1,
        overlay: {
          mode: 'range',
          file: {
            path: 'src/app.ts',
            oldPath: null,
            status: 'modified',
            additions: 2,
            deletions: 1,
            isBinary: false,
            isPreviewable: true
          },
          range: {
            branch: 'main',
            baseCommit: 'base',
            headCommit: 'head',
            startAt: null,
            endAt: null
          },
          rows: [
            {
              oldLineNumber: null,
              newLineNumber: 1,
              oldText: null,
              newText: 'const value = 1;',
              type: 'added',
              wordChanges: null,
              blockId: 'block-1'
            }
          ],
          blocks: [
            {
              id: 'block-1',
              oldStart: 0,
              oldEnd: 0,
              newStart: 1,
              newEnd: 1,
              rowStartIndex: null,
              rowEndIndex: 0,
              changeType: 'added',
              authors: [{ name: 'Alice', email: null }],
              rows: [
                {
                  oldLineNumber: null,
                  newLineNumber: 1,
                  oldText: null,
                  newText: 'const value = 1;',
                  type: 'added',
                  wordChanges: null,
                  blockId: 'block-1'
                }
              ],
              relatedCommits: [
                {
                  hash: 'abc123',
                  shortHash: 'abc123',
                  authorName: 'Alice',
                  authorEmail: null,
                  committedAt: '2026-01-01T00:00:00Z',
                  subject: '添加 value',
                  matchedByFilter: true,
                  touchedRanges: [
                    {
                      oldStart: null,
                      oldEnd: null,
                      newStart: 1,
                      newEnd: 1
                    }
                  ],
                  attribution: {
                    method: 'blame',
                    viaMergeHashes: []
                  }
                }
              ],
              attribution: {
                confidence: 'precise',
                warnings: []
              }
            }
          ],
          warnings: ['overlay warning']
        },
        warnings: ['top-level warning']
      }),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    const result = await client.getFileOverlay({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: 'src/app.ts',
      globRules: []
    });

    expect(executor).toHaveBeenCalledWith('revier-analysis', [
      'file-overlay',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--file',
      'src/app.ts',
      '--format',
      'json'
    ]);
    expect(result.file.oldPath).toBeUndefined();
    expect(result.rows?.[0].oldLineNumber).toBeUndefined();
    expect(result.blocks[0].rowStartIndex).toBeUndefined();
    expect(result.blocks[0].authors[0].email).toBeUndefined();
    expect(result.blocks[0].relatedCommits[0].authorEmail).toBeUndefined();
    expect(result.blocks[0].relatedCommits[0].touchedRanges[0].oldStart).toBeUndefined();
    expect(result.warnings).toEqual([
      {
        code: 'UNKNOWN',
        message: 'overlay warning',
        recoverable: true,
        detail: undefined
      }
    ]);
  });

  it('passes file-overlay filters and require-index to Rust CLI', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify(buildFileOverlayOutput()),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    await client.getFileOverlay({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: 'src/app.ts',
      globRules: ['src/**/*.ts', 'tests/**/*.ts'],
      authorKeys: ['alice@example.com', 'bob@example.com'],
      authorQuery: '  Alice  ',
      messageQuery: '  fix  ',
      requireIndex: true
    });

    expect(executor).toHaveBeenCalledWith('revier-analysis', [
      'file-overlay',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--file',
      'src/app.ts',
      '--author',
      'alice@example.com',
      '--author',
      'bob@example.com',
      '--author-query',
      'Alice',
      '--message',
      'fix',
      '--glob',
      'src/**/*.ts',
      '--glob',
      'tests/**/*.ts',
      '--require-index',
      '--format',
      'json'
    ]);
  });

  it('assembles trace-block selectors and normalizes trace output', async () => {
    const deletionTraceMethod: AttributionMethod = 'deletion-trace';
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        version: 1,
        file: 'src/app.ts',
        blockId: 'block-1',
        attribution: {
          confidence: 'partial',
          warnings: [
            {
              code: 'DELETION_TRACE_INCOMPLETE',
              message: '部分删除来源无法定位'
            }
          ]
        },
        authors: [{ name: 'Alice', email: null }],
        relatedCommits: [
          {
            hash: 'def456',
            shortHash: 'def456',
            authorName: 'Alice',
            authorEmail: null,
            committedAt: '2026-01-02T00:00:00Z',
            subject: '删除旧实现',
            matchedByFilter: true,
            touchedRanges: [
              {
                oldStart: 4,
                oldEnd: 8,
                newStart: null,
                newEnd: null
              }
            ],
            attribution: {
              method: deletionTraceMethod,
              viaMergeHashes: ['merge123']
            }
          }
        ],
        warnings: ['trace warning']
      }),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    const byBlockId = await client.traceBlock({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: 'src/app.ts',
      globRules: [],
      blockId: 'block-1'
    });

    await client.traceBlock({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: 'src/app.ts',
      globRules: [],
      oldStart: 4,
      oldEnd: 8,
      newStart: 9,
      newEnd: 12
    });

    expect(executor).toHaveBeenNthCalledWith(1, 'revier-analysis', [
      'trace-block',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--file',
      'src/app.ts',
      '--block-id',
      'block-1',
      '--format',
      'json'
    ]);
    expect(executor).toHaveBeenNthCalledWith(2, 'revier-analysis', [
      'trace-block',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--file',
      'src/app.ts',
      '--old-start',
      '4',
      '--old-end',
      '8',
      '--new-start',
      '9',
      '--new-end',
      '12',
      '--format',
      'json'
    ]);
    expect(byBlockId.blockId).toBe('block-1');
    expect(byBlockId.authors[0].email).toBeUndefined();
    expect(byBlockId.relatedCommits[0].authorEmail).toBeUndefined();
    expect(byBlockId.relatedCommits[0].touchedRanges[0].newStart).toBeUndefined();
    expect(byBlockId.relatedCommits[0].attribution?.method).toBe('deletion-trace');
    expect(byBlockId.warnings).toEqual([
      {
        code: 'UNKNOWN',
        message: 'trace warning',
        recoverable: true,
        detail: undefined
      }
    ]);
  });
});

function buildFileOverlayOutput() {
  return {
    version: 1,
    overlay: {
      mode: 'range',
      file: {
        path: 'src/app.ts',
        oldPath: null,
        status: 'modified',
        additions: 1,
        deletions: 0,
        isBinary: false,
        isPreviewable: true
      },
      range: {
        branch: 'main',
        baseCommit: 'base',
        headCommit: 'head'
      },
      rows: [],
      blocks: [],
      warnings: []
    },
    warnings: []
  };
}
