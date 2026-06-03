import { mkdir, mkdtemp, rm } from 'node:fs/promises';
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

  it('normalizes repository subdirectories to the Git root', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-git-'));

    try {
      await initTestRepo(dir);
      await mkdir(join(dir, 'src', 'nested'), { recursive: true });

      const service = new GitService();
      const validation = await service.validateRepository(join(dir, 'src', 'nested'));

      expect(validation.valid).toBe(true);
      expect(validation.repoPath.replaceAll('\\', '/')).toBe(dir.replaceAll('\\', '/'));
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('returns a recoverable validation result for non-git directories', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-not-git-'));

    try {
      const service = new GitService();
      const validation = await service.validateRepository(dir);

      expect(validation.valid).toBe(false);
      expect(validation.error).toBe('请选择一个 Git 仓库目录');
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
