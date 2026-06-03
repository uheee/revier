export interface CommitSummaryForRange {
  hash: string;
  committedAt: string;
}

export interface RangeInput {
  branch: string;
  startAt?: string;
  endAt?: string;
}

export interface ResolvedCommitRange {
  branch: string;
  baseCommit: string;
  headCommit: string;
  rangeCommits: CommitSummaryForRange[];
  startAt: string;
  endAt: string;
}

export function resolveDefaultDateRange(
  input: Partial<RangeInput>,
  now = new Date()
): Required<Pick<RangeInput, 'startAt' | 'endAt'>> {
  const end = input.endAt ? new Date(input.endAt) : now;
  const start = input.startAt ? new Date(input.startAt) : new Date(end);
  if (!input.startAt) {
    start.setUTCDate(start.getUTCDate() - 30);
  }

  return { startAt: start.toISOString(), endAt: end.toISOString() };
}

export function selectRangeCommits(
  commits: CommitSummaryForRange[],
  input: RangeInput
): ResolvedCommitRange {
  const dates = resolveDefaultDateRange(input);
  const start = Date.parse(dates.startAt);
  const end = Date.parse(dates.endAt);
  const sorted = [...commits].sort(
    (left, right) => Date.parse(left.committedAt) - Date.parse(right.committedAt)
  );
  const rangeCommits = sorted.filter((commit) => {
    const time = Date.parse(commit.committedAt);
    return time >= start && time <= end;
  });
  const baseCommit =
    [...sorted].reverse().find((commit) => Date.parse(commit.committedAt) < start)?.hash ??
    sorted[0]?.hash;
  const headCommit =
    [...sorted].reverse().find((commit) => Date.parse(commit.committedAt) <= end)?.hash ??
    sorted.at(-1)?.hash;

  if (!baseCommit || !headCommit) {
    throw new Error('No commits available for selected range');
  }

  return { branch: input.branch, baseCommit, headCommit, rangeCommits, ...dates };
}
