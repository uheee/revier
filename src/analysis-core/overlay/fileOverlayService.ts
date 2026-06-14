import type { GitCommitSummary } from '../../main/git/gitTypes';
import { attachAttribution } from '../../main/analysis/attributionEngine';
import { buildFileOverlayDiff } from '../../main/analysis/overlayEngine';
import { parsePatchTouchedRanges } from '../../main/analysis/patchRanges';
import type { AnalysisRange, ChangedFile, FileOverlay, RelatedCommit, ReviewFilters } from '../../shared/reviewTypes';
import type { AnalysisGitClient } from '../git/analysisGitClient';
import { resolveBlockAttribution } from '../attribution/attributionService';

export interface BuildFileOverlayCoreInput { repoPath: string; file: ChangedFile; range: AnalysisRange; rangeCommits: GitCommitSummary[]; filters: ReviewFilters; git: Pick<AnalysisGitClient, 'readFileAtCommit' | 'showFilePatch' | 'blameFileRange' | 'listParents' | 'getCommit'>; }

export async function buildFileOverlay({ repoPath, file, range, rangeCommits, filters, git }: BuildFileOverlayCoreInput): Promise<FileOverlay> {
  const oldText = await git.readFileAtCommit(repoPath, range.baseCommit, file.oldPath ?? file.path);
  const newText = await git.readFileAtCommit(repoPath, range.headCommit, file.path);
  const diff = buildFileOverlayDiff({ file, oldText, newText });
  const patchRelated = await buildRelatedCommits(repoPath, file, rangeCommits, filters, git);
  const patchBlocks = attachAttribution(diff.blocks, patchRelated);
  const blocks = [];
  for (const block of patchBlocks) blocks.push(await resolveBlockAttribution({ repoPath, headCommit: range.headCommit, filePath: file.path, block, git }));
  return { mode: 'range', file, range, rows: diff.rows, blocks, warnings: [] };
}
async function buildRelatedCommits(repoPath: string, file: ChangedFile, commits: GitCommitSummary[], _filters: ReviewFilters, git: Pick<AnalysisGitClient, 'showFilePatch'>): Promise<RelatedCommit[]> { const out: RelatedCommit[] = []; for (const c of commits) { const patch = await git.showFilePatch(repoPath, c.hash, file.path); const touchedRanges = parsePatchTouchedRanges(patch); if (touchedRanges.length) out.push({ hash: c.hash, shortHash: c.shortHash, authorName: c.authorName, authorEmail: c.authorEmail, committedAt: c.committedAt, subject: c.subject, matchedByFilter: hasDisplayCommitFilters(_filters) ? commitMatchesDisplayFilters(c, _filters) : false, touchedRanges, attribution: { method: 'patch-inference', viaMergeHashes: [] } }); } return out; }

function hasDisplayCommitFilters(filters: ReviewFilters): boolean { return Boolean((filters.authorKeys?.length ?? 0) > 0 || filters.authorQuery?.trim() || filters.messageQuery?.trim()); }
function commitMatchesDisplayFilters(commit: GitCommitSummary, filters: ReviewFilters): boolean { return matchesAuthorKeys(commit, filters.authorKeys ?? []) && matchesQuery(`${commit.authorName} ${commit.authorEmail ?? ''}`, filters.authorQuery) && matchesQuery(commit.subject, filters.messageQuery); }
function matchesAuthorKeys(commit: GitCommitSummary, authorKeys: string[]): boolean { if (authorKeys.length === 0) return true; const keys = new Set(authorKeys.map((key) => key.toLowerCase())); return keys.has((commit.authorEmail?.trim() || commit.authorName.trim()).toLowerCase()); }
function matchesQuery(value: string, query?: string): boolean { const normalized = query?.trim().toLowerCase(); return !normalized || value.toLowerCase().includes(normalized); }
