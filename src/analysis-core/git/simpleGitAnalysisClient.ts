import simpleGit from 'simple-git';
import { normalizeGitPath, parseNameStatus, parseNumstat } from '../../main/analysis/changedFiles';
import type { ChangedFile, RelatedCommit } from '../../shared/reviewTypes';
import type { AnalysisGitClient, BlameLine } from './analysisGitClient';
import { parseBlamePorcelain } from './blameParser';

export class SimpleGitAnalysisClient implements AnalysisGitClient {
  async listChangedFiles(repoPath: string, baseCommit: string, headCommit: string): Promise<ChangedFile[]> {
    const git = simpleGit(repoPath);
    const nameStatus = await git.diff(['--name-status', '--find-renames', `${baseCommit}..${headCommit}`]);
    const numstat = await git.diff(['--numstat', `${baseCommit}..${headCommit}`]);
    return parseNumstat(parseNameStatus(nameStatus), numstat);
  }
  async readFileAtCommit(repoPath: string, commit: string, filePath: string): Promise<string> { try { return await simpleGit(repoPath).show([`${commit}:${normalizeGitPath(filePath)}`]); } catch { return ''; } }
  async showFilePatch(repoPath: string, commit: string, filePath: string): Promise<string> { try { return await simpleGit(repoPath).show(['--format=', '--find-renames', commit, '--', normalizeGitPath(filePath)]); } catch { return ''; } }
  async blameFileRange(repoPath: string, commit: string, filePath: string, startLine: number, endLine: number): Promise<BlameLine[]> { if (startLine <= 0 || endLine < startLine) return []; try { const out = await simpleGit(repoPath).raw(['blame', '--line-porcelain', '-M', '-C', `-L${startLine},${endLine}`, commit, '--', normalizeGitPath(filePath)]); return parseBlamePorcelain(out); } catch { return []; } }
  async listParents(repoPath: string, commitHash: string): Promise<string[]> { const line = (await simpleGit(repoPath).raw(['rev-list', '--parents', '-n', '1', commitHash])).trim(); return line.split(/\s+/).slice(1); }
  async getCommit(repoPath: string, commitHash: string): Promise<RelatedCommit | undefined> { try { const out = await simpleGit(repoPath).show(['-s', '--date=iso-strict', '--format=%H%x00%an%x00%ae%x00%aI%x00%s', commitHash]); const [hash, authorName, authorEmail, committedAt, subject] = out.trim().split('\0'); return { hash, shortHash: hash.slice(0,8), authorName, authorEmail, committedAt, subject, matchedByFilter: false, touchedRanges: [] }; } catch { return undefined; } }
}
