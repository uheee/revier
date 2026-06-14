import type { AuthorSummary, DiffBlock, RelatedCommit } from '../../shared/reviewTypes';
import type { AnalysisGitClient, BlameLine } from '../git/analysisGitClient';

export interface ResolveBlockAttributionInput { repoPath: string; headCommit: string; filePath: string; block: DiffBlock; git: Pick<AnalysisGitClient, 'blameFileRange' | 'listParents' | 'getCommit'>; }

export async function resolveBlockAttribution({ repoPath, headCommit, filePath, block, git }: ResolveBlockAttributionInput): Promise<DiffBlock> {
  if (block.newStart <= 0 || block.newEnd <= 0) return inferred(block, 'DELETION_TRACE_INCOMPLETE');
  const blame = await git.blameFileRange(repoPath, headCommit, filePath, block.newStart, block.newEnd);
  if (blame.length === 0) return { ...inferred(block, 'BLAME_UNAVAILABLE'), authors: block.authors, relatedCommits: block.relatedCommits };
  const related = new Map<string, RelatedCommit>();
  for (const line of blame) {
    const commit = await toRelatedCommit(repoPath, line, git);
    related.set(commit.hash, commit);
  }
  const relatedCommits = [...related.values()];
  return { ...block, authors: uniqueAuthors(relatedCommits), relatedCommits, attribution: { confidence: 'precise', warnings: [] } };
}

async function toRelatedCommit(repoPath: string, line: BlameLine, git: Pick<AnalysisGitClient, 'listParents' | 'getCommit'>): Promise<RelatedCommit> {
  const parents = await git.listParents(repoPath, line.commitHash).catch(() => []);
  const commit = (await git.getCommit(repoPath, line.commitHash)) ?? { hash: line.commitHash, shortHash: line.commitHash.slice(0,8), authorName: line.authorName, authorEmail: line.authorEmail, committedAt: line.committedAt, subject: line.subject, matchedByFilter: false, touchedRanges: [] };
  return { ...commit, attribution: { method: parents.length > 1 ? 'merge-trace' : 'blame', viaMergeHashes: parents.length > 1 ? [line.commitHash] : [] } };
}

function inferred(block: DiffBlock, code: 'BLAME_UNAVAILABLE' | 'DELETION_TRACE_INCOMPLETE'): DiffBlock { return { ...block, attribution: { confidence: 'inferred', warnings: [{ code, message: code === 'BLAME_UNAVAILABLE' ? '无法使用 blame 结果，已根据提交差异推断' : '删除块无法通过最终内容 blame，已根据提交差异推断' }] } }; }
function uniqueAuthors(commits: RelatedCommit[]): AuthorSummary[] { const m = new Map<string, AuthorSummary>(); for (const c of commits) m.set(`${c.authorName}<${c.authorEmail ?? ''}>`, { name: c.authorName, email: c.authorEmail }); return [...m.values()]; }
