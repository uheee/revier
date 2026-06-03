import { attachAttribution } from '../../src/main/analysis/attributionEngine';
import type { DiffBlock, RelatedCommit } from '../../src/shared/reviewTypes';

const block: DiffBlock = {
  id: 'block-1',
  oldStart: 10,
  oldEnd: 11,
  newStart: 10,
  newEnd: 12,
  changeType: 'modified',
  authors: [],
  rows: [],
  relatedCommits: []
};

const commits: RelatedCommit[] = [
  {
    hash: 'a'.repeat(40),
    shortHash: 'aaaaaaaa',
    authorName: 'Alice',
    authorEmail: 'alice@example.com',
    committedAt: '2026-05-01T00:00:00.000Z',
    subject: 'touch block',
    matchedByFilter: false,
    touchedRanges: [{ oldStart: 10, oldEnd: 10, newStart: 10, newEnd: 10 }]
  },
  {
    hash: 'b'.repeat(40),
    shortHash: 'bbbbbbbb',
    authorName: 'Bob',
    authorEmail: 'bob@example.com',
    committedAt: '2026-05-02T00:00:00.000Z',
    subject: 'outside block',
    matchedByFilter: false,
    touchedRanges: [{ oldStart: 30, oldEnd: 30, newStart: 30, newEnd: 30 }]
  }
];

describe('attributionEngine', () => {
  it('adds authors and related commits that intersect a diff block', () => {
    const [attributed] = attachAttribution([block], commits);

    expect(attributed.authors).toEqual([{ name: 'Alice', email: 'alice@example.com' }]);
    expect(attributed.relatedCommits.map((commit) => commit.authorName)).toEqual(['Alice']);
  });
});
