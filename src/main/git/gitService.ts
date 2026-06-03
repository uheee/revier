import simpleGit from 'simple-git';
import type { GitBranch, RepositoryValidation } from '../../shared/projectTypes';

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
}
