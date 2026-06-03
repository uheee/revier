import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GitService } from '../../src/main/git/gitService';
import { initTestRepo } from '../fixtures/gitRepo';

describe('GitService repository validation', () => {
  it('validates a real Git repository and lists branches', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-git-'));

    try {
      await initTestRepo(dir);
      const service = new GitService();

      const validation = await service.validateRepository(dir);
      const branches = await service.listBranches(dir);

      expect(validation.valid).toBe(true);
      expect(branches.some((branch) => branch.current)).toBe(true);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
