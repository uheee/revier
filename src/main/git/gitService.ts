import simpleGit from 'simple-git';
import { normalizeGitPath, parseNameStatus, parseNumstat } from '../analysis/changedFiles';
import type { GitBranch, RepositoryValidation } from '../../shared/projectTypes';
import type { ChangedFile } from '../../shared/reviewTypes';
import type { GitCommitSummary } from './gitTypes';

export class GitService {
  async validateRepository(repoPath: string): Promise<RepositoryValidation> {
    const git = simpleGit(repoPath);
    try {
      const isRepo = await git.checkIsRepo();
      if (!isRepo) {
        return { valid: false, repoPath, error: 'Not a Git repository' };
      }

      const branch = await git.branch();
      return { valid: true, repoPath, currentBranch: branch.current };
    } catch (error) {
      return { valid: false, repoPath, error: String(error) };
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
