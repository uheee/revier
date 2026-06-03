import type { AuthorSummary, DiffBlock, RelatedCommit, TouchedRange } from '../../shared/reviewTypes';
import { rangesIntersect } from './patchRanges';

export function attachAttribution(blocks: DiffBlock[], commits: RelatedCommit[]): DiffBlock[] {
  return blocks.map((block) => {
    const blockRange: TouchedRange = {
      oldStart: block.oldStart || undefined,
      oldEnd: block.oldEnd || undefined,
      newStart: block.newStart || undefined,
      newEnd: block.newEnd || undefined
    };
    const relatedCommits = commits.filter((commit) =>
      commit.touchedRanges.some((range) => rangesIntersect(blockRange, range))
    );

    return {
      ...block,
      authors: uniqueAuthors(relatedCommits),
      relatedCommits
    };
  });
}

function uniqueAuthors(commits: RelatedCommit[]): AuthorSummary[] {
  const authors = new Map<string, AuthorSummary>();
  for (const commit of commits) {
    const key = `${commit.authorName}<${commit.authorEmail ?? ''}>`;
    authors.set(key, { name: commit.authorName, email: commit.authorEmail });
  }
  return [...authors.values()];
}
