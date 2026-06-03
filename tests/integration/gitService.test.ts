import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GitService } from '../../src/main/git/gitService';
import { commitFiles, initTestRepo } from '../fixtures/gitRepo';

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

  it('lists unique authors for a branch', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-git-authors-'));

    try {
      const repo = await initTestRepo(dir);
      await commitFiles(repo, {
        message: 'feat: alice',
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        files: { 'src/a.ts': 'export const a = 1;\n' }
      });
      await commitFiles(repo, {
        message: 'fix: alice again',
        authorName: 'Alice',
        authorEmail: 'alice@example.com',
        files: { 'src/a.ts': 'export const a = 2;\n' }
      });
      await commitFiles(repo, {
        message: 'feat: bob',
        authorName: 'Bob',
        authorEmail: 'bob@example.com',
        files: { 'src/b.ts': 'export const b = 1;\n' }
      });

      const service = new GitService();
      const authors = await service.listAuthors(dir, 'HEAD');

      expect(authors).toContainEqual({
        key: 'alice@example.com',
        name: 'Alice',
        email: 'alice@example.com',
        commitCount: 2
      });
      expect(authors).toContainEqual({
        key: 'bob@example.com',
        name: 'Bob',
        email: 'bob@example.com',
        commitCount: 1
      });
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('resolves the first parent of a commit', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'revier-git-parent-'));

    try {
      const repo = await initTestRepo(dir);
      const child = await commitFiles(repo, {
        message: 'feat: child',
        files: { 'src/app.ts': 'export const value = 1;\n' }
      });
      const service = new GitService();
      const parent = await service.getFirstParent(dir, child);

      expect(parent).toHaveLength(40);
      expect(parent).not.toBe(child);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
