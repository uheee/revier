import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GitService } from '../../src/main/git/gitService';
import { commitFiles, initTestRepo, type TestRepo } from '../fixtures/gitRepo';

describe('medium repository performance', () => {
  let repo: TestRepo;

  beforeEach(async () => {
    repo = await initTestRepo(await mkdtemp(join(tmpdir(), 'revier-perf-')));
  });

  afterEach(async () => {
    await rm(repo.path, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  });

  it('lists changed files for a 120-commit multi-author repository in under 10 seconds', async () => {
    const fileCount = 30;
    const baseCommit = (await repo.git.revparse(['HEAD'])).trim();

    for (let index = 0; index < 120; index += 1) {
      const fileIndex = index % fileCount;
      const authorName = index % 2 === 0 ? 'Alice' : 'Bob';
      await commitFiles(repo, {
        message: `update file ${fileIndex}`,
        authorName,
        authorEmail: `${authorName.toLowerCase()}@example.com`,
        date: new Date(Date.UTC(2026, 4, 1, 0, index)).toISOString(),
        files: {
          [`src/file-${fileIndex}.ts`]: `export const value${fileIndex} = ${index};\n`
        }
      });
    }

    const headCommit = (await repo.git.revparse(['HEAD'])).trim();
    const startedAt = performance.now();
    const files = await new GitService().listChangedFiles(repo.path, baseCommit, headCommit);
    const elapsed = performance.now() - startedAt;

    expect(files).toHaveLength(fileCount);
    expect(elapsed).toBeLessThan(10_000);
  }, 120_000);
});
