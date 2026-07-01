import {
  RustAnalysisClient,
  type RustAnalysisExecutor
} from '../../src/main/analysis/rustAnalysisClient';

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
});
