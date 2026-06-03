import { resolveDefaultDateRange, selectRangeCommits } from '../../src/main/analysis/rangeResolver';

describe('rangeResolver', () => {
  it('uses the last 30 days when no dates are supplied', () => {
    const now = new Date('2026-06-03T00:00:00.000Z');
    const range = resolveDefaultDateRange({}, now);

    expect(range.startAt).toBe('2026-05-04T00:00:00.000Z');
    expect(range.endAt).toBe('2026-06-03T00:00:00.000Z');
  });

  it('selects base, head, and range commits from chronological commits', () => {
    const resolved = selectRangeCommits(
      [
        { hash: 'a', committedAt: '2026-05-01T00:00:00.000Z' },
        { hash: 'b', committedAt: '2026-05-10T00:00:00.000Z' },
        { hash: 'c', committedAt: '2026-05-20T00:00:00.000Z' }
      ],
      {
        branch: 'main',
        startAt: '2026-05-05T00:00:00.000Z',
        endAt: '2026-05-31T00:00:00.000Z'
      }
    );

    expect(resolved.baseCommit).toBe('a');
    expect(resolved.headCommit).toBe('c');
    expect(resolved.rangeCommits.map((commit) => commit.hash)).toEqual(['b', 'c']);
  });
});
