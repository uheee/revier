import simpleGit from 'simple-git';
import { resolve } from 'node:path';
import { normalizeGitPath, parseNameStatus, parseNumstat } from '../analysis/changedFiles';
import type { GitBranch, RepositoryValidation } from '../../shared/projectTypes';
import type { AuthorFilterOption, ChangedFile } from '../../shared/reviewTypes';
import type { GitCommitSummary } from './gitTypes';

const invalidRepositoryMessage = '请选择一个 Git 仓库目录';

export class GitService {
  async validateRepository(repoPath: string): Promise<RepositoryValidation> {
    const git = simpleGit(repoPath);
    try {
      const isRepo = await git.checkIsRepo();
      if (!isRepo) {
        return { valid: false, repoPath, error: invalidRepositoryMessage };
      }

      const root = normalizeRepositoryPath((await git.revparse(['--show-toplevel'])).trim());
      const branch = await simpleGit(root).branch();
      return { valid: true, repoPath: root, currentBranch: branch.current };
    } catch {
      return { valid: false, repoPath, error: invalidRepositoryMessage };
    }
  }

  async listBranches(repoPath: string): Promise<GitBranch[]> {
    const branchSummary = await simpleGit(repoPath).branchLocal();
    return branchSummary.all.map((name) => ({
      name,
      current: name === branchSummary.current
    }));
  }

  async listCommits(repoPath: string, branch: string): Promise<GitCommitSummary[]> {
    const result = await simpleGit(repoPath).log([branch, '--date=iso-strict']);
    return result.all.map((commit) => ({
      hash: commit.hash,
      shortHash: commit.hash.slice(0, 8),
      authorName: commit.author_name,
      authorEmail: commit.author_email,
      committedAt: new Date(commit.date).toISOString(),
      subject: commit.message
    }));
  }

  async listAuthors(
    repoPath: string,
    branch: string,
    startAt?: string,
    endAt?: string
  ): Promise<AuthorFilterOption[]> {
    const commits = await this.listCommits(repoPath, branch);
    const startTime = startAt ? new Date(startAt).getTime() : Number.NEGATIVE_INFINITY;
    const endTime = endAt ? new Date(endAt).getTime() : Number.POSITIVE_INFINITY;
    const authors = new Map<string, AuthorFilterOption>();

    for (const commit of commits) {
      const committedTime = new Date(commit.committedAt).getTime();
      if (committedTime < startTime || committedTime > endTime) {
        continue;
      }

      const key = authorKey(commit.authorName, commit.authorEmail);
      const current = authors.get(key);
      authors.set(key, {
        key,
        name: commit.authorName,
        email: commit.authorEmail,
        commitCount: (current?.commitCount ?? 0) + 1
      });
    }

    return [...authors.values()].sort((left, right) => {
      if (right.commitCount !== left.commitCount) {
        return right.commitCount - left.commitCount;
      }
      return left.name.localeCompare(right.name);
    });
  }

  async listChangedFiles(
    repoPath: string,
    baseCommit: string,
    headCommit: string
  ): Promise<ChangedFile[]> {
    const git = simpleGit(repoPath);
    const nameStatus = await git.diff(['--name-status', '--find-renames', `${baseCommit}..${headCommit}`]);
    const numstat = await git.diff(['--numstat', `${baseCommit}..${headCommit}`]);
    return parseNumstat(parseNameStatus(nameStatus), numstat);
  }

  async readFileAtCommit(repoPath: string, commit: string, filePath: string): Promise<string> {
    try {
      return await simpleGit(repoPath).show([`${commit}:${normalizeGitPath(filePath)}`]);
    } catch {
      return '';
    }
  }

  async showFilePatch(repoPath: string, commit: string, filePath: string): Promise<string> {
    try {
      return await simpleGit(repoPath).show([
        '--format=',
        '--find-renames',
        commit,
        '--',
        normalizeGitPath(filePath)
      ]);
    } catch {
      return '';
    }
  }
}

function normalizeRepositoryPath(repoPath: string): string {
  return resolve(repoPath).replace(/\\/g, '/').replace(/\/$/, '');
}

function authorKey(name: string, email?: string): string {
  return (email?.trim() || name.trim()).toLowerCase();
}
